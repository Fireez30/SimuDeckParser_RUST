//! Deck builder: the deck being built, shown next to the card browser.

use std::path::PathBuf;

use egui::{CornerRadius, Margin, RichText, ScrollArea, Sense, Stroke, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ADD, ICON_DELETE_SWEEP, ICON_GRID_VIEW, ICON_REMOVE, ICON_SAVE, ICON_STACKS,
    ICON_VIEW_LIST, ICON_WARNING,
};
use simu_deck_parser::builder::{DECK_SIZE, DeckList, MAX_CLIMAXES};
use simu_deck_parser::decks::Sleeve;
use simu_deck_parser::model::{Card, CardType};

use super::card_list::Picker;
use super::card_view::CARD_RATIO;
use super::theme::{self, scheme};
use crate::assets;

/// Width the sleeve picker wraps its thumbnails at.
const SLEEVE_GRID_WIDTH: f32 = 480.0;
/// Columns of thumbnails in the sleeve picker.
const SLEEVE_GRID_COLUMNS: usize = 5;
/// Height the sleeve popup keeps when nothing is filtered.
const SLEEVE_POPUP_HEIGHT: f32 = 440.0;

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
    pub list: DeckList,
    /// Names of the other decks, to warn about duplicates.
    taken_names: Vec<String>,
    dirty: bool,
    /// Why the last card could not be added.
    refusal: Option<String>,
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
        sleeves: Vec<Sleeve>,
        list: DeckList,
        taken_names: Vec<String>,
    ) -> Self {
        Self {
            focus_name: name.is_empty(),
            name,
            target,
            sleeve,
            sleeves,
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
                        .fit_to_exact_size(vec2(18.0, 18.0 * CARD_RATIO))
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
            );
        });
        if let Some(sleeve) = picked {
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

/// The sleeves of the simulator as a picker: a name filter, then "No sleeve", then a
/// fixed-height grid of thumbnails the user scrolls. Sets `picked` to the chosen sleeve
/// file name, empty to have none. The filter text lives in egui memory at `id` until the
/// popup closes.
fn sleeve_grid(
    ui: &mut Ui,
    id: egui::Id,
    sleeves: &[Sleeve],
    current: &str,
    picked: &mut Option<String>,
) {
    // A width of its own, so the popup does not size itself to the (narrow) button.
    ui.set_width(SLEEVE_GRID_WIDTH);

    let query_id = id.with("sleeve_query");
    let mut query = ui.data(|d| d.get_temp::<String>(query_id)).unwrap_or_default();
    let edit = ui.add(
        egui::TextEdit::singleline(&mut query)
            .hint_text("Find a sleeve")
            .desired_width(f32::INFINITY),
    );
    if !edit.has_focus() && query.is_empty() {
        edit.request_focus();
    }
    ui.data_mut(|d| d.insert_temp(query_id, query.clone()));

    if ui
        .selectable_label(current.trim().is_empty(), "No sleeve")
        .on_hover_text("The deck is played without a sleeve")
        .clicked()
    {
        *picked = Some(String::new());
        ui.close();
    }
    ui.separator();

    let needle = query.trim().to_lowercase();
    let shown: Vec<&Sleeve> = sleeves
        .iter()
        .filter(|sl| needle.is_empty() || sl.name.to_lowercase().contains(&needle))
        .collect();
    // A fixed region for the grid: a plain scroll area would fill all the height the
    // popup still has, and the popup would grow to the whole screen while the grid
    // stayed small. Fixed, the popup is always the same, healthy size.
    ui.allocate_ui(vec2(ui.available_width(), SLEEVE_POPUP_HEIGHT), |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            if shown.is_empty() {
                ui.label(
                    RichText::new(format!("No sleeve matches \"{}\"", query.trim()))
                        .color(scheme(ui).on_surface_variant),
                );
            } else {
                // Rows of fixed columns: `horizontal_wrapped` does not wrap inside a popup.
                egui::Grid::new(id.with("sleeve_grid"))
                    .num_columns(SLEEVE_GRID_COLUMNS)
                    .spacing(vec2(8.0, 8.0))
                    .show(ui, |ui| {
                        for (i, sleeve) in shown.iter().enumerate() {
                            let selected = sleeve.name.eq_ignore_ascii_case(current.trim());
                            sleeve_item(ui, sleeve, selected, picked);
                            if (i + 1) % SLEEVE_GRID_COLUMNS == 0 {
                                ui.end_row();
                            }
                        }
                    });
            }
        });
    });
}

