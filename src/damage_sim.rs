//! Damage calculator: how much damage a turn deals, estimated by playing the turn many times
//! with shuffled decks (a Monte Carlo simulation).
//!
//! A [`Turn`] describes the player's side: the soul of the three characters on stage (the
//! climax's bonus included, as the user enters it), the [`Source`]s of damage in the order
//! they happen, and what is left in the player's deck (for trigger checks). [`Opponent`]
//! describes the defending player's deck, waiting room, stock, level and clock. [`simulate`]
//! plays the turn `trials` times and returns an [`Outcome`]: how often each total damage
//! happened, and how often the opponent died.
//!
//! The sources, one after the other:
//! - an attack: the top card of the player's deck is the trigger check, its soul icons go to
//!   the attacking character for the rest of the turn (a character attacking again keeps
//!   them); then cards are revealed from the opponent's deck one by one, a climax cancels the
//!   damage (the cards go to the waiting room), otherwise they go to the clock; a shot trigger
//!   deals 1 more damage when the damage is cancelled;
//! - a burn, maybe only when the last attack was cancelled, not cancelled, or its trigger
//!   check revealed a climax;
//! - a mill: the top cards of the opponent's deck go to their waiting room;
//! - a Musashi: reveals a card of the player's deck and deals its level + 1 damage. The card
//!   is drawn from the deck as it was at the start of the turn ([`Turn::musashi`]), apart from
//!   the trigger checks.
//!
//! The opponent refreshes when their deck runs out (waiting room shuffled back, plus 1
//! uncancellable damage), and levels up at 7 clock. Card power, battles and counters are not
//! simulated.
//!
//! Decks are not stored card by card: a deck is a count of each kind of card, and drawing
//! picks one at random. That is the same as drawing from a shuffled deck, and much faster.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{Card, CardType, Trigger};

/// Cards in the clock that make the player level up.
const CLOCK_SIZE: u32 = 7;
/// The level at which a player loses.
const LETHAL_LEVEL: u32 = 4;
/// Deck building rules: a deck has at most 50 cards, and at most 8 of them are climaxes.
pub const DECK_SIZE: u32 = 50;
pub const DECK_CLIMAXES: u32 = 8;
/// Characters on the stage that can attack.
pub const CHARACTERS: usize = 3;

/// What a source of damage is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    /// A character attacks, with a trigger check.
    Attack,
    /// Deal damage (when attacking, after the attack, when the climax is placed…).
    Burn,
    /// Deal damage if the last attack's damage was cancelled.
    CancelBurn,
    /// Deal damage if the last attack's damage was not cancelled.
    NotCancelledBurn,
    /// Deal damage if the last attack's trigger check revealed a climax.
    ClimaxTriggerBurn,
    /// Put the top cards of the opponent's deck in their waiting room.
    Mill,
    /// Reveal a card of the player's deck and deal its level + 1 damage.
    Musashi,
    /// The same, if the last attack's damage was cancelled.
    CancelMusashi,
}

impl SourceKind {
    pub const ALL: [SourceKind; 8] = [
        SourceKind::Attack,
        SourceKind::Burn,
        SourceKind::CancelBurn,
        SourceKind::NotCancelledBurn,
        SourceKind::ClimaxTriggerBurn,
        SourceKind::Mill,
        SourceKind::Musashi,
        SourceKind::CancelMusashi,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SourceKind::Attack => "Attack",
            SourceKind::Burn => "Burn",
            SourceKind::CancelBurn => "Burn if cancelled",
            SourceKind::NotCancelledBurn => "Burn if not cancelled",
            SourceKind::ClimaxTriggerBurn => "Burn on climax trigger",
            SourceKind::Mill => "Mill opponent",
            SourceKind::Musashi => "Musashi",
            SourceKind::CancelMusashi => "Musashi if cancelled",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            SourceKind::Attack => {
                "A character attacks: trigger check, then damage equal to its soul. Its trigger \
                 soul stays for its next attacks"
            }
            SourceKind::Burn => {
                "Deal this much damage: when attacking, after the attack, when the climax is \
                 placed… Its place in the list says when"
            }
            SourceKind::CancelBurn => "If the last attack's damage was cancelled, deal this much",
            SourceKind::NotCancelledBurn => {
                "If the last attack's damage was not cancelled, deal this much"
            }
            SourceKind::ClimaxTriggerBurn => {
                "If the last attack's trigger check revealed a climax, deal this much"
            }
            SourceKind::Mill => {
                "Put this many cards from the top of the opponent's deck into their waiting room"
            }
            SourceKind::Musashi => {
                "Reveal a card of your deck (minus the stage and the climax) and deal its \
                 level + 1 damage"
            }
            SourceKind::CancelMusashi => {
                "If the last attack's damage was cancelled: reveal a card of your deck and deal \
                 its level + 1 damage"
            }
        }
    }

    /// Whether `amount` matters: damage or cards milled.
    pub fn has_amount(self) -> bool {
        !matches!(
            self,
            SourceKind::Attack | SourceKind::Musashi | SourceKind::CancelMusashi
        )
    }
}

