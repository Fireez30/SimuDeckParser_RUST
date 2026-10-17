//! The deck editor: the deck being built or edited, in a panel next to the card browser.
//!
//! The editor implements the browser's [`Picker`] interface, so clicking a card in the browser
//! adds it here. Saving is done by the Decks screen, which owns the deck files.

use std::path::PathBuf;

use egui::{CornerRadius, Margin, RichText, ScrollArea, Sense, Stroke, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ADD, ICON_DELETE_SWEEP, ICON_GRID_VIEW, ICON_REMOVE, ICON_SAVE, ICON_STACKS,
    ICON_VIEW_LIST, ICON_WARNING,
};
use simu_deck_parser::deck_builder::{DECK_SIZE, DeckList, MAX_CLIMAXES};
use simu_deck_parser::model::{CARD_IMAGE_RATIO, Card, CardType};
use simu_deck_parser::simulator::sleeves::{Sleeve, list_sleeves};

use super::assets;
use super::card_browser::Picker;
use super::sleeve_picker::{SleeveAdder, sleeve_combo};
use super::theme::{self, scheme};

/// What the user asked of the editor; the Decks screen carries it out.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    Save,
    Cancel,
}

pub struct DeckEditor {
    pub name: String,
    /// Deck file being edited; `None` for a new deck.
    pub target: Option<PathBuf>,
    /// Sleeve image of the deck: a file name of the simulator's `Sleeves` folder.
    pub sleeve: String,
    /// Sleeves the simulator offers, for the picker.
    sleeves: Vec<Sleeve>,
    /// The simulator's `Sleeves` folder, read again when a sleeve is added.
    sleeves_dir: PathBuf,
    sleeve_adder: SleeveAdder,
    pub list: DeckList,
    /// Names of the other decks, to warn about duplicates.
    taken_names: Vec<String>,
    dirty: bool,
    /// Why the last card could not be added.
    refusal: Option<String>,
    /// Save or Cancel was clicked; taken by the Decks screen.
    action: Option<EditorAction>,
    /// The save failed; shown in the panel.
    pub error: Option<String>,
    /// Put the cursor in the name field on the first frame (new, unnamed decks).
    focus_name: bool,
}

