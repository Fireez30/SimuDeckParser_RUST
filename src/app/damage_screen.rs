//! The Damage screen: how much damage a turn deals, estimated by playing it many times
//! (`simu_deck_parser::damage_sim`).
//!
//! The decks saved in the simulator are on the left. With one picked, the page in the middle
//! sets up the turn: the three characters on stage with their soul (the climax's bonus
//! included), the climax (only shown, and out of the deck), the sources of damage in order
//! (attacks, burns, mills, Musashi), the trigger cards left in the player's deck, and the
//! opponent's deck. "Run" plays the turn in the background and shows the damage dealt:
//! average, chance to kill, and the chance of each total.

use std::path::{Path, PathBuf};

use egui::{Align2, CornerRadius, Margin, RichText, ScrollArea, Sense, Shape, Stroke, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ADD, ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_CALCULATE, ICON_CLOSE, ICON_INFO,
    ICON_PLAY_ARROW, ICON_REFRESH, ICON_WARNING,
};
use simu_deck_parser::card_index::CardIndex;
use simu_deck_parser::damage_sim::{
    AttackKind, CHARACTERS, DECK_CLIMAXES, DECK_SIZE, DeckState, Levels, Opponent, Outcome, Source,
    SourceKind, TriggerKind, Turn, random_seed, simulate,
};
use simu_deck_parser::filters::sort_for_deck;
use simu_deck_parser::model::{CARD_IMAGE_RATIO, Card, CardType, Deck, Serie};
use simu_deck_parser::simulator::deck_files::{self, DeckFile};

use super::Notice;
use super::assets;
use super::background::{Task, finished};
use super::card_view::DetailPane;
use super::theme::{self, scheme};

/// Width range of a stage slot's card: three of them share a row when they fit.
const SLOT_WIDTHS: std::ops::RangeInclusive<f32> = 200.0..=320.0;
/// Space between two cards of a row.
const GAP: f32 = 12.0;
/// Width of a slot's card picture.
const THUMB_WIDTH: f32 = 48.0;
/// The number of trials the user can pick from.
const TRIAL_CHOICES: [u32; 3] = [1_000, 10_000, 100_000];
const CHARACTER_NAMES: [&str; CHARACTERS] = ["Character 1", "Character 2", "Character 3"];

/// The deck the turn is set up from.
struct PickedDeck {
    path: PathBuf,
    deck: Deck,
    /// Each different character of the deck once, in deck order.
    characters: Vec<Card>,
    climaxes: Vec<Card>,
}

/// A character on stage: the card (optional: the soul can be typed in without one) and its
/// soul, every bonus included.
struct Slot {
    card: Option<Card>,
    soul: u32,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            card: None,
            soul: 1,
        }
    }
}

/// The inputs of the last run, to tell when the results no longer match the setup.
type Inputs = (Turn, Opponent, u32);

pub struct DamageScreen {
    decks: Vec<DeckFile>,
    /// The deck list's search field.
    query: String,
    picked: Option<PickedDeck>,
    characters: [Slot; CHARACTERS],
    /// Only shown, and out of the deck: its soul bonus is typed in the soul of the characters
    /// it gives it to (not always all of them).
    climax: Option<Card>,
    /// In the order they happen.
    sources: Vec<Source>,
    /// What is left in the player's deck, for the trigger checks.
    deck: DeckState,
    opponent: Opponent,
    trials: u32,
    running: Option<Task<Outcome>>,
    /// The inputs of the run in progress.
    running_inputs: Option<Inputs>,
    /// The last results, with the inputs they were computed from.
    outcome: Option<(Outcome, Inputs)>,
    /// New results arrived: scroll down to them.
    scroll_to_results: bool,
    /// The card under the mouse, shown on the right.
    pane: DetailPane,
    /// Message for the user, taken by the app.
    notice: Option<Notice>,
}

impl Default for DamageScreen {
    fn default() -> Self {
        Self {
            decks: Vec::new(),
            query: String::new(),
            picked: None,
            characters: Default::default(),
            climax: None,
            sources: (0..CHARACTERS).map(Source::attack).collect(),
            deck: DeckState::default(),
            opponent: Opponent::default(),
            trials: TRIAL_CHOICES[1],
            running: None,
            running_inputs: None,
            outcome: None,
            scroll_to_results: false,
            pane: DetailPane::default(),
            notice: None,
        }
    }
}

impl DamageScreen {
    /// Re-reads the deck files. The picked deck stays picked if its file still exists.
    pub fn reload(&mut self, decks_dir: &Path) -> anyhow::Result<()> {
        self.decks = deck_files::list_decks(decks_dir)?;
        if let Some(picked) = &self.picked
            && !self.decks.iter().any(|d| d.path == picked.path)
        {
            self.picked = None;
        }
        Ok(())
    }

    pub fn take_notice(&mut self) -> Option<Notice> {
        self.notice.take()
    }