/// How an attack is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttackKind {
    Front,
    /// Side attack: loses as much soul as the level of the character in front of it.
    Side,
    /// Direct attack (no character in front): +1 soul.
    Direct,
}

impl AttackKind {
    pub const ALL: [AttackKind; 3] = [AttackKind::Front, AttackKind::Side, AttackKind::Direct];

    pub fn label(self) -> &'static str {
        match self {
            AttackKind::Front => "Front",
            AttackKind::Side => "Side",
            AttackKind::Direct => "Direct",
        }
    }
}

/// One source of damage of the turn.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Source {
    pub kind: SourceKind,
    /// Damage, or cards milled. Unused by attacks and Musashi.
    pub amount: u32,
    /// For an attack: which character attacks (0 to 2), and how.
    pub character: usize,
    pub attack: AttackKind,
    /// For a side attack: the level of the opponent's character in front of it.
    pub defender_level: u32,
}

impl Source {
    pub fn new(kind: SourceKind, amount: u32) -> Self {
        Self {
            kind,
            amount,
            character: 0,
            attack: AttackKind::Front,
            defender_level: 0,
        }
    }

    /// A front attack by `character`.
    pub fn attack(character: usize) -> Self {
        Self {
            character,
            ..Self::new(SourceKind::Attack, 0)
        }
    }
}

/// What a card revealed by a trigger check does to the attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TriggerKind {
    pub climax: bool,
    /// Soul icons (0, 1 or 2).
    pub soul: u32,
    /// Shot trigger: 1 damage when the attack's damage is cancelled.
    pub shot: bool,
}

impl TriggerKind {
    /// The kinds the deck state is entered with, in display order.
    pub const ALL: [TriggerKind; 7] = [
        TriggerKind::new(false, 0, false),
        TriggerKind::new(false, 1, false),
        TriggerKind::new(false, 2, false),
        TriggerKind::new(true, 0, false),
        TriggerKind::new(true, 1, false),
        TriggerKind::new(true, 2, false),
        TriggerKind::new(true, 1, true),
    ];

    pub const fn new(climax: bool, soul: u32, shot: bool) -> Self {
        Self { climax, soul, shot }
    }

    /// The kind of a card, from its trigger icons. A non-climax shot card counts as a climax
    /// shot trigger: shots are on climaxes.
    pub fn of(card: &Card) -> Self {
        let soul = card
            .triggers
            .iter()
            .filter(|t| **t == Trigger::Soul)
            .count() as u32;
        let shot = card.triggers.contains(&Trigger::Burn);
        if shot {
            return TriggerKind::new(true, 1, true);
        }
        TriggerKind::new(card.card_type == CardType::Climax, soul.min(2), false)
    }

    pub fn label(self) -> &'static str {
        match (self.shot, self.soul) {
            (true, _) => "Shot (+1 soul)",
            (false, 0) => "No soul",
            (false, 1) => "+1 soul",
            _ => "+2 soul",
        }
    }
}

/// The player's deck for the trigger checks: how many cards of each [`TriggerKind::ALL`] kind
/// are left in it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DeckState {
    pub counts: [u32; TriggerKind::ALL.len()],
}