impl DeckEditor {
    pub fn new(
        name: String,
        target: Option<PathBuf>,
        sleeve: String,
        sleeves_dir: PathBuf,
        list: DeckList,
        taken_names: Vec<String>,
    ) -> Self {
        Self {
            focus_name: name.is_empty(),
            name,
            target,
            sleeve,
            sleeves: list_sleeves(&sleeves_dir),
            sleeve_adder: SleeveAdder::new(sleeves_dir.clone()),
            sleeves_dir,
            list,
            taken_names,
            dirty: false,
            refusal: None,
            action: None,
            error: None,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn take_action(&mut self) -> Option<EditorAction> {
        self.action.take()
    }

    pub fn title(&self) -> &'static str {
        if self.target.is_some() {
            "Edit deck"
        } else {
            "New deck"
        }
    }

    fn name_taken(&self) -> bool {
        let name = self.name.trim();
        self.taken_names
            .iter()
            .any(|n| n.eq_ignore_ascii_case(name))
    }

    /// Sleeve of the deck: a thumbnail and a combo box with the simulator's sleeves.
    fn sleeve_picker(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ui.add_space(6.0);
        let mut picked = None;
        ui.horizontal(|ui| {
            ui.label(theme::label("Sleeve").color(s.on_surface_variant));
            let current = self
                .sleeves
                .iter()
                .find(|sl| sl.name.eq_ignore_ascii_case(self.sleeve.trim()));
            if let Some(sleeve) = current {
                ui.add(
                    assets::image_from_path(&sleeve.path)
                        .fit_to_exact_size(vec2(18.0, 18.0 * CARD_IMAGE_RATIO))
                        .corner_radius(CornerRadius::same(3)),
                );
            }
            sleeve_combo(
                ui,
                egui::Id::new("deck_sleeve"),
                ui.available_width(),
                &self.sleeves,
                &self.sleeve,
                &mut picked,
                &mut self.sleeve_adder,
            );
        });
        if let Some(sleeve) = picked {
            // The pick may be a sleeve just added to the folder.
            self.sleeves = list_sleeves(&self.sleeves_dir);
            self.sleeve = sleeve;
            self.dirty = true;
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ui.label(theme::label(self.title()).color(s.on_surface_variant));
        let edit = egui::Frame::new()
            .stroke(Stroke::new(1.0, s.outline))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 2))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .hint_text("Deck name")
                        .font(theme::regular(18.0))
                        .frame(egui::Frame::NONE)
                        .background_color(egui::Color32::TRANSPARENT)
                        .margin(Margin::symmetric(0, 8))
                        .desired_width(f32::INFINITY),
                )
            })
            .inner;
        if std::mem::take(&mut self.focus_name) {
            edit.request_focus();
        }
        if edit.changed() {
            self.dirty = true;
            self.error = None;
        }
        if self.name.trim().is_empty() {
            ui.label(
                theme::body_small("Give the deck a name to save it.").color(s.on_surface_variant),
            );
        } else if self.name_taken() {
            let text = if self.target.is_some() {
                "Another deck already has this name.".to_string()
            } else {
                format!(
                    "A deck with this name exists: it will be saved as \"{} (2)\".",
                    self.name.trim()
                )
            };
            ui.label(theme::body_small(text).color(s.error));
        }
    }

    fn summary(&self, ui: &mut Ui) {
        let s = scheme(ui);
        let total = self.list.total();
        ui.horizontal(|ui| {
            ui.label(theme::headline(format!("{total}")).color(s.on_surface));
            ui.label(theme::title(format!("/ {DECK_SIZE}")).color(s.on_surface_variant));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let climaxes = self.list.count(CardType::Climax);
                let climax_color = if climaxes > MAX_CLIMAXES {
                    s.error
                } else {
                    s.on_surface_variant
                };
                ui.label(
                    theme::body_small(format!("CX {climaxes}/{MAX_CLIMAXES}")).color(climax_color),
                );
                ui.label(
                    theme::body_small(format!(
                        "Char {} · Event {} ·",
                        self.list.count(CardType::Character),
                        self.list.count(CardType::Event)
                    ))
                    .color(s.on_surface_variant),
                );
            });
        });
        // Progress towards 50 cards.
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(3), s.surface_container_highest);
        let done = (total as f32 / DECK_SIZE as f32).min(1.0);
        if done > 0.0 {
            let mut filled = rect;
            filled.set_width(rect.width() * done);
            let color = if total == DECK_SIZE {
                s.success
            } else {
                s.primary
            };
            painter.rect_filled(filled, CornerRadius::same(3), color);
        }
    }

    /// The deck's cards by level (climaxes last), as rows or as a grid of pictures. Returns
    /// the card under the pointer, for the detail pane.
    fn deck_cards(&mut self, ui: &mut Ui, grid: bool) -> Option<Card> {
        let mut add = None;
        let mut remove = None;
        let mut hovered = None;
        for (group, cards) in self.list.by_level() {
            ui.add_space(6.0);
            let count = cards.iter().map(|(_, n)| n).sum();
            theme::section_header(ui, &group.label(), Some(count));
            if grid {
                card_grid(ui, &cards, &mut add, &mut remove, &mut hovered);
            } else {
                for (card, copies) in &cards {
                    card_row(ui, card, *copies, &mut add, &mut remove, &mut hovered);
                }
            }
        }
        if !self.list.unmatched.is_empty() {
            ui.add_space(8.0);
            theme::banner(
                ui,
                ICON_WARNING,
                &format!(
                    "{} code(s) match no loaded card and are kept as they are: {}",
                    self.list.unmatched.len(),
                    self.list.unmatched.join(", ")
                ),
            );
        }
        if let Some(card) = add {
            Picker::add(self, &card);
        }
        if let Some(card) = remove {
            Picker::remove(self, &card);
        }
        hovered
    }
}

/// Thumbnail, name, code and level/cost, and −/+ buttons.
fn card_row(
    ui: &mut Ui,
    card: &Card,
    copies: usize,
    add: &mut Option<Card>,
    remove: &mut Option<Card>,
    hovered: &mut Option<Card>,
) {
    let s = scheme(ui);
    let row = ui.horizontal(|ui| {
        ui.set_height(48.0);
        ui.add(
            assets::card_image(card)
                .fit_to_exact_size(vec2(34.0, 34.0 * CARD_IMAGE_RATIO))
                .corner_radius(CornerRadius::same(4)),
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add(
                egui::Label::new(
                    RichText::new(&card.name)
                        .font(theme::medium(13.0))
                        .color(s.on_surface),
                )
                .truncate(),
            );
            let stats = match card.card_type {
                CardType::Climax => card.key.clone(),
                t => format!("{} · {} · C{}", card.key, t.label(), card.cost),
            };
            ui.label(theme::body_small(stats).color(s.on_surface_variant));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::small_icon_button(ui, ICON_ADD, "Add a copy").clicked() {
                *add = Some(card.clone());
            }
            ui.label(theme::title_medium(copies.to_string()).color(s.on_surface));
            if theme::small_icon_button(ui, ICON_REMOVE, "Remove a copy").clicked() {
                *remove = Some(card.clone());
            }
        });
    });
    if row.response.hovered() {
        *hovered = Some(card.clone());
    }
}