    /// Opens the deck file at `path`, and starts a new setup from it. The opponent is kept:
    /// it does not depend on the player's deck.
    fn pick(&mut self, ctx: &egui::Context, path: &Path, series: &[Serie]) {
        ctx.forget_all_images();
        let Some(file) = self.decks.iter().find(|d| d.path == path) else {
            return;
        };
        let deck = CardIndex::new(series).resolve_deck(&file.name, &file.date, &file.codes);
        let path = file.path.clone();
        let mut unique = deck.cards.clone();
        sort_for_deck(&mut unique);
        unique.dedup_by(|a, b| a.key == b.key);
        let (climaxes, characters) = unique
            .into_iter()
            .filter(|c| c.card_type != CardType::Event)
            .partition(|c| c.card_type == CardType::Climax);
        *self = Self {
            decks: std::mem::take(&mut self.decks),
            query: std::mem::take(&mut self.query),
            opponent: self.opponent.clone(),
            trials: self.trials,
            picked: Some(PickedDeck {
                path,
                deck,
                characters,
                climaxes,
            }),
            ..Self::default()
        };
        self.reset_deck_state();
    }

    /// The cards of the deck that are not on the stage or in the climax area.
    fn off_board(&self) -> Vec<&Card> {
        let Some(picked) = &self.picked else {
            return Vec::new();
        };
        let mut on_board: Vec<&str> = self.in_play().map(|c| c.key.as_str()).collect();
        picked
            .deck
            .cards
            .iter()
            .filter(|c| match on_board.iter().position(|k| *k == c.key) {
                Some(i) => {
                    on_board.swap_remove(i);
                    false
                }
                None => true,
            })
            .collect()
    }

    /// The player's deck: the whole deck, without the cards on the stage and the climax.
    fn reset_deck_state(&mut self) {
        self.deck = DeckState::of(self.off_board());
    }

    /// The cards on the stage and in the climax area: out of the deck.
    fn in_play(&self) -> impl Iterator<Item = &Card> {
        self.characters
            .iter()
            .filter_map(|s| s.card.as_ref())
            .chain(&self.climax)
    }

    /// Puts `card` on the stage or in the climax area (`slot` `None`), in place of the card
    /// there: the old card goes back to the deck, the new one comes out of it.
    fn place(&mut self, slot: Option<usize>, card: Option<Card>) {
        let place = match slot {
            Some(i) => &mut self.characters[i].card,
            None => &mut self.climax,
        };
        if let Some(old) = place.take() {
            self.deck.add(TriggerKind::of(&old), 1);
        }
        if let Some(new) = &card {
            self.deck.add(TriggerKind::of(new), -1);
            if let Some(i) = slot {
                self.characters[i].soul = new.soul_count;
            }
        }
        match slot {
            Some(i) => self.characters[i].card = card,
            None => self.climax = card,
        }
    }

    /// How many cards and climaxes "Your deck" may hold, by the deck building rules: 50 cards
    /// and 8 climaxes, minus the ones in play.
    fn deck_limits(&self) -> (u32, u32) {
        let cards = self.in_play().count() as u32;
        let climaxes = u32::from(self.climax.is_some());
        (
            DECK_SIZE.saturating_sub(cards),
            DECK_CLIMAXES.saturating_sub(climaxes),
        )
    }

    /// The turn as set up on the page.
    fn turn(&self) -> Turn {
        Turn {
            souls: std::array::from_fn(|i| self.characters[i].soul),
            sources: self.sources.clone(),
            deck: self.deck.clone(),
            musashi: Levels::of(self.off_board()),
        }
    }