impl DeckState {
    /// The state of a deck holding `cards`.
    pub fn of<'a>(cards: impl IntoIterator<Item = &'a Card>) -> Self {
        let mut state = Self::default();
        for card in cards {
            state.add(TriggerKind::of(card), 1);
        }
        state
    }

    pub fn total(&self) -> u32 {
        self.counts.iter().sum()
    }

    pub fn climaxes(&self) -> u32 {
        TriggerKind::ALL
            .iter()
            .zip(&self.counts)
            .filter(|(k, _)| k.climax)
            .map(|(_, n)| n)
            .sum()
    }

    /// Adds (or with a negative `n`, removes) cards of a kind; counts stay at 0 or more.
    pub fn add(&mut self, kind: TriggerKind, n: i32) {
        if let Some(i) = TriggerKind::ALL.iter().position(|k| *k == kind) {
            self.counts[i] = self.counts[i].saturating_add_signed(n);
        }
    }

    /// Takes a random card out; `None` when the deck is empty.
    fn draw(&mut self, rng: &mut Rng) -> Option<TriggerKind> {
        let mut pick = rng.below(self.total());
        for (i, n) in self.counts.iter_mut().enumerate() {
            if pick < *n {
                *n -= 1;
                return Some(TriggerKind::ALL[i]);
            }
            pick -= *n;
        }
        None
    }
}

/// Card levels a deck holds: 0 to 3. Climaxes are level 0.
pub const LEVELS: usize = 4;

/// How many cards of each level a deck holds, for Musashi effects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Levels(pub [u32; LEVELS]);

impl Levels {
    pub fn of<'a>(cards: impl IntoIterator<Item = &'a Card>) -> Self {
        let mut levels = Self::default();
        for card in cards {
            levels.0[(card.level as usize).min(LEVELS - 1)] += 1;
        }
        levels
    }

    pub fn total(&self) -> u32 {
        self.0.iter().sum()
    }

    /// The average damage of a Musashi (the level + 1 of a random card); `None` when there
    /// are no cards.
    pub fn musashi_damage(&self) -> Option<f64> {
        let total = self.total();
        if total == 0 {
            return None;
        }
        let sum: u32 = (1..).zip(self.0).map(|(damage, n)| damage * n).sum();
        Some(f64::from(sum) / f64::from(total))
    }

    /// The level + 1 of a random card, which stays in the deck; `None` when there are none.
    fn musashi(&self, rng: &mut Rng) -> Option<u32> {
        let mut pick = rng.below(self.total());
        for (damage, n) in (1..).zip(self.0) {
            if pick < n {
                return Some(damage);
            }
            pick -= n;
        }
        None
    }
}

/// The player's side of the turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Turn {
    /// The soul of each character on stage, every bonus included (the climax's…).
    pub souls: [u32; CHARACTERS],
    /// In the order they happen.
    pub sources: Vec<Source>,
    /// What is left in the player's deck, for the trigger checks.
    pub deck: DeckState,
    /// The player's deck at the start of the turn, for Musashi effects.
    pub musashi: Levels,
}

/// The defending player.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Opponent {
    pub deck_cards: u32,
    pub deck_climaxes: u32,
    pub waiting_room_cards: u32,
    pub waiting_room_climaxes: u32,
    /// Cards in the stock: out of the deck, none of them a climax.
    pub stock: u32,
    /// 0 to 3.
    pub level: u32,
    /// Cards in the clock, 0 to 6.
    pub clock: u32,
}

impl Default for Opponent {
    /// Level 3 with 8 stock, just refreshed.
    fn default() -> Self {
        let mut o = Self {
            deck_cards: 0,
            deck_climaxes: 0,
            waiting_room_cards: 0,
            waiting_room_climaxes: 0,
            stock: 8,
            level: 3,
            clock: 0,
        };
        o.refresh();
        o
    }
}

impl Opponent {
    /// A deck just refreshed: every card not in the stock, level or clock is in the deck,
    /// with all 8 climaxes.
    pub fn refresh(&mut self) {
        self.deck_cards = DECK_SIZE.saturating_sub(self.stock + self.level + self.clock);
        self.deck_climaxes = DECK_CLIMAXES.min(self.deck_cards);
        self.waiting_room_cards = 0;
        self.waiting_room_climaxes = 0;
    }

    /// Damage that kills the opponent: what is left before level 4.
    pub fn damage_to_kill(&self) -> u32 {
        (LETHAL_LEVEL - 1).saturating_sub(self.level) * CLOCK_SIZE
            + CLOCK_SIZE.saturating_sub(self.clock)
    }