/// Pictures with their copies: click adds a copy, right-click removes one.
fn card_grid(
    ui: &mut Ui,
    cards: &[(Card, usize)],
    add: &mut Option<Card>,
    remove: &mut Option<Card>,
    hovered: &mut Option<Card>,
) {
    const GAP: f32 = 6.0;
    let width = ui.available_width();
    let columns = ((width + GAP) / (72.0 + GAP)).floor().max(3.0);
    let thumb_width = (width - GAP * (columns - 1.0)) / columns - 0.5;
    let thumb = vec2(thumb_width, thumb_width * CARD_IMAGE_RATIO);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        for (card, copies) in cards {
            let r = ui.add(
                assets::card_image(card)
                    .fit_to_exact_size(thumb)
                    .corner_radius(CornerRadius::same(6))
                    .sense(Sense::click()),
            );
            if r.hovered() {
                ui.painter().rect_stroke(
                    r.rect,
                    CornerRadius::same(6),
                    Stroke::new(2.0, scheme(ui).outline),
                    egui::StrokeKind::Outside,
                );
            }
            theme::count_badge(ui, r.rect, *copies);
            let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
            if r.hovered() {
                *hovered = Some(card.clone());
            }
            if r.clicked() {
                *add = Some(card.clone());
            }
            if r.secondary_clicked() {
                *remove = Some(card.clone());
            }
        }
    });
}

impl Picker for DeckEditor {
    fn copies(&self, card: &Card) -> usize {
        self.list.copies(&card.key)
    }

    fn add(&mut self, card: &Card) {
        match self.list.add(card) {
            Ok(()) => {
                self.dirty = true;
                self.refusal = None;
            }
            Err(e) => self.refusal = Some(e.to_string()),
        }
    }

    fn remove(&mut self, card: &Card) {
        if self.list.remove(&card.key) {
            self.dirty = true;
            self.refusal = None;
        }
    }

    fn side_panel(&mut self, ui: &mut Ui) -> Option<Card> {
        let s = scheme(ui);
        self.header(ui);
        self.sleeve_picker(ui);
        ui.add_space(8.0);
        self.summary(ui);
        if let Some(refusal) = &self.refusal {
            ui.add_space(4.0);
            ui.label(theme::body_small(refusal).color(s.error));
        }
        ui.add_space(4.0);

        // List / grid switch, remembered for the session.
        let grid_id = egui::Id::new("deck_editor_grid");
        let mut grid = ui.data(|d| d.get_temp::<bool>(grid_id)).unwrap_or(false);
        ui.horizontal(|ui| {
            let distinct = self.list.entries().len();
            ui.label(
                theme::label(format!("{distinct} different cards")).color(s.on_surface_variant),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button(ICON_GRID_VIEW, "Deck as a grid")
                    .selected(grid)
                    .show(ui)
                    .clicked()
                {
                    grid = true;
                }
                if theme::icon_button(ICON_VIEW_LIST, "Deck as a list")
                    .selected(!grid)
                    .show(ui)
                    .clicked()
                {
                    grid = false;
                }
            });
        });
        ui.data_mut(|d| d.insert_temp(grid_id, grid));

        // Actions stay at the bottom; the card list scrolls above them.
        let actions_height = 56.0 + if self.error.is_some() { 40.0 } else { 0.0 };
        let list_height = (ui.available_height() - actions_height).max(80.0);
        let mut hovered = None;
        ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(list_height)
            .show(ui, |ui| {
                if self.list.is_empty() {
                    ui.add_space(24.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            ICON_STACKS
                                .outlined()
                                .rich_text()
                                .size(40.0)
                                .color(s.outline),
                        );
                        ui.label(
                            RichText::new("Click cards on the left to add them.")
                                .color(s.on_surface_variant),
                        );
                    });
                } else {
                    hovered = self.deck_cards(ui, grid);
                }
            });

        if let Some(error) = &self.error {
            theme::banner(ui, ICON_WARNING, error);
        }
        ui.add_space(8.0);
        theme::actions_row(ui, |ui| {
            let can_save = !self.name.trim().is_empty() && !self.list.is_empty();
            let save = theme::filled(Some(ICON_SAVE), "Save")
                .enabled(can_save)
                .show(ui);
            let save = if self.list.total() != DECK_SIZE && can_save {
                save.on_hover_text(format!(
                    "The deck has {} cards; the simulator plays {DECK_SIZE}-card decks.",
                    self.list.total()
                ))
            } else {
                save
            };
            if save.clicked() {
                self.action = Some(EditorAction::Save);
            }
            if theme::text(None, "Cancel").show(ui).clicked() {
                self.action = Some(EditorAction::Cancel);
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let clear = theme::small_icon_button(ui, ICON_DELETE_SWEEP, "Remove every card");
                if clear.clicked() && !self.list.is_empty() {
                    self.list.clear();
                    self.dirty = true;
                }
            });
        });
        hovered
    }
}