    /// Why the turn cannot be simulated, if it cannot.
    fn problem(&self) -> Option<&'static str> {
        if self.sources.is_empty() {
            return Some("Add a source of damage.");
        }
        let (max_cards, max_climaxes) = self.deck_limits();
        if self.deck.total() > max_cards {
            return Some("Your deck has more than 50 cards, counting the stage and the climax.");
        }
        if self.deck.climaxes() > max_climaxes {
            return Some("Your deck has more than 8 climaxes, counting the one placed.");
        }
        self.opponent.problem()
    }

    pub(super) fn run(&mut self, ctx: &egui::Context) {
        let inputs = self.inputs();
        let (turn, opponent, trials) = inputs.clone();
        self.running = Some(Task::spawn(ctx, move || {
            Ok(simulate(&turn, &opponent, trials, random_seed()))
        }));
        self.running_inputs = Some(inputs);
    }

    /// Puts the deck's first characters on the stage and its first climax in the climax area,
    /// with a restand and a burn (the screenshots).
    #[cfg(test)]
    pub(super) fn fill_stage(&mut self) {
        let Some(picked) = &self.picked else {
            return;
        };
        let characters = picked.characters.clone();
        let climax = picked.climaxes.first().cloned();
        for (i, card) in characters.into_iter().take(CHARACTERS).enumerate() {
            self.place(Some(i), Some(card));
            self.characters[i].soul += 1;
        }
        self.place(None, climax);
        self.sources[1].attack = AttackKind::Side;
        self.sources[1].defender_level = 1;
        self.sources.push(Source::new(SourceKind::CancelBurn, 1));
        self.sources.push(Source::attack(2));
    }

    #[cfg(test)]
    pub(super) fn has_results(&self) -> bool {
        self.outcome.is_some()
    }

    /// The setup as it would be run now.
    fn inputs(&self) -> Inputs {
        (self.turn(), self.opponent.clone(), self.trials)
    }

    pub fn show(&mut self, ui: &mut Ui, series: &[Serie], decks_dir: &Path) {
        match finished(&mut self.running) {
            Some(Ok(outcome)) => {
                let inputs = self.running_inputs.take().unwrap_or_else(|| self.inputs());
                self.outcome = Some((outcome, inputs));
                self.scroll_to_results = true;
            }
            Some(Err(e)) => self.notice = Some(Notice::from_error("The simulation failed", &e)),
            None => {}
        }

        let mut clicked = None;
        theme::list_pane(ui, "damage_deck_list", |ui| {
            clicked = self.deck_list(ui, decks_dir)
        });
        if let Some(path) = clicked {
            self.pick(ui.ctx(), &path, series);
        }
        if self.picked.is_some() {
            theme::detail_pane(ui, "damage_card_panel", 16, |ui| {
                self.pane.show(ui, None);
            });
        }
        let s = scheme(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                left: 12,
                right: 8,
                top: 16,
                bottom: 0,
            }))
            .show(ui, |ui| self.page(ui));
    }

    /// The list of deck files. Returns the deck file the user clicked.
    fn deck_list(&mut self, ui: &mut Ui, decks_dir: &Path) -> Option<PathBuf> {
        let s = scheme(ui);
        let mut clicked = None;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(theme::headline("Damage").color(s.on_surface));
        });
        ui.add_space(4.0);
        if self.decks.is_empty() {
            ui.add_space(24.0);
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "No decks yet.\nDecks saved in the simulator appear here.\n\n{}",
                        decks_dir.display()
                    ))
                    .color(s.on_surface_variant),
                )
                .wrap(),
            );
            return None;
        }
        if self.decks.len() > 8 {
            let width = ui.available_width() - 16.0;
            theme::search_bar(ui, &mut self.query, "Find a deck", width);
            ui.add_space(4.0);
        }
        let query = self.query.trim().to_lowercase();
        let picked_path = self.picked.as_ref().map(|p| &p.path);
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            // The user's decks, then the AI opponents' ones.
            for ai in [false, true] {
                let decks: Vec<&DeckFile> = self
                    .decks
                    .iter()
                    .filter(|d| d.ai == ai)
                    .filter(|d| query.is_empty() || d.name.to_lowercase().contains(&query))
                    .collect();
                if decks.is_empty() {
                    continue;
                }
                if ai {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(theme::label("AI opponents").color(s.on_surface_variant));
                    });
                    ui.add_space(2.0);
                }
                for deck in decks {
                    let selected = picked_path == Some(&deck.path);
                    let name = if ai {
                        deck.name.strip_prefix("AI_").unwrap_or(&deck.name)
                    } else {
                        &deck.name
                    };
                    let supporting = format!("{} cards", deck.codes.len());
                    if theme::list_item(ui, name, &supporting, selected).clicked() && !selected {
                        clicked = Some(deck.path.clone());
                    }
                }
            }
        });
        clicked
    }

    /// The setup of the turn, the Run button and the results.
    fn page(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        let Some(picked) = &self.picked else {
            theme::empty_state(
                ui,
                ICON_CALCULATE,
                "Pick a deck",
                "Choose a deck on the left to set up a turn and simulate its damage.",
            );
            return;
        };
        let mut hovered: Option<Card> = None;
        // (slot, card): `None` slot for the climax.
        let mut placed: Option<(Option<usize>, Option<Card>)> = None;
        let mut reset_deck = false;
        let mut run = false;
        // Worked out before drawing: the drawing code borrows the fields it edits.
        let problem = self.problem();
        let (max_cards, max_climaxes) = self.deck_limits();
        let musashi = Levels::of(self.off_board()).musashi_damage();
        let names: [String; CHARACTERS] = std::array::from_fn(|i| match &self.characters[i].card {
            Some(card) => format!("{} · {}", i + 1, card.name),
            None => CHARACTER_NAMES[i].to_string(),
        });
        let running = self.running.is_some();
        let stale = !running
            && self
                .outcome
                .as_ref()
                .is_some_and(|(_, i)| *i != self.inputs());
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            // Rounded down, with a margin: the wrapping layout starts a new row when the
            // last card overflows by a fraction of a pixel.
            let width = ((ui.available_width() - 2.0 * GAP) / 3.0 - 2.0)
                .floor()
                .clamp(*SLOT_WIDTHS.start(), *SLOT_WIDTHS.end());
            let full = 3.0 * width + 2.0 * GAP;
            ui.add(egui::Label::new(theme::headline(&picked.deck.name).color(s.on_surface)).wrap());
            ui.label(
                RichText::new(
                    "Set up your stage, the damage of the turn and both decks, then run the turn \
                     many times with shuffled decks.",
                )
                .color(s.on_surface_variant),
            );
            ui.add_space(12.0);

            theme::section_header(ui, "Stage", None);
            ui.label(
                theme::body_small(
                    "Set each character's soul with every bonus: the climax's, the backrow's…",
                )
                .color(s.on_surface_variant),
            );
            ui.add_space(4.0);
            slot_row(ui, |ui| {
                for (i, slot) in self.characters.iter_mut().enumerate() {
                    if let Some(card) =
                        character_card(ui, width, i, slot, &picked.characters, &mut hovered)
                    {
                        placed = Some((Some(i), card));
                    }
                }
            });
            ui.add_space(12.0);

            theme::section_header(ui, "Damage", None);
            ui.label(
                theme::body_small(
                    "In the order it happens. Conditions (\"if cancelled\"…) are about the last \
                     attack. A character attacking again keeps the soul of its earlier triggers.",
                )
                .color(s.on_surface_variant),
            );
            ui.add_space(4.0);
            sources_editor(ui, full, &mut self.sources, &names, musashi);
            ui.add_space(12.0);

            slot_row(ui, |ui| {
                if let Some(card) = climax_card(
                    ui,
                    width,
                    self.climax.as_ref(),
                    &picked.climaxes,
                    &mut hovered,
                ) {
                    placed = Some((None, card));
                }
                reset_deck = deck_card(ui, width, &mut self.deck, max_cards, max_climaxes);
                opponent_card(ui, width, &mut self.opponent);
            });
            ui.add_space(16.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let button = theme::filled(Some(ICON_PLAY_ARROW), "Run")
                    .enabled(problem.is_none() && !running)
                    .show(ui);
                run = button.clicked();
                ui.add_space(8.0);
                ui.label(theme::label("Trials").color(s.on_surface_variant));
                for n in TRIAL_CHOICES {
                    if theme::chip(&thousands(n), self.trials == n)
                        .show(ui)
                        .clicked()
                    {
                        self.trials = n;
                    }
                }
                if running {
                    ui.add_space(8.0);
                    ui.spinner();
                }
                if let Some(problem) = problem {
                    ui.add_space(8.0);
                    ui.label(ICON_WARNING.rich_text().size(18.0).color(s.error));
                    ui.label(RichText::new(problem).color(s.error));
                }
            });
            ui.add_space(16.0);

            if let Some((outcome, (turn, _, _))) = &self.outcome {
                if std::mem::take(&mut self.scroll_to_results) {
                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                }
                results(ui, outcome, &turn.sources, stale);
            }
            ui.add_space(24.0);
        });

        if let Some(card) = hovered {
            self.pane.hover(&card);
        }
        if let Some((slot, card)) = placed {
            self.place(slot, card);
        }
        if reset_deck {
            self.reset_deck_state();
        }
        if run {
            self.run(ui.ctx());
        }
    }
}