    /// Cards of the opponent's deck the page knows of: deck, waiting room, stock, clock and
    /// level.
    pub fn cards(&self) -> u32 {
        self.deck_cards + self.waiting_room_cards + self.stock + self.clock + self.level
    }

    pub fn climaxes(&self) -> u32 {
        self.deck_climaxes + self.waiting_room_climaxes
    }

    /// Why this state cannot be simulated, if it cannot.
    pub fn problem(&self) -> Option<&'static str> {
        if self.cards() > DECK_SIZE {
            Some(
                "The opponent has more than 50 cards in their deck, waiting room, stock, clock \
                 and level.",
            )
        } else if self.climaxes() > DECK_CLIMAXES {
            Some("The opponent has more than 8 climaxes in their deck and waiting room.")
        } else if self.deck_climaxes > self.deck_cards {
            Some("The opponent's deck has more climaxes than cards.")
        } else if self.waiting_room_climaxes > self.waiting_room_cards {
            Some("The opponent's waiting room has more climaxes than cards.")
        } else if self.deck_cards + self.waiting_room_cards == 0 {
            Some("The opponent has no cards in their deck or waiting room.")
        } else {
            None
        }
    }
}

/// What one source did, summed over all trials.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourceStats {
    /// Trials where it happened (its condition was met).
    pub happened: u32,
    /// Damage it dealt, with its trigger's shot for an attack.
    pub damage: u64,
    /// For an attack: trials where its damage was cancelled.
    pub cancels: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub trials: u32,
    /// `histogram[n]`: trials where the opponent took `n` damage in total.
    pub histogram: Vec<u32>,
    /// Trials where the opponent reached level 4 (or could not refresh).
    pub kills: u32,
    pub damage_to_kill: u32,
    /// One per source of the turn, in the same order.
    pub sources: Vec<SourceStats>,
}

impl Outcome {
    pub fn mean(&self) -> f64 {
        let total: u64 = self
            .histogram
            .iter()
            .enumerate()
            .map(|(n, &t)| n as u64 * u64::from(t))
            .sum();
        total as f64 / f64::from(self.trials.max(1))
    }

    /// The chance of exactly `n` damage.
    pub fn chance_of(&self, n: usize) -> f64 {
        f64::from(self.histogram.get(n).copied().unwrap_or(0)) / f64::from(self.trials.max(1))
    }

    /// The chance of `n` damage or more.
    pub fn chance_at_least(&self, n: usize) -> f64 {
        let hits: u32 = self.histogram.iter().skip(n).sum();
        f64::from(hits) / f64::from(self.trials.max(1))
    }

    pub fn kill_chance(&self) -> f64 {
        f64::from(self.kills) / f64::from(self.trials.max(1))
    }

    /// The damage reached or exceeded in half of the trials.
    pub fn median(&self) -> usize {
        let half = self.trials.div_ceil(2);
        let mut seen = 0;
        for (n, &t) in self.histogram.iter().enumerate() {
            seen += t;
            if seen >= half {
                return n;
            }
        }
        0
    }
}

/// What the last attack did, for the sources that depend on it.
#[derive(Clone, Copy, Default)]
struct LastAttack {
    cancelled: bool,
    climax_trigger: bool,
}