/// A combo box for the deck's sleeve: a button that opens the popup with
/// [`sleeve_grid`]. Sets `picked` to the chosen sleeve file name, empty for none.
///
/// Not an `egui::ComboBox`: its popup, once shrunk by a filter, never grows back,
/// while this one returns to its full height when the filter is cleared.
pub(super) fn sleeve_combo(
    ui: &mut Ui,
    id: egui::Id,
    width: f32,
    sleeves: &[Sleeve],
    current: &str,
    picked: &mut Option<String>,
) -> egui::Response {
    let label = if sleeves.is_empty() {
        "No sleeves found".to_string()
    } else if current.trim().is_empty() {
        "None".to_string()
    } else {
        sleeve_display_name(current.trim())
    };
    let button = egui::Button::new(format!("{label} \u{25be}"))
        .wrap_mode(egui::TextWrapMode::Truncate)
        .min_size(vec2(width, 0.0));
    let response = ui.add_enabled(!sleeves.is_empty(), button);
    let shown = egui::Popup::from_toggle_button_response(&response)
        .id(id.with("sleeve_popup"))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            sleeve_grid(ui, id, sleeves, current, picked);
        });
    if shown.is_none() {
        clear_sleeve_query(ui.ctx(), id);
    }
    response
}

/// The sleeve as the pickers show it: file name without extension.
fn sleeve_display_name(name: &str) -> String {
    name.strip_suffix(".jpg")
        .or_else(|| name.strip_suffix(".jpeg"))
        .or_else(|| name.strip_suffix(".png"))
        .unwrap_or(name)
        .to_string()
}

/// Forgets the sleeve filter of the picker `id`: called when its popup closes.
fn clear_sleeve_query(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|d| d.remove_temp::<String>(id.with("sleeve_query")));
}