/// A row of slot cards, wrapping on narrow windows, aligned at the top.
fn slot_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let layout = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
    ui.with_layout(layout, |ui| {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        add(ui);
    });
}

/// A rounded card of the page's sections, `width` wide.
fn section_card(ui: &mut Ui, width: f32, title: &str, add: impl FnOnce(&mut Ui)) {
    let s = scheme(ui);
    ui.allocate_ui(vec2(width, 0.0), |ui| {
        theme::surface_card(s.surface_container_low)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(width - 24.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.label(theme::title_medium(title).color(s.on_surface));
                    add(ui);
                });
            });
    });
}

/// A card's picture, which shows the card on the right when hovered.
fn thumbnail(ui: &mut Ui, card: Option<&Card>, hovered: &mut Option<Card>) {
    let size = vec2(THUMB_WIDTH, THUMB_WIDTH * CARD_IMAGE_RATIO);
    let image = match card {
        Some(card) => assets::card_image(card),
        None => egui::Image::new(assets::EMPTY_CARD),
    };
    let r = ui.add(
        image
            .fit_to_exact_size(size)
            .corner_radius(CornerRadius::same(6))
            .sense(Sense::hover()),
    );
    if let Some(card) = card
        && r.hovered()
    {
        *hovered = Some(card.clone());
    }
}

/// Runs `add` with `width` of room for a truncating combo box, given the width to use.
/// egui truncates the text against the available width, then pads the frame around it:
/// without this, the box overflows its room by that padding.
fn combo_room<R>(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui, f32) -> R) -> R {
    let inner = width - 2.0 * ui.spacing().button_padding.x;
    let size = vec2(inner, ui.spacing().interact_size.y);
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| add(ui, inner))
        .inner
}

/// A truncating combo box `width` wide, to pick `value` from `options` (value, label).
fn combo<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: egui::Id,
    width: f32,
    value: &mut T,
    options: &[(T, &str)],
) -> egui::Response {
    let selected = options
        .iter()
        .find(|(v, _)| v == value)
        .map_or("", |(_, label)| label)
        .to_string();
    combo_room(ui, width, |ui, width| {
        egui::ComboBox::from_id_salt(id)
            .width(width)
            .truncate()
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (v, label) in options {
                    ui.selectable_value(value, *v, *label);
                }
            })
            .response
    })
}