/// Plays the turn `trials` times; `seed` picks the random sequence (the same seed gives the
/// same outcome).
pub fn simulate(turn: &Turn, opponent: &Opponent, trials: u32, seed: u64) -> Outcome {
    let mut rng = Rng::new(seed);
    let mut outcome = Outcome {
        trials,
        damage_to_kill: opponent.damage_to_kill(),
        sources: vec![SourceStats::default(); turn.sources.len()],
        ..Outcome::default()
    };
    for _ in 0..trials {
        let mut opp = Defender::new(opponent);
        let mut deck = turn.deck.clone();
        // Trigger soul stays on the character for the turn.
        let mut souls = turn.souls;
        // Before any attack, nothing was cancelled.
        let mut last = LastAttack::default();
        for (source, stats) in turn.sources.iter().zip(&mut outcome.sources) {
            let before = opp.taken;
            let happened = match source.kind {
                SourceKind::Attack => {
                    let character = source.character.min(CHARACTERS - 1);
                    let soul = match source.attack {
                        AttackKind::Front => souls[character],
                        AttackKind::Side => souls[character].saturating_sub(source.defender_level),
                        AttackKind::Direct => souls[character] + 1,
                    };
                    let trigger = deck.draw(&mut rng);
                    let trigger_soul = trigger.map_or(0, |t| t.soul);
                    souls[character] += trigger_soul;
                    let cancelled = opp.damage(soul + trigger_soul, &mut rng);
                    if cancelled {
                        stats.cancels += 1;
                        if trigger.is_some_and(|t| t.shot) {
                            opp.damage(1, &mut rng);
                        }
                    }
                    last = LastAttack {
                        cancelled,
                        climax_trigger: trigger.is_some_and(|t| t.climax),
                    };
                    true
                }
                SourceKind::Burn => true,
                SourceKind::CancelBurn | SourceKind::CancelMusashi => last.cancelled,
                SourceKind::NotCancelledBurn => !last.cancelled,
                SourceKind::ClimaxTriggerBurn => last.climax_trigger,
                SourceKind::Mill => {
                    opp.mill(source.amount, &mut rng);
                    true
                }
                SourceKind::Musashi => true,
            };
            if !happened {
                continue;
            }
            match source.kind {
                SourceKind::Burn
                | SourceKind::CancelBurn
                | SourceKind::NotCancelledBurn
                | SourceKind::ClimaxTriggerBurn => {
                    opp.damage(source.amount, &mut rng);
                }
                SourceKind::Musashi | SourceKind::CancelMusashi => {
                    if let Some(n) = turn.musashi.musashi(&mut rng) {
                        opp.damage(n, &mut rng);
                    }
                }
                SourceKind::Attack | SourceKind::Mill => {}
            }
            stats.happened += 1;
            stats.damage += u64::from(opp.taken - before);
        }
        let taken = opp.taken as usize;
        if outcome.histogram.len() <= taken {
            outcome.histogram.resize(taken + 1, 0);
        }
        outcome.histogram[taken] += 1;
        if opp.dead {
            outcome.kills += 1;
        }
    }
    outcome
}

/// A random seed, different on each run.
pub fn random_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0x5EED, |d| d.as_nanos() as u64)
}

/// A pile of cards, of which some are climaxes.
#[derive(Clone, Copy, Default)]
struct Pile {
    cards: u32,
    climaxes: u32,
}

impl Pile {
    /// Takes a random card; returns whether it is a climax. The pile must not be empty.
    fn draw(&mut self, rng: &mut Rng) -> bool {
        let climax = rng.below(self.cards) < self.climaxes;
        self.cards -= 1;
        if climax {
            self.climaxes -= 1;
        }
        climax
    }

    fn put(&mut self, climax: bool) {
        self.cards += 1;
        if climax {
            self.climaxes += 1;
        }
    }

    fn put_all(&mut self, other: Pile) {
        self.cards += other.cards;
        self.climaxes += other.climaxes;
    }
}

/// The opponent during one trial.
struct Defender {
    deck: Pile,
    waiting_room: Pile,
    clock: Pile,
    level: u32,
    /// Damage taken this turn.
    taken: u32,
    dead: bool,
}

impl Defender {
    fn new(o: &Opponent) -> Self {
        Self {
            deck: Pile {
                cards: o.deck_cards,
                climaxes: o.deck_climaxes,
            },
            waiting_room: Pile {
                cards: o.waiting_room_cards,
                climaxes: o.waiting_room_climaxes,
            },
            // The cards already in the clock are not climaxes: a climax cancels damage.
            clock: Pile {
                cards: o.clock.min(CLOCK_SIZE - 1),
                climaxes: 0,
            },
            level: o.level,
            taken: 0,
            dead: o.level >= LETHAL_LEVEL,
        }
    }

    /// Refreshes when the deck is empty: the waiting room becomes the deck. The rules do it
    /// at once, even in the middle of a damage.
    fn refresh_if_empty(&mut self, refreshes: &mut u32) {
        if self.deck.cards == 0 && self.waiting_room.cards > 0 {
            self.deck = std::mem::take(&mut self.waiting_room);
            *refreshes += 1;
        }
    }