/// One sleeve of the picker: thumbnail and name, highlighted when selected.
fn sleeve_item(ui: &mut Ui, sleeve: &Sleeve, selected: bool, picked: &mut Option<String>) {
    let s = scheme(ui);
    let name = sleeve_display_name(&sleeve.name);
    ui.vertical(|ui| {
        ui.set_max_width(80.0);
        let image = assets::image_from_path(&sleeve.path)
            .fit_to_exact_size(vec2(80.0, 80.0 * CARD_RATIO))
            .corner_radius(CornerRadius::same(6))
            .sense(Sense::click());
        let r = ui.add(image);
        if selected {
            ui.painter().rect_stroke(
                r.rect,
                CornerRadius::same(6),
                Stroke::new(2.0, s.primary),
                egui::StrokeKind::Outside,
            );
        }
        let r = r
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_ui(|ui| {
                ui.set_width(200.0);
                ui.add(
                    assets::image_from_path(&sleeve.path)
                        .fit_to_exact_size(vec2(160.0, 160.0 * CARD_RATIO)),
                );
            });
        if r.clicked() {
            *picked = Some(sleeve.name.clone());
            ui.close();
        }
        // The name is clickable too, not only the picture.
        let text = RichText::new(name).size(9.0).color(if selected {
            s.on_surface
        } else {
            s.on_surface_variant
        });
        if ui
            .add(egui::Button::new(text).frame(false))
            .on_hover_ui(|ui| {
                ui.set_width(200.0);
                ui.add(
                    assets::image_from_path(&sleeve.path)
                        .fit_to_exact_size(vec2(160.0, 160.0 * CARD_RATIO)),
                );
            })
            .clicked()
        {
            *picked = Some(sleeve.name.clone());
            ui.close();
        }
    });
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
                .fit_to_exact_size(vec2(34.0, 34.0 * CARD_RATIO))
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
    let thumb = vec2(thumb_width, thumb_width * CARD_RATIO);
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
        super::actions_row(ui, |ui| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

    fn fake_sleeves(n: usize) -> Vec<Sleeve> {
        (0..n)
            .map(|i| Sleeve {
                name: format!("Sleeve {i}.jpg"),
                path: format!("/nowhere/sleeve {i}.jpg").into(),
            })
            .collect()
    }

    /// The picker lays the sleeves out in wrapped rows, not one endless line, and picking
    /// one (or none at all) reports it.
    #[test]
    fn sleeve_grid_wraps_into_rows() {
        use std::cell::RefCell;

        let sleeves = fake_sleeves(20);
        let picked = RefCell::new(None);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let mut picked = picked.borrow_mut();
                sleeve_grid(ui, egui::Id::new("test_grid"), &sleeves, "", &mut picked);
            });
        harness.run_steps(2);

        let first = harness.get_by_label("Sleeve 0").rect();
        let last = harness.get_by_label("Sleeve 19").rect();
        assert!(
            first.min.y < last.min.y,
            "the grid must have several rows, got one at y {} and {}",
            first.min.y,
            last.min.y
        );
        assert!(
            last.max.x < SLEEVE_GRID_WIDTH + 20.0,
            "the grid must wrap at {SLEEVE_GRID_WIDTH}, sleeve 19 ends at x {}",
            last.max.x
        );

        harness.get_by_label("Sleeve 19").click();
        harness.run_steps(2);
        assert_eq!(picked.borrow().as_deref(), Some("Sleeve 19.jpg"));

        harness.get_by_label("No sleeve").click();
        harness.run_steps(2);
        assert_eq!(picked.borrow().as_deref(), Some(""));
    }

    /// Typing in the picker keeps only the sleeves whose name matches.
    #[test]
    fn sleeve_grid_filters_by_name() {
        use std::cell::RefCell;

        let sleeves = fake_sleeves(20);
        let picked = RefCell::new(None);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let mut picked = picked.borrow_mut();
                sleeve_grid(ui, egui::Id::new("test_filter"), &sleeves, "", &mut picked);
            });
        harness.run_steps(2);
        assert!(harness.query_by_label("Sleeve 2").is_some());
        assert!(harness.query_by_label("Sleeve 19").is_some());

        let filter = harness.get_by_role(egui::accesskit::Role::TextInput);
        filter.click();
        filter.type_text("19");
        harness.run_steps(2);
        assert!(harness.query_by_label("Sleeve 19").is_some());
        assert!(
            harness.query_by_label("Sleeve 2").is_none(),
            "a filtered-out sleeve must not be in the grid"
        );

        // Picking the only match works with the filter on.
        harness.get_by_label("Sleeve 19").click();
        harness.run_steps(2);
        assert_eq!(picked.borrow().as_deref(), Some("Sleeve 19.jpg"));
    }

    /// The picker keeps a bounded height: full size with the whole list, smaller with a
    /// filter, and back to full size when the filter is cleared. It must never grow to
    /// the screen (as it did when the scroll area filled whatever height was left).
    #[test]
    fn sleeve_grid_keeps_a_bounded_fixed_height() {
        use std::cell::RefCell;

        let sleeves = fake_sleeves(20);
        let picked: RefCell<Option<String>> = RefCell::new(None);
        let height = RefCell::new(0.0);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let mut picked = picked.borrow_mut();
                sleeve_grid(ui, egui::Id::new("test_height"), &sleeves, "", &mut picked);
                *height.borrow_mut() = ui.min_rect().height();
            });
        harness.run_steps(2);
        let unfiltered = *height.borrow();
        assert!(
            unfiltered >= SLEEVE_POPUP_HEIGHT,
            "the picker must be at least the grid's {SLEEVE_POPUP_HEIGHT}, got {unfiltered}"
        );
        assert!(
            unfiltered < 600.0,
            "the picker must stay bounded, got {unfiltered}"
        );

        let filter = harness.get_by_role(egui::accesskit::Role::TextInput);
        filter.click();
        filter.type_text("19");
        harness.run_steps(2);
        let filtered = *height.borrow();
        assert!(
            filtered < SLEEVE_POPUP_HEIGHT,
            "filtered, the picker shrinks to its few sleeves, got {filtered}"
        );

        for _ in 0.."19".len() {
            harness.key_press(egui::Key::Backspace);
        }
        harness.run_steps(2);
        assert!(
            (*height.borrow() - unfiltered).abs() < 1.0,
            "the filter cleared, the picker must be full size again: {} vs {}",
            *height.borrow(),
            unfiltered
        );
    }

    /// The popup of the combo stays bounded: a tall grid must not make it grow to the
    /// screen with the grid stuck at the top.
    #[test]
    fn sleeve_combo_popup_stays_bounded() {
        use std::cell::RefCell;

        let sleeves = fake_sleeves(20);
        let picked: RefCell<Option<String>> = RefCell::new(None);
        let popup = RefCell::new(Vec::new());
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let button = ui.add(egui::Button::new("open"));
                egui::Popup::from_toggle_button_response(&button)
                    .id(egui::Id::new("probe"))
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        let mut picked = picked.borrow_mut();
                        sleeve_grid(ui, egui::Id::new("probe_grid"), &sleeves, "", &mut picked);
                        popup.borrow_mut().push(ui.min_rect());
                    });
            });
        harness.run_steps(2);
        harness.get_by_label("open").click();
        harness.run_steps(5);

        let heights: Vec<f32> = popup.borrow().iter().map(|r| r.height()).collect();
        assert!(!heights.is_empty(), "the popup never opened");
        for (i, h) in heights.iter().enumerate() {
            assert!(
                *h < 700.0,
                "the popup must stay bounded, frame {i} is {h} tall (window is 900)"
            );
        }
        let first = heights[heights.len() / 2];
        let last = *heights.last().unwrap();
        assert!(
            (first - last).abs() < 2.0,
            "the popup must not resize while it is open: {first} vs {last}"
        );
    }
}