/// A list of cards to pick one from, or none. Returns the pick, once made.
fn card_combo(
    ui: &mut Ui,
    id: egui::Id,
    cards: &[Card],
    current: Option<&Card>,
    none: &str,
    width: f32,
    hovered: &mut Option<Card>,
) -> Option<Option<Card>> {
    let mut picked = None;
    let selected = current.map_or(none.to_string(), |c| c.name.clone());
    combo_room(ui, width, |ui, width| {
        egui::ComboBox::from_id_salt(id)
            .width(width)
            .height(360.0)
            .truncate()
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), none).clicked() {
                    picked = Some(None);
                }
                for card in cards {
                    let is_current = current.is_some_and(|c| c.key == card.key);
                    let text = if card.card_type == CardType::Climax {
                        format!("{}  ·  {}", card.name, card.key)
                    } else {
                        format!(
                            "{}  ·  L{} · {} soul  ·  {}",
                            card.name, card.level, card.soul_count, card.key
                        )
                    };
                    let r = ui.selectable_label(is_current, text);
                    if r.hovered() {
                        *hovered = Some(card.clone());
                    }
                    if r.clicked() {
                        picked = Some(Some(card.clone()));
                    }
                }
            });
    });
    picked
}

/// A number field with a label in front of it.
fn number(ui: &mut Ui, label: &str, value: &mut u32, max: u32, tip: &str) {
    let s = scheme(ui);
    ui.label(theme::body_small(label).color(s.on_surface_variant))
        .on_hover_text(tip);
    ui.add(egui::DragValue::new(value).range(0..=max).speed(0.1))
        .on_hover_text(tip);
}

/// A character on stage: its card and soul. Returns the card picked, once picked.
fn character_card(
    ui: &mut Ui,
    width: f32,
    i: usize,
    slot: &mut Slot,
    characters: &[Card],
    hovered: &mut Option<Card>,
) -> Option<Option<Card>> {
    let mut pick = None;
    section_card(ui, width, CHARACTER_NAMES[i], |ui| {
        let full = ui.available_width();
        let id = egui::Id::new(("damage_character", i));
        let current = slot.card.as_ref();
        pick = card_combo(ui, id, characters, current, "Any character", full, hovered);
        ui.horizontal_top(|ui| {
            thumbnail(ui, slot.card.as_ref(), hovered);
            ui.add(
                egui::DragValue::new(&mut slot.soul)
                    .range(0..=20)
                    .speed(0.1)
                    .suffix(" soul"),
            )
            .on_hover_text("Its soul when it attacks, with every bonus (the climax's…)");
        });
    });
    pick
}

/// The climax slot: only shown, the card is out of the deck. Returns the climax picked, once
/// picked.
fn climax_card(
    ui: &mut Ui,
    width: f32,
    climax: Option<&Card>,
    climaxes: &[Card],
    hovered: &mut Option<Card>,
) -> Option<Option<Card>> {
    let mut pick = None;
    section_card(ui, width, "Climax", |ui| {
        let full = ui.available_width();
        let id = egui::Id::new("climax_card");
        pick = card_combo(ui, id, climaxes, climax, "No climax", full, hovered);
        ui.horizontal_top(|ui| {
            thumbnail(ui, climax, hovered);
            let s = scheme(ui);
            ui.add(
                egui::Label::new(
                    theme::body_small(
                        "Out of your deck. Add its soul bonus to the soul of the characters it gives it to.",
                    )
                    .color(s.on_surface_variant),
                )
                .wrap(),
            );
        });
    });
    pick
}

/// The sources of damage, in order: each one's kind and numbers, movable and removable; and
/// "Add". `names`: the characters' names; `musashi`: a Musashi's average damage.
fn sources_editor(
    ui: &mut Ui,
    width: f32,
    sources: &mut Vec<Source>,
    names: &[String; CHARACTERS],
    musashi: Option<f64>,
) {
    let s = scheme(ui);
    let kinds: Vec<(SourceKind, &str)> = SourceKind::ALL.iter().map(|k| (*k, k.label())).collect();
    let characters: Vec<(usize, &str)> = names.iter().map(String::as_str).enumerate().collect();
    let attacks: Vec<(AttackKind, &str)> =
        AttackKind::ALL.iter().map(|k| (*k, k.label())).collect();
    let last = sources.len().saturating_sub(1);
    // (from, to): a source moved; a source removed.
    let mut swap = None;
    let mut remove = None;
    ui.allocate_ui(vec2(width, 0.0), |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        for (i, source) in sources.iter_mut().enumerate() {
            let id = egui::Id::new(("damage_source", i));
            egui::Frame::new()
                .fill(s.surface_container_low)
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.set_width(width - 24.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.label(theme::label(format!("{}.", i + 1)).color(s.on_surface_variant));
                        combo(ui, id.with("kind"), 180.0, &mut source.kind, &kinds)
                            .on_hover_text(source.kind.description());
                        match source.kind {
                            SourceKind::Attack => {
                                combo(
                                    ui,
                                    id.with("character"),
                                    210.0,
                                    &mut source.character,
                                    &characters,
                                );
                                combo(ui, id.with("attack"), 120.0, &mut source.attack, &attacks)
                                    .on_hover_text(
                                        "Front attack, side attack (loses soul), direct attack \
                                         (+1 soul)",
                                    );
                                if source.attack == AttackKind::Side {
                                    number(
                                        ui,
                                        "Level in front",
                                        &mut source.defender_level,
                                        3,
                                        "A side attack loses as much soul as the level of the \
                                         character in front",
                                    );
                                }
                            }
                            SourceKind::Musashi | SourceKind::CancelMusashi => {
                                let damage = match musashi {
                                    Some(d) => format!("{d:.2} damage on average"),
                                    None => "No damage: your deck is empty".into(),
                                };
                                ui.label(theme::body_small(damage).color(s.on_surface_variant))
                                    .on_hover_text(
                                        "A card's level + 1, from your deck minus the stage and \
                                         the climax",
                                    );
                            }
                            kind => {
                                let unit = if kind == SourceKind::Mill {
                                    " cards"
                                } else {
                                    " damage"
                                };
                                ui.add(
                                    egui::DragValue::new(&mut source.amount)
                                        .range(0..=20)
                                        .speed(0.1)
                                        .suffix(unit),
                                );
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            if theme::small_icon_button(ui, ICON_CLOSE, "Remove").clicked() {
                                remove = Some(i);
                            }
                            if i < last {
                                if theme::small_icon_button(ui, ICON_ARROW_DOWNWARD, "Move down")
                                    .clicked()
                                {
                                    swap = Some((i, i + 1));
                                }
                            } else {
                                ui.add_space(32.0);
                            }
                            if i > 0
                                && theme::small_icon_button(ui, ICON_ARROW_UPWARD, "Move up")
                                    .clicked()
                            {
                                swap = Some((i, i - 1));
                            }
                        });
                    });
                });
        }
        ui.horizontal(|ui| {
            if theme::text(Some(ICON_ADD), "Add attack").show(ui).clicked() {
                // The next character in line.
                let attacks = sources
                    .iter()
                    .filter(|s| s.kind == SourceKind::Attack)
                    .count();
                sources.push(Source::attack(attacks % CHARACTERS));
            }
            if theme::text(Some(ICON_ADD), "Add burn").show(ui).clicked() {
                sources.push(Source::new(SourceKind::Burn, 1));
            }
        });
    });
    if let Some((a, b)) = swap {
        sources.swap(a, b);
    }
    if let Some(i) = remove {
        sources.remove(i);
    }
}