    /// Takes the top card of the deck; returns whether it is a climax. `None` when there is
    /// nothing left, even to refresh with: the opponent loses.
    fn reveal(&mut self, refreshes: &mut u32, rng: &mut Rng) -> Option<bool> {
        self.refresh_if_empty(refreshes);
        if self.deck.cards == 0 {
            self.dead = true;
            return None;
        }
        let climax = self.deck.draw(rng);
        self.refresh_if_empty(refreshes);
        Some(climax)
    }

    /// Deals `n` damage; returns whether it was cancelled.
    fn damage(&mut self, n: u32, rng: &mut Rng) -> bool {
        if n == 0 {
            return false;
        }
        let mut refreshes = 0;
        let mut revealed = Pile::default();
        let mut cancelled = false;
        for _ in 0..n {
            let Some(climax) = self.reveal(&mut refreshes, rng) else {
                break;
            };
            revealed.put(climax);
            if climax {
                cancelled = true;
                break;
            }
        }
        if cancelled {
            self.waiting_room.put_all(revealed);
        } else {
            for _ in 0..revealed.cards {
                self.put_in_clock(false);
            }
        }
        self.pay_refreshes(refreshes, rng);
        cancelled
    }

    /// The refresh penalty: the top card of the deck goes to the clock, cancelled by nothing.
    fn pay_refreshes(&mut self, mut refreshes: u32, rng: &mut Rng) {
        while refreshes > 0 {
            refreshes -= 1;
            if let Some(climax) = self.reveal(&mut refreshes, rng) {
                self.put_in_clock(climax);
            }
        }
    }

    fn put_in_clock(&mut self, climax: bool) {
        self.clock.put(climax);
        self.taken += 1;
        if self.clock.cards >= CLOCK_SIZE {
            // Level up: 1 card to the level zone (a non-climax when possible), the rest to
            // the waiting room.
            let to_level_is_climax = self.clock.climaxes == self.clock.cards;
            let mut rest = std::mem::take(&mut self.clock);
            rest.cards -= 1;
            if to_level_is_climax {
                rest.climaxes -= 1;
            }
            self.waiting_room.put_all(rest);
            self.level += 1;
            if self.level >= LETHAL_LEVEL {
                self.dead = true;
            }
        }
    }

    fn mill(&mut self, n: u32, rng: &mut Rng) {
        let mut refreshes = 0;
        for _ in 0..n {
            let Some(climax) = self.reveal(&mut refreshes, rng) else {
                break;
            };
            self.waiting_room.put(climax);
        }
        self.pay_refreshes(refreshes, rng);
    }
}

