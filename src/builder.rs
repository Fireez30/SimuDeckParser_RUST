//! A deck being built: cards with their number of copies, and the deck construction rules.

use std::fmt;

use crate::filters::{DECK_ORDER, sort_cards};
use crate::model::{Card, CardType};

pub const DECK_SIZE: usize = 50;
/// Copies allowed of cards sharing a name.
pub const MAX_COPIES: usize = 4;
pub const MAX_CLIMAXES: usize = 8;

/// Section of a deck list while building: one per level, climaxes last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Level(u32),
    Climax,
}

impl Group {
    pub fn of(card: &Card) -> Self {
        match card.card_type {
            CardType::Climax => Group::Climax,
            _ => Group::Level(card.level),
        }
    }

    pub fn label(self) -> String {
        match self {
            Group::Level(n) => format!("Level {n}"),
            Group::Climax => "Climaxes".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddError {
    Full,
    TooManyCopies(String),
}

impl fmt::Display for AddError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddError::Full => write!(f, "The deck already has {DECK_SIZE} cards."),
            AddError::TooManyCopies(name) => {
                write!(
                    f,
                    "A deck can hold only {MAX_COPIES} cards named \"{name}\"."
                )
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeckList {
    entries: Vec<(Card, usize)>,
    /// Codes of an edited deck that match no loaded card; kept as they are when saving.
    pub unmatched: Vec<String>,
}

impl DeckList {
    pub fn new(cards: &[Card], unmatched: Vec<String>) -> Self {
        let mut list = Self {
            entries: Vec::new(),
            unmatched,
        };
        for card in cards {
            list.push(card);
        }
        list
    }

    fn push(&mut self, card: &Card) {
        match self.entries.iter_mut().find(|(c, _)| c.key == card.key) {
            Some((_, n)) => *n += 1,
            None => self.entries.push((card.clone(), 1)),
        }
    }

    pub fn total(&self) -> usize {
        self.entries.iter().map(|(_, n)| n).sum::<usize>() + self.unmatched.len()
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    pub fn copies(&self, key: &str) -> usize {
        self.entries
            .iter()
            .find(|(c, _)| c.key == key)
            .map_or(0, |(_, n)| *n)
    }

    /// Copies of every card named `name` (foils and reprints share the limit).
    pub fn copies_named(&self, name: &str) -> usize {
        self.entries
            .iter()
            .filter(|(c, _)| c.name == name)
            .map(|(_, n)| n)
            .sum()
    }

    pub fn count(&self, card_type: CardType) -> usize {
        self.entries
            .iter()
            .filter(|(c, _)| c.card_type == card_type)
            .map(|(_, n)| n)
            .sum()
    }

    pub fn add(&mut self, card: &Card) -> Result<(), AddError> {
        if self.total() >= DECK_SIZE {
            return Err(AddError::Full);
        }
        if !card.name.is_empty() && self.copies_named(&card.name) >= MAX_COPIES {
            return Err(AddError::TooManyCopies(card.name.clone()));
        }
        self.push(card);
        Ok(())
    }

    /// Removes one copy. Returns `false` when the card is not in the deck.
    pub fn remove(&mut self, key: &str) -> bool {
        let Some(i) = self.entries.iter().position(|(c, _)| c.key == key) else {
            return false;
        };
        self.entries[i].1 -= 1;
        if self.entries[i].1 == 0 {
            self.entries.remove(i);
        }
        true
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.unmatched.clear();
    }

    /// Distinct cards in deck order, with their copies.
    pub fn entries(&self) -> Vec<(Card, usize)> {
        let mut cards: Vec<Card> = self.entries.iter().map(|(c, _)| c.clone()).collect();
        sort_cards(&mut cards, DECK_ORDER);
        cards
            .into_iter()
            .map(|c| {
                let n = self.copies(&c.key);
                (c, n)
            })
            .collect()
    }

    /// Distinct cards grouped by level (characters and events together), climaxes last.
    /// Inside a group, cards keep the deck order (characters first, then by cost, power…).
    pub fn by_level(&self) -> Vec<(Group, Vec<(Card, usize)>)> {
        let mut entries = self.entries();
        // Stable: keeps the deck order inside each group.
        entries.sort_by_key(|(c, _)| Group::of(c));
        let mut groups: Vec<(Group, Vec<(Card, usize)>)> = Vec::new();
        for (card, n) in entries {
            let group = Group::of(&card);
            match groups.last_mut() {
                Some((g, cards)) if *g == group => cards.push((card, n)),
                _ => groups.push((group, vec![(card, n)])),
            }
        }
        groups
    }

    /// Every card code, one per copy, in deck order (unmatched codes last).
    pub fn codes(&self) -> Vec<String> {
        self.entries()
            .into_iter()
            .flat_map(|(c, n)| std::iter::repeat_n(c.key, n))
            .chain(self.unmatched.iter().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;

    fn card(key: &str, name: &str, card_type: CardType, level: u32) -> Card {
        Card {
            key: key.into(),
            card_type,
            name: name.into(),
            image: None,
            color: Color::Red,
            level,
            cost: 0,
            power: 0,
            triggers: vec![],
            soul_count: 1,
            traits: vec![],
            code: String::new(),
            text: String::new(),
        }
    }

    #[test]
    fn rules() {
        let aqua = card("KS/W49-001", "Aqua", CardType::Character, 1);
        let aqua_foil = card("KS/W49-001SP", "Aqua", CardType::Character, 1);
        let cx = card("KS/W49-099", "Splash", CardType::Climax, 0);
        let mut deck = DeckList::new(&[], vec!["XX/Y-001".into()]);

        for _ in 0..3 {
            deck.add(&aqua).unwrap();
        }
        deck.add(&aqua_foil).unwrap();
        // The foil shares the name, so a fifth "Aqua" is refused.
        assert_eq!(deck.add(&aqua), Err(AddError::TooManyCopies("Aqua".into())));
        assert_eq!(deck.copies("KS/W49-001"), 3);
        assert_eq!(deck.copies_named("Aqua"), 4);
        deck.add(&cx).unwrap();
        assert_eq!(deck.total(), 6);
        assert_eq!(deck.count(CardType::Climax), 1);
        assert_eq!(
            deck.codes(),
            [
                "KS/W49-001",
                "KS/W49-001",
                "KS/W49-001",
                "KS/W49-001SP",
                "KS/W49-099",
                "XX/Y-001"
            ]
        );

        assert!(deck.remove("KS/W49-001SP"));
        assert!(!deck.remove("KS/W49-001SP"));
        assert_eq!(deck.copies_named("Aqua"), 3);

        let ev = card("KS/W49-050", "Event", CardType::Event, 1);
        let l1 = card("KS/W49-010", "Megumin", CardType::Character, 1);
        let l0 = card("KS/W49-020", "Kazuma", CardType::Character, 0);
        let deck = DeckList::new(&[cx.clone(), ev, l1, l0.clone(), l0], vec![]);
        let by_level = deck.by_level();
        let groups: Vec<(String, Vec<&str>)> = by_level
            .iter()
            .map(|(g, cards)| {
                (
                    g.label(),
                    cards.iter().map(|(c, _)| c.key.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            groups,
            [
                ("Level 0".to_string(), vec!["KS/W49-020"]),
                ("Level 1".to_string(), vec!["KS/W49-010", "KS/W49-050"]),
                ("Climaxes".to_string(), vec!["KS/W49-099"]),
            ]
        );

        let mut full = DeckList::new(&vec![cx.clone(); DECK_SIZE], vec![]);
        assert_eq!(full.add(&aqua), Err(AddError::Full));
        full.clear();
        assert!(full.is_empty());
    }
}