/// The player's deck: how many cards of each trigger kind are left, at most `max_cards` cards
/// and `max_climaxes` climaxes. Returns whether "Reset" was clicked.
fn deck_card(
    ui: &mut Ui,
    width: f32,
    deck: &mut DeckState,
    max_cards: u32,
    max_climaxes: u32,
) -> bool {
    let mut reset = false;
    section_card(ui, width, "Your deck", |ui| {
        let s = scheme(ui);
        ui.add(
            egui::Label::new(
                theme::body_small("Cards left, revealed by the trigger checks.")
                    .color(s.on_surface_variant),
            )
            .wrap(),
        );
        // Room left under the deck building rules: a field can grow by that much.
        let card_room = max_cards.saturating_sub(deck.total());
        let climax_room = card_room.min(max_climaxes.saturating_sub(deck.climaxes()));
        egui::Grid::new("damage_own_deck")
            .num_columns(2)
            .spacing(vec2(12.0, 6.0))
            .show(ui, |ui| {
                for (kind, count) in TriggerKind::ALL.iter().zip(&mut deck.counts) {
                    let what = if kind.climax { "Climax" } else { "Character" };
                    ui.label(
                        theme::body_small(format!("{what}, {}", kind.label().to_lowercase()))
                            .color(s.on_surface_variant),
                    );
                    let room = if kind.climax { climax_room } else { card_room };
                    let max = *count + room;
                    ui.add(egui::DragValue::new(count).range(0..=max).speed(0.1));
                    ui.end_row();
                }
            });
        ui.label(
            theme::label(format!(
                "{} / {max_cards} cards, {} / {max_climaxes} climaxes",
                deck.total(),
                deck.climaxes()
            ))
            .color(s.on_surface),
        )
        .on_hover_text("At most 50 cards and 8 climaxes, minus the stage and the climax");
        reset = theme::text(Some(ICON_REFRESH), "Reset")
            .show(ui)
            .on_hover_text("The whole deck, minus the cards on the stage and the climax")
            .clicked();
    });
    reset
}