/// A small, fast random number generator (SplitMix64): plenty for shuffling cards.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number from 0 to `n - 1`.
    fn below(&mut self, n: u32) -> u32 {
        (((self.next() >> 32) * u64::from(n)) >> 32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;

    fn card(card_type: CardType, level: u32, triggers: Vec<Trigger>) -> Card {
        Card {
            key: "XX/Y1-001".into(),
            card_type,
            name: "Card".into(),
            image: None,
            color: Color::Yellow,
            level,
            cost: 2,
            power: 9000,
            triggers,
            soul_count: 2,
            traits: Vec::new(),
            code: String::new(),
            text: String::new(),
        }
    }

    fn deck(non_climax: u32, climaxes: u32) -> DeckState {
        let mut d = DeckState::default();
        d.add(TriggerKind::new(false, 0, false), non_climax as i32);
        d.add(TriggerKind::new(true, 0, false), climaxes as i32);
        d
    }

    /// The three characters attack, front, in order.
    fn turn(souls: [u32; 3], own: DeckState) -> Turn {
        Turn {
            souls,
            sources: (0..CHARACTERS).map(Source::attack).collect(),
            deck: own,
            musashi: Levels::default(),
        }
    }

    /// Only the first character attacks, then `then`.
    fn one_attack(soul: u32, own: DeckState, then: SourceKind) -> Turn {
        Turn {
            souls: [soul, 0, 0],
            sources: vec![Source::attack(0), Source::new(then, 1)],
            deck: own,
            musashi: Levels::default(),
        }
    }

    fn opponent(cards: u32, climaxes: u32) -> Opponent {
        Opponent {
            deck_cards: cards,
            deck_climaxes: climaxes,
            waiting_room_cards: 0,
            waiting_room_climaxes: 0,
            stock: 0,
            level: 0,
            clock: 0,
        }
    }

    #[test]
    fn no_climax_means_every_damage_sticks() {
        let out = simulate(&turn([2, 2, 3], deck(20, 0)), &opponent(30, 0), 1000, 1);
        assert_eq!(out.histogram[7], 1000);
        assert_eq!(out.mean(), 7.0);
        assert_eq!(out.sources[2].damage, 3000);
    }

    #[test]
    fn only_climaxes_cancel_everything() {
        let out = simulate(&turn([2, 2, 2], deck(20, 0)), &opponent(10, 10), 500, 2);
        assert_eq!(out.histogram, vec![500]);
        assert_eq!(out.sources[0].cancels, 500);
    }

    #[test]
    fn one_damage_cancel_rate_matches_the_climax_ratio() {
        let mut t = turn([1, 1, 1], deck(20, 0));
        t.sources.truncate(1);
        let out = simulate(&t, &opponent(40, 10), 100_000, 3);
        let rate = f64::from(out.sources[0].cancels) / 100_000.0;
        assert!((rate - 0.25).abs() < 0.01, "{rate}");
    }

    #[test]
    fn attack_kinds_change_soul() {
        let mut t = turn([2, 2, 2], deck(20, 0));
        t.sources[0].attack = AttackKind::Direct;
        t.sources[1].attack = AttackKind::Side;
        t.sources[1].defender_level = 3;
        t.sources.truncate(2);
        let out = simulate(&t, &opponent(30, 0), 10, 4);
        assert_eq!(out.histogram[3], 10);
    }

    #[test]
    fn soul_triggers_add_damage() {
        let mut own = DeckState::default();
        own.add(TriggerKind::new(false, 2, false), 10);
        let out = simulate(&turn([1, 1, 1], own), &opponent(30, 0), 10, 5);
        assert_eq!(out.histogram[9], 10);
    }

    #[test]
    fn a_restanded_character_keeps_its_trigger_soul() {
        // 1 soul, +1 trigger: 2 damage; again with the first +1 and a new +1: 3 damage.
        let mut own = DeckState::default();
        own.add(TriggerKind::new(false, 1, false), 10);
        let t = Turn {
            souls: [1, 1, 1],
            sources: vec![Source::attack(0), Source::attack(0)],
            deck: own,
            musashi: Levels::default(),
        };
        let out = simulate(&t, &opponent(30, 0), 10, 6);
        assert_eq!(out.histogram[5], 10);
        assert_eq!(out.sources[1].damage, 30);
    }

    #[test]
    fn burns_and_mills() {
        let t = Turn {
            souls: [0; 3],
            sources: vec![
                Source::new(SourceKind::Burn, 3),
                Source::new(SourceKind::Mill, 5),
                Source::new(SourceKind::Burn, 2),
            ],
            ..Turn::default()
        };
        let out = simulate(&t, &opponent(30, 0), 10, 7);
        assert_eq!(out.histogram[5], 10);
        assert_eq!(out.sources[1].damage, 0);
    }

    #[test]
    fn cancelled_attacks_trigger_cancel_burns_and_shots() {
        let mut own = DeckState::default();
        own.add(TriggerKind::new(true, 1, true), 5);
        let t = Turn {
            souls: [0, 0, 0],
            ..one_attack(0, own, SourceKind::CancelBurn)
        };
        // The deck is a climax then nothing but non-climaxes: the attack is cancelled, the
        // shot and the effect stick.
        let mut opp = opponent(1, 1);
        opp.waiting_room_cards = 10;
        let out = simulate(&t, &opp, 10, 8);
        assert_eq!(out.sources[0].cancels, 10);
        assert_eq!(out.sources[1].happened, 10);
        // 1 refresh penalty + shot + cancel burn.
        assert_eq!(out.histogram[3], 10);
    }

    #[test]
    fn conditions_follow_the_last_attack() {
        let t = one_attack(1, deck(20, 0), SourceKind::NotCancelledBurn);
        assert_eq!(simulate(&t, &opponent(30, 0), 10, 9).histogram[2], 10);
        assert_eq!(simulate(&t, &opponent(10, 10), 10, 9).histogram, vec![10]);
        let t = one_attack(1, deck(20, 0), SourceKind::CancelBurn);
        let out = simulate(&t, &opponent(30, 0), 10, 9);
        assert_eq!((out.histogram[1], out.sources[1].happened), (10, 0));
        let t = one_attack(1, deck(0, 10), SourceKind::ClimaxTriggerBurn);
        assert_eq!(simulate(&t, &opponent(30, 0), 10, 9).histogram[2], 10);
    }

    #[test]
    fn refresh_adds_one_damage() {
        let mut t = turn([3, 0, 0], deck(20, 0));
        t.sources.truncate(1);
        let mut opp = opponent(2, 0);
        opp.waiting_room_cards = 10;
        let out = simulate(&t, &opp, 10, 9);
        assert_eq!(out.histogram[4], 10);
    }

    #[test]
    fn an_empty_deck_and_waiting_room_is_a_win() {
        let mut t = turn([5, 0, 0], deck(20, 0));
        t.sources.truncate(1);
        let out = simulate(&t, &opponent(2, 0), 10, 10);
        assert_eq!(out.kills, 10);
    }

    #[test]
    fn kills_count_reaching_level_four() {
        let t = turn([2, 2, 3], deck(20, 0));
        let mut opp = opponent(30, 0);
        opp.level = 3;
        assert_eq!(opp.damage_to_kill(), 7);
        assert_eq!(simulate(&t, &opp, 10, 11).kills, 10);
        opp.level = 2;
        assert_eq!(simulate(&t, &opp, 10, 11).kills, 0);
    }

    #[test]
    fn level_up_sends_the_clock_to_the_waiting_room() {
        let mut d = Defender::new(&opponent(10, 0));
        let mut rng = Rng::new(1);
        d.damage(7, &mut rng);
        assert_eq!((d.level, d.clock.cards, d.waiting_room.cards), (1, 0, 6));
    }

    #[test]
    fn the_default_opponent_just_refreshed() {
        let o = Opponent::default();
        assert_eq!(
            (o.deck_cards, o.deck_climaxes, o.stock, o.level),
            (39, 8, 8, 3)
        );
        assert_eq!((o.cards(), o.problem()), (50, None));
    }

    #[test]
    fn statistics() {
        let out = Outcome {
            trials: 4,
            histogram: vec![1, 0, 2, 1],
            ..Outcome::default()
        };
        assert_eq!(out.mean(), 1.75);
        assert_eq!(out.median(), 2);
        assert_eq!(out.chance_at_least(2), 0.75);
        assert_eq!(out.chance_of(0), 0.25);
    }

    #[test]
    fn deck_state_counts_trigger_kinds() {
        let cards = [
            card(CardType::Character, 1, vec![Trigger::Soul]),
            card(CardType::Character, 2, vec![Trigger::Soul, Trigger::Soul]),
            card(CardType::Character, 0, vec![]),
            card(CardType::Climax, 0, vec![Trigger::Choice]),
            card(CardType::Climax, 0, vec![Trigger::Burn, Trigger::Soul]),
        ];
        let state = DeckState::of(&cards);
        assert_eq!(state.counts, [1, 1, 1, 1, 0, 0, 1]);
        assert_eq!((state.total(), state.climaxes()), (5, 2));
        assert_eq!(Levels::of(&cards), Levels([3, 1, 1, 0]));
    }

    #[test]
    fn musashi_deals_the_level_plus_one() {
        let t = Turn {
            souls: [0; 3],
            sources: vec![Source::new(SourceKind::Musashi, 0); 2],
            musashi: Levels([0, 0, 10, 0]),
            ..Turn::default()
        };
        let out = simulate(&t, &opponent(30, 0), 100, 6);
        assert_eq!(out.histogram[6], 100);
    }

    #[test]
    fn musashi_damage_is_the_average_level_plus_one() {
        assert_eq!(Levels::default().musashi_damage(), None);
        assert_eq!(Levels([3, 0, 0, 1]).musashi_damage(), Some(1.75));
    }

    #[test]
    fn cancel_musashi_only_on_cancel() {
        let mut t = one_attack(1, deck(10, 0), SourceKind::CancelMusashi);
        t.musashi = Levels([0, 10, 0, 0]);
        let out = simulate(&t, &opponent(30, 0), 100, 8);
        assert_eq!(out.histogram[1], 100);
    }
}