/// The defending player: their deck, waiting room, stock, level and clock.
fn opponent_card(ui: &mut Ui, width: f32, opponent: &mut Opponent) {
    section_card(ui, width, "Opponent", |ui| {
        let s = scheme(ui);
        // Cards put in (or taken from) the stock, level or clock come from (or go back to)
        // the deck.
        let held = |o: &Opponent| o.stock + o.level + o.clock;
        let held_before = held(opponent);
        egui::Grid::new("damage_opponent")
            .num_columns(2)
            .spacing(vec2(16.0, 6.0))
            .show(ui, |ui| {
                // Room left under the deck building rules: a field can grow by that much.
                let card_room = DECK_SIZE.saturating_sub(opponent.cards());
                let climax_room = DECK_CLIMAXES.saturating_sub(opponent.climaxes());
                let o = &mut *opponent;
                // (label, value, own maximum, how it is limited, tip)
                let rows: [(&str, &mut u32, u32, Room, &str); 7] = [
                    (
                        "Cards in deck",
                        &mut o.deck_cards,
                        DECK_SIZE,
                        Room::Cards,
                        "Cards left in the opponent's deck",
                    ),
                    (
                        "Climaxes in deck",
                        &mut o.deck_climaxes,
                        DECK_CLIMAXES,
                        Room::Climaxes,
                        "Climaxes among them",
                    ),
                    (
                        "Waiting room",
                        &mut o.waiting_room_cards,
                        DECK_SIZE,
                        Room::Cards,
                        "Cards in the waiting room: shuffled back into the deck when it runs out",
                    ),
                    (
                        "Climaxes in waiting room",
                        &mut o.waiting_room_climaxes,
                        DECK_CLIMAXES,
                        Room::Climaxes,
                        "Climaxes among them",
                    ),
                    (
                        "Stock",
                        &mut o.stock,
                        DECK_SIZE,
                        Room::FromDeck,
                        "Cards in the opponent's stock, none of them a climax",
                    ),
                    (
                        "Level",
                        &mut o.level,
                        3,
                        Room::FromDeck,
                        "The opponent's level",
                    ),
                    (
                        "Clock",
                        &mut o.clock,
                        6,
                        Room::FromDeck,
                        "Cards in the opponent's clock",
                    ),
                ];
                for (label, value, max, room, tip) in rows {
                    let max = match room {
                        Room::Cards => max.min(*value + card_room),
                        Room::Climaxes => max.min(*value + climax_room),
                        Room::FromDeck => max,
                    };
                    number(ui, label, value, max.max(*value), tip);
                    ui.end_row();
                }
            });
        let held_after = held(opponent);
        if held_after != held_before {
            let o = &mut *opponent;
            o.deck_cards = (o.deck_cards + held_before).saturating_sub(held_after);
            o.deck_climaxes = o.deck_climaxes.min(o.deck_cards);
        }
        ui.label(
            theme::label(format!(
                "{} / 50 cards, {} / 8 climaxes",
                opponent.cards(),
                opponent.climaxes()
            ))
            .color(s.on_surface),
        )
        .on_hover_text(
            "Deck, waiting room, stock, clock and level: at most 50 cards and 8 climaxes",
        );
        ui.label(
            theme::label(format!("{} damage to kill", opponent.damage_to_kill()))
                .color(s.on_surface),
        );
        if theme::text(Some(ICON_REFRESH), "Refreshed deck")
            .show(ui)
            .on_hover_text(
                "Every card not in the stock, level or clock in the deck, with all 8 climaxes",
            )
            .clicked()
        {
            opponent.refresh();
        }
    });
}

/// What limits a field of the opponent's card.
#[derive(Clone, Copy)]
enum Room {
    /// The cards left under 50.
    Cards,
    /// The climaxes left under 8.
    Climaxes,
    /// Nothing: the cards come from the deck.
    FromDeck,
}

/// "10 000"
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn percent(p: f64) -> String {
    format!("{:.1}%", p * 100.0)
}

/// The results of a run: headline numbers, the damage chart and each source's share.
/// `sources`: the turn's, as run. `stale`: the setup changed since.
fn results(ui: &mut Ui, outcome: &Outcome, sources: &[Source], stale: bool) {
    let s = scheme(ui);
    theme::section_header(ui, "Results", None);
    ui.label(
        theme::body_small(format!("{} turns played", thousands(outcome.trials)))
            .color(s.on_surface_variant),
    );
    if stale {
        ui.horizontal(|ui| {
            ui.label(ICON_INFO.rich_text().size(16.0).color(s.on_surface_variant));
            ui.label(
                theme::body_small("The setup changed since this run: run it again to update.")
                    .color(s.on_surface_variant),
            );
        });
    }
    ui.add_space(8.0);

    // Four tiles to a row when they fit.
    let width = ((ui.available_width() - 3.0 * GAP) / 4.0).clamp(150.0, 220.0);
    slot_row(ui, |ui| {
        let tile = |ui: &mut Ui, label: &str, value: String, supporting: String| {
            ui.allocate_ui(vec2(width, 104.0), |ui| {
                theme::surface_card(s.surface_container_low)
                    .inner_margin(Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_width(width - 32.0);
                        ui.set_height(104.0 - 24.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            ui.label(theme::label(label).color(s.on_surface_variant));
                            ui.label(theme::headline(value).color(s.on_surface));
                            ui.label(theme::body_small(supporting).color(s.on_surface_variant));
                        });
                    });
            });
        };
        tile(
            ui,
            "Average damage",
            format!("{:.2}", outcome.mean()),
            format!("Median {}", outcome.median()),
        );
        tile(
            ui,
            "Kill chance",
            percent(outcome.kill_chance()),
            format!("{} damage to kill", outcome.damage_to_kill),
        );
        let max = outcome.histogram.len().saturating_sub(1);
        let likeliest = (0..=max)
            .max_by(|&a, &b| outcome.chance_of(a).total_cmp(&outcome.chance_of(b)))
            .unwrap_or(0);
        tile(
            ui,
            "Most likely",
            format!("{likeliest} damage"),
            format!("{} of the turns", percent(outcome.chance_of(likeliest))),
        );
        tile(
            ui,
            "Range",
            format!(
                "{} – {max}",
                outcome.histogram.iter().position(|&n| n > 0).unwrap_or(0)
            ),
            "Lowest to highest".into(),
        );
    });
    ui.add_space(16.0);

    theme::section_header(ui, "Damage dealt", None);
    ui.label(
        theme::body_small("Chance of each total. Hover a bar for the chance of at least as much.")
            .color(s.on_surface_variant),
    );
    ui.add_space(4.0);
    histogram(ui, outcome);
    ui.add_space(16.0);

    theme::section_header(ui, "By source", None);
    ui.add_space(4.0);
    egui::Grid::new("damage_by_source")
        .num_columns(4)
        .spacing(vec2(32.0, 6.0))
        .show(ui, |ui| {
            for head in ["", "Average damage", "Happened", "Cancelled"] {
                ui.label(theme::label(head).color(s.on_surface_variant));
            }
            ui.end_row();
            let trials = f64::from(outcome.trials.max(1));
            for (i, (source, stats)) in sources.iter().zip(&outcome.sources).enumerate() {
                let name = match source.kind {
                    SourceKind::Attack => format!(
                        "{}. {} attack, character {}",
                        i + 1,
                        source.attack.label(),
                        source.character + 1
                    ),
                    kind if kind.has_amount() => {
                        format!("{}. {} {}", i + 1, kind.label(), source.amount)
                    }
                    kind => format!("{}. {}", i + 1, kind.label()),
                };
                ui.label(theme::body_small(name).color(s.on_surface));
                ui.label(format!("{:.2}", stats.damage as f64 / trials));
                ui.label(percent(f64::from(stats.happened) / trials));
                if source.kind == SourceKind::Attack {
                    ui.label(percent(
                        f64::from(stats.cancels) / f64::from(stats.happened.max(1)),
                    ));
                } else {
                    ui.label("–");
                }
                ui.end_row();
            }
        });
}

/// One bar per total damage, its height the chance of exactly that total, with a dashed line
/// at the damage that kills.
fn histogram(ui: &mut Ui, outcome: &Outcome) {
    let s = scheme(ui);
    let bars = outcome.histogram.len().max(1);
    let slot = (ui.available_width() / bars as f32).clamp(20.0, 48.0);
    let (rect, _) = ui.allocate_exact_size(vec2(slot * bars as f32, 200.0), Sense::hover());
    let painter = ui.painter_at(rect.expand(2.0));
    let baseline = rect.bottom() - 18.0;
    let chart_height = rect.height() - 40.0;
    let highest = (0..bars)
        .map(|n| outcome.chance_of(n))
        .fold(0.0, f64::max)
        .max(f64::EPSILON);
    for n in 0..bars {
        let chance = outcome.chance_of(n);
        let x = rect.left() + n as f32 * slot;
        let hit = egui::Rect::from_min_max(
            egui::pos2(x, rect.top()),
            egui::pos2(x + slot, rect.bottom()),
        );
        let r = ui.interact(hit, ui.id().with(("damage_bar", n)), Sense::hover());
        if r.hovered() {
            painter.rect_filled(hit, CornerRadius::same(4), s.surface_container_high);
        }
        let height = (chart_height as f64 * chance / highest) as f32;
        let bar = egui::Rect::from_min_max(
            egui::pos2(x + 3.0, baseline - height),
            egui::pos2(x + slot - 3.0, baseline),
        );
        if chance > 0.0 {
            painter.rect_filled(
                bar,
                CornerRadius {
                    nw: 4,
                    ne: 4,
                    sw: 0,
                    se: 0,
                },
                s.primary,
            );
        }
        // Labels only on the bars that matter, the rest on hover. The hovered bar shows its
        // exact chance.
        if r.hovered() {
            painter.text(
                egui::pos2(bar.center().x, bar.top() - 2.0),
                Align2::CENTER_BOTTOM,
                format!("{:.2}%", chance * 100.0),
                theme::medium(11.0),
                s.on_surface,
            );
        } else if chance >= 0.02 && slot >= 32.0 {
            painter.text(
                egui::pos2(bar.center().x, bar.top() - 2.0),
                Align2::CENTER_BOTTOM,
                format!("{:.0}%", chance * 100.0),
                theme::medium(11.0),
                s.on_surface,
            );
        }
        painter.text(
            egui::pos2(x + slot / 2.0, baseline + 3.0),
            Align2::CENTER_TOP,
            n.to_string(),
            theme::regular(11.0),
            s.on_surface_variant,
        );
        r.on_hover_text(format!(
            "{n} damage: {:.2}% of the turns\n{n} or more: {:.2}%",
            chance * 100.0,
            outcome.chance_at_least(n) * 100.0,
        ));
    }
    painter.line_segment(
        [
            egui::pos2(rect.left(), baseline),
            egui::pos2(rect.right(), baseline),
        ],
        Stroke::new(1.0, s.outline_variant),
    );
    let kill = outcome.damage_to_kill as usize;
    if kill < bars {
        let x = rect.left() + kill as f32 * slot;
        painter.add(Shape::dashed_line(
            &[egui::pos2(x, rect.top() + 14.0), egui::pos2(x, baseline)],
            Stroke::new(1.5, s.on_surface_variant),
            4.0,
            3.0,
        ));
        painter.text(
            egui::pos2(x + 4.0, rect.top()),
            Align2::LEFT_TOP,
            format!("Kill ({kill}+): {}", percent(outcome.kill_chance())),
            theme::medium(11.0),
            s.on_surface_variant,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_spaced() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1 000");
        assert_eq!(thousands(100_000), "100 000");
    }
}
