//! Card library: browse a serie (or one of its sets) with search, filter chips and a preview pane.
//! The deck builder reuses it with a [`Picker`]: clicks add cards to the deck instead.

use egui::{CornerRadius, Margin, ScrollArea, Sense, Stroke, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ADD, ICON_FILTER_LIST, ICON_FILTER_LIST_OFF, ICON_GRID_VIEW, ICON_REMOVE, ICON_SEARCH_OFF,
    ICON_STYLE, ICON_VIEW_LIST,
};
use simu_deck_parser::filters::{CARD_LIST_ORDER, Choice, Filters, sort_cards};
use simu_deck_parser::model::{Card, Serie};

use super::card_view::{self, CARD_RATIO, DetailPane, card_summary, card_text};
use super::theme::{self, scheme};
use crate::assets;

const GRID_GAP: f32 = 10.0;
const LIST_IMAGE_WIDTH: f32 = 150.0;
/// Trait groups longer than this get their own filter field.
const TRAIT_SEARCH_THRESHOLD: usize = 12;

/// Deck building on top of the card browser.
pub trait Picker {
    fn copies(&self, card: &Card) -> usize;
    fn add(&mut self, card: &Card);
    fn remove(&mut self, card: &Card);
    /// Right-hand panel (the deck being built). Returns the card under the pointer there, for
    /// the detail pane, which keeps showing it when the pointer leaves.
    fn side_panel(&mut self, ui: &mut Ui) -> Option<Card>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Grid,
    List,
}

pub struct CardList {
    /// Loaded serie, and the set shown (`None` = every set of the serie).
    serie: Option<usize>,
    set: Option<usize>,
    serie_query: String,
    trait_query: String,
    /// Every card of the loaded serie/set, and what survives the filters.
    cards: Vec<Card>,
    shown: Vec<Card>,
    filters: Filters,
    show_filters: bool,
    view: View,
    thumb_width: f32,
    pane: DetailPane,
    focus_search: bool,
}

impl Default for CardList {
    fn default() -> Self {
        Self {
            serie: None,
            set: None,
            serie_query: String::new(),
            trait_query: String::new(),
            cards: Vec::new(),
            shown: Vec::new(),
            filters: Filters::default(),
            show_filters: true,
            view: View::Grid,
            thumb_width: 150.0,
            pane: DetailPane::default(),
            focus_search: false,
        }
    }
}

impl CardList {
    pub fn serie_name<'a>(&self, series: &'a [Serie]) -> Option<&'a str> {
        self.serie
            .and_then(|i| series.get(i))
            .map(|s| s.name.as_str())
    }

    /// Opens the serie named `name` (falls back to the first one).
    pub fn open_serie(&mut self, series: &[Serie], name: Option<&str>) {
        let index = name
            .and_then(|n| series.iter().position(|s| s.name == n))
            .or((!series.is_empty()).then_some(0));
        if let Some(i) = index {
            self.load(series, i, None);
        }
    }

    fn load(&mut self, series: &[Serie], serie: usize, set: Option<usize>) {
        let Some(s) = series.get(serie) else { return };
        self.serie = Some(serie);
        self.set = set.filter(|&i| i < s.sets.len());
        self.cards = match self.set {
            Some(i) => s.sets[i].cards.values().cloned().collect(),
            None => s
                .sets
                .iter()
                .flat_map(|s| s.cards.values())
                .cloned()
                .collect(),
        };
        let search = std::mem::take(&mut self.filters.search);
        self.filters = Filters::from_cards(&self.cards);
        self.filters.search = search;
        self.trait_query.clear();
        self.pane.clear();
        self.refilter();
    }

    fn refilter(&mut self) {
        self.shown = self.filters.apply(&self.cards);
        sort_cards(&mut self.shown, CARD_LIST_ORDER);
    }

    /// Opens the serie holding the card `key`, if any.
    pub fn open_serie_of(&mut self, series: &[Serie], key: &str) {
        if let Some(serie) = series
            .iter()
            .find(|s| s.sets.iter().any(|set| set.cards.contains_key(key)))
        {
            self.open_serie(series, Some(&serie.name));
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        series: &[Serie],
        title: &str,
        mut picker: Option<&mut dyn Picker>,
    ) {
        if self.serie.is_none_or(|i| i >= series.len()) {
            self.open_serie(series, None);
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            self.focus_search = true;
        }
        let s = scheme(ui);

        let mut changed_serie = None;
        egui::Panel::top("card_list_top")
            .frame(
                egui::Frame::new()
                    .fill(s.surface)
                    .inner_margin(Margin::symmetric(20, 12)),
            )
            .show_separator_line(false)
            .show(ui, |ui| changed_serie = self.top_bar(ui, series, title));
        if let Some((serie, set)) = changed_serie {
            ui.ctx().forget_all_images();
            self.load(series, serie, set);
        }

        if self.show_filters && self.serie.is_some() {
            egui::Panel::left("card_list_filters")
                .resizable(false)
                .exact_size(280.0)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                    left: 16,
                    right: 8,
                    top: 0,
                    bottom: 16,
                }))
                .show(ui, |ui| {
                    theme::surface_card(s.surface_container_low).show(ui, |ui| {
                        ui.set_min_height(ui.available_height());
                        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                            if self.filter_panel(ui) {
                                self.refilter();
                            }
                        });
                    });
                });
        }

        let mut panel_hovered = None;
        if let Some(picker) = picker.as_deref_mut() {
            egui::Panel::right("card_list_deck")
                .resizable(true)
                .default_size(400.0)
                .size_range(340.0..=600.0)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                    left: 8,
                    right: 16,
                    top: 0,
                    bottom: 16,
                }))
                .show(ui, |ui| {
                    theme::surface_card(s.surface_container_low).show(ui, |ui| {
                        ui.set_min_height(ui.available_height());
                        panel_hovered = picker.side_panel(ui);
                    });
                });
        }
        if let Some(card) = &panel_hovered {
            self.pane.hover(card);
        }
        // Grid view of the library; in list view only for a card pinned from the related
        // cards window. The deck builder always shows it: it follows the pointer over the
        // library and the deck and keeps the last card when the pointer leaves.
        let pane = (picker.is_none() && self.view == View::Grid)
            || picker.is_some()
            || card_view::pin_requested(ui.ctx());
        if pane {
            egui::Panel::right("card_list_detail")
                .resizable(true)
                .default_size(360.0)
                .size_range(280.0..=560.0)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                    left: 8,
                    right: 16,
                    top: 0,
                    bottom: 16,
                }))
                .show(ui, |ui| {
                    theme::surface_card(s.surface_container_low).show(ui, |ui| {
                        ui.set_min_height(ui.available_height());
                        self.pane.show(ui);
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                left: 8,
                right: 8,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| self.content(ui, series, picker));
    }

    /// Serie & set pickers, search and view toggles. Returns a new (serie, set) to load.
    fn top_bar(
        &mut self,
        ui: &mut Ui,
        series: &[Serie],
        title: &str,
    ) -> Option<(usize, Option<usize>)> {
        let s = scheme(ui);
        let mut load = None;
        ui.horizontal(|ui| {
            ui.label(theme::headline(title).color(s.on_surface));
            ui.add_space(16.0);
            if series.is_empty() {
                ui.label("No series found in the simulator.");
                return;
            }
            let current = self.serie.unwrap_or(0).min(series.len() - 1);

            let serie_combo = egui::ComboBox::from_id_salt("serie")
                .width(280.0)
                .height(420.0)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .selected_text(&series[current].name)
                .show_ui(ui, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.serie_query)
                            .hint_text("Find a serie")
                            .desired_width(f32::INFINITY),
                    );
                    if !edit.has_focus() && self.serie_query.is_empty() {
                        edit.request_focus();
                    }
                    ui.separator();
                    let query = self.serie_query.trim().to_lowercase();
                    for (i, serie) in series.iter().enumerate() {
                        if !query.is_empty() && !serie.name.to_lowercase().contains(&query) {
                            continue;
                        }
                        if ui.selectable_label(i == current, &serie.name).clicked() {
                            load = Some((i, None));
                            ui.close();
                        }
                    }
                });
            serie_combo.response.on_hover_text("Serie");
            if serie_combo.inner.is_none() && !self.serie_query.is_empty() {
                self.serie_query.clear();
            }

            let sets = &series[current].sets;
            if sets.len() > 1 {
                let set_name = |i: Option<usize>| {
                    i.and_then(|i| sets.get(i))
                        .map_or_else(|| format!("All sets ({})", sets.len()), |s| s.name.clone())
                };
                egui::ComboBox::from_id_salt("set")
                    .width(220.0)
                    .selected_text(set_name(self.set))
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.set.is_none(), set_name(None))
                            .clicked()
                        {
                            load = Some((current, None));
                        }
                        for (i, set) in sets.iter().enumerate() {
                            if ui
                                .selectable_label(self.set == Some(i), &set.name)
                                .clicked()
                            {
                                load = Some((current, Some(i)));
                            }
                        }
                    })
                    .response
                    .on_hover_text("Set");
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let list = theme::icon_button(ICON_VIEW_LIST, "List view")
                    .selected(self.view == View::List)
                    .show(ui);
                if list.clicked() {
                    self.view = View::List;
                }
                let grid = theme::icon_button(ICON_GRID_VIEW, "Grid view")
                    .selected(self.view == View::Grid)
                    .show(ui);
                if grid.clicked() {
                    self.view = View::Grid;
                }
                let filter_tip = if self.show_filters {
                    "Hide filters"
                } else {
                    "Show filters"
                };
                let filters = theme::icon_button(ICON_FILTER_LIST, filter_tip)
                    .selected(self.show_filters)
                    .show(ui);
                if !self.filters.is_unfiltered() {
                    // Badge: filters are narrowing the list.
                    let c = filters.rect.right_top() + vec2(-8.0, 8.0);
                    ui.painter().circle_filled(c, 4.0, s.error);
                }
                if filters.clicked() {
                    self.show_filters = !self.show_filters;
                }
                ui.add_space(8.0);
                let width = (ui.available_width() - 16.0).clamp(160.0, 360.0);
                let search = theme::search_bar(
                    ui,
                    &mut self.filters.search,
                    "Search name, code or text  (Ctrl+F)",
                    width,
                );
                if std::mem::take(&mut self.focus_search) {
                    search.request_focus();
                }
                if search.changed() {
                    self.refilter();
                }
            });
        });
        load
    }

    /// Filter chips. Returns `true` when a filter changed.
    fn filter_panel(&mut self, ui: &mut Ui) -> bool {
        fn group<T: Ord + Clone>(
            ui: &mut Ui,
            title: &str,
            choice: &mut Choice<T>,
            label: impl Fn(&T) -> String,
            dot: impl Fn(&T) -> Option<egui::Color32>,
        ) -> bool {
            if choice.available.len() < 2 {
                return false;
            }
            let mut changed = false;
            ui.add_space(8.0);
            theme::section_header(ui, title, None);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for v in choice.available.clone() {
                    let text = label(&v);
                    let mut chip = theme::chip(&text, choice.is_picked(&v));
                    if let Some(color) = dot(&v) {
                        chip = chip.dot(color);
                    }
                    if chip.show(ui).clicked() {
                        choice.click(&v);
                        changed = true;
                    }
                }
            });
            changed
        }

        let s = scheme(ui);
        let dark = ui.visuals().dark_mode;
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(theme::title("Filters").color(s.on_surface));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let reset = theme::text(None, "Clear")
                    .enabled(!self.filters.is_unfiltered())
                    .show(ui);
                if reset.clicked() {
                    self.filters.clear();
                    changed = true;
                }
            });
        });
        let f = &mut self.filters;
        changed |= group(ui, "Type", &mut f.types, |t| t.label().into(), |_| None);
        changed |= group(
            ui,
            "Color",
            &mut f.colors,
            |c| c.label().into(),
            |c| Some(theme::card_color(*c, dark)),
        );
        changed |= group(ui, "Level", &mut f.levels, |v| v.to_string(), |_| None);
        changed |= group(ui, "Cost", &mut f.costs, |v| v.to_string(), |_| None);

        if f.has_cx_combo {
            ui.add_space(8.0);
            theme::section_header(ui, "Keywords", None);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                if theme::chip("Climax combo", f.cx_combo).show(ui).clicked() {
                    f.cx_combo = !f.cx_combo;
                    changed = true;
                }
            });
        }

        // Traits: often dozens, so they get a filter field and pick-all shortcuts.
        if !f.traits.available.is_empty() {
            ui.add_space(8.0);
            theme::section_header(ui, "Traits", Some(f.traits.available.len()));
            if f.traits.available.len() > TRAIT_SEARCH_THRESHOLD {
                ui.add(
                    egui::TextEdit::singleline(&mut self.trait_query)
                        .hint_text("Find a trait")
                        .desired_width(f32::INFINITY)
                        .margin(Margin::symmetric(12, 6)),
                );
            }
            let query = self.trait_query.trim().to_lowercase();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for t in f.traits.available.clone() {
                    if !query.is_empty() && !t.to_lowercase().contains(&query) {
                        continue;
                    }
                    if theme::chip(&t, f.traits.is_picked(&t)).show(ui).clicked() {
                        f.traits.click(&t);
                        changed = true;
                    }
                }
            });
        }
        changed
    }

    fn content(&mut self, ui: &mut Ui, series: &[Serie], picker: Option<&mut dyn Picker>) {
        let s = scheme(ui);
        let Some(serie) = self.serie.and_then(|i| series.get(i)) else {
            theme::empty_state(
                ui,
                ICON_STYLE,
                "No cards",
                "No serie was found in the simulator folder.",
            );
            return;
        };
        ui.horizontal(|ui| {
            let title = match self.set.and_then(|i| serie.sets.get(i)) {
                Some(set) => format!("{} · {}", serie.name, set.name),
                None => serie.name.clone(),
            };
            ui.label(theme::title_medium(title).color(s.on_surface));
            ui.label(
                theme::label(if self.shown.len() == self.cards.len() {
                    format!("{} cards", self.cards.len())
                } else {
                    format!("{} of {} cards", self.shown.len(), self.cards.len())
                })
                .color(s.on_surface_variant),
            );
            if picker.is_some() {
                ui.label(
                    theme::body_small("· click a card to add it, right-click to remove it")
                        .color(s.on_surface_variant),
                );
            }
            if self.view == View::Grid {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Slider::new(&mut self.thumb_width, 100.0..=240.0)
                            .show_value(false)
                            .trailing_fill(true),
                    )
                    .on_hover_text("Card size");
                    ui.label(theme::label("Size").color(s.on_surface_variant));
                });
            }
        });
        ui.add_space(4.0);

        if self.shown.is_empty() {
            let filtered = !self.filters.is_unfiltered();
            theme::empty_state(
                ui,
                ICON_SEARCH_OFF,
                "No matching cards",
                if filtered {
                    "The search only looks at cards that pass the filters."
                } else {
                    "Try another search."
                },
            );
            if filtered {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    if theme::tonal(Some(ICON_FILTER_LIST_OFF), "Clear filters")
                        .show(ui)
                        .clicked()
                    {
                        self.filters.clear();
                        self.refilter();
                    }
                });
            }
            return;
        }
        match self.view {
            View::Grid => self.grid(ui, picker),
            View::List => self.list(ui, picker),
        }
    }

    fn grid(&mut self, ui: &mut Ui, picker: Option<&mut dyn Picker>) {
        let s = scheme(ui);
        let thumb = vec2(self.thumb_width, self.thumb_width * CARD_RATIO);
        let columns = (((ui.available_width() - 12.0 + GRID_GAP) / (thumb.x + GRID_GAP)).floor()
            as usize)
            .max(1);
        let rows = self.shown.len().div_ceil(columns);
        let pinned = self.pane.pinned_key().map(str::to_owned);
        let mut hovered = None;
        let mut clicked = None;
        let mut removed = None;
        ScrollArea::vertical().auto_shrink(false).show_rows(
            ui,
            thumb.y + GRID_GAP,
            rows,
            |ui, range| {
                ui.spacing_mut().item_spacing = vec2(GRID_GAP, GRID_GAP);
                for row in range {
                    ui.horizontal(|ui| {
                        let start = row * columns;
                        let end = (start + columns).min(self.shown.len());
                        for card in &self.shown[start..end] {
                            let r = ui.add(
                                assets::card_image(card)
                                    .fit_to_exact_size(thumb)
                                    .corner_radius(CornerRadius::same(8))
                                    .sense(Sense::click()),
                            );
                            let copies = picker.as_deref().map_or(0, |p| p.copies(card));
                            let selected = copies > 0
                                || (picker.is_none()
                                    && pinned.as_deref() == Some(card.key.as_str()));
                            if selected || r.hovered() {
                                let width = if selected { 3.0 } else { 2.0 };
                                let color = if selected { s.primary } else { s.outline };
                                ui.painter().rect_stroke(
                                    r.rect,
                                    CornerRadius::same(8),
                                    Stroke::new(width, color),
                                    egui::StrokeKind::Outside,
                                );
                            }
                            if copies > 0 {
                                theme::count_badge(ui, r.rect, copies);
                            }
                            let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
                            if r.hovered() {
                                hovered = Some(card);
                            }
                            if r.clicked() {
                                clicked = Some(card);
                            }
                            if r.secondary_clicked() {
                                removed = Some(card);
                            }
                        }
                    });
                }
            },
        );
        match picker {
            Some(picker) => {
                if let Some(card) = hovered {
                    self.pane.hover(card);
                }
                if let Some(card) = clicked {
                    picker.add(card);
                }
                if let Some(card) = removed {
                    picker.remove(card);
                }
            }
            None => {
                if let Some(card) = hovered {
                    self.pane.hover(card);
                }
                if let Some(card) = clicked {
                    self.pane.click(card);
                }
            }
        }
    }

    fn list(&self, ui: &mut Ui, mut picker: Option<&mut dyn Picker>) {
        let s = scheme(ui);
        let image = vec2(LIST_IMAGE_WIDTH, LIST_IMAGE_WIDTH * CARD_RATIO);
        let padding = 12.0;
        let spacing = 8.0;
        ScrollArea::vertical().auto_shrink(false).show_rows(
            ui,
            image.y + padding * 2.0 + spacing,
            self.shown.len(),
            |ui, rows| {
                ui.spacing_mut().item_spacing.y = spacing;
                for card in &self.shown[rows] {
                    theme::surface_card(s.surface_container_low)
                        .inner_margin(Margin::same(padding as i8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width() - padding * 2.0 - 8.0);
                            // Room for the effect text too when a side pane narrows the list.
                            let summary = (ui.available_width() - image.x - 24.0) * 0.45;
                            ui.horizontal_top(|ui| {
                                ui.set_height(image.y);
                                ui.add(
                                    assets::card_image(card)
                                        .fit_to_exact_size(image)
                                        .corner_radius(CornerRadius::same(8)),
                                );
                                ui.add_space(12.0);
                                // Scrolls when narrow: every row has the picture's height.
                                ScrollArea::vertical()
                                    .id_salt((&card.key, "summary"))
                                    .max_height(image.y)
                                    .max_width(summary.min(400.0))
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            card_summary(ui, card);
                                            if let Some(picker) = picker.as_deref_mut() {
                                                ui.add_space(4.0);
                                                pick_buttons(ui, picker, card);
                                            }
                                        });
                                    });
                                ui.add_space(12.0);
                                ScrollArea::vertical()
                                    .id_salt(&card.key)
                                    .max_height(image.y)
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| card_text(ui, card));
                                    });
                            });
                        });
                }
            },
        );
    }
}

/// "Add" / "Remove" buttons of the list view while building a deck.
fn pick_buttons(ui: &mut Ui, picker: &mut dyn Picker, card: &Card) {
    ui.horizontal(|ui| {
        if theme::tonal(Some(ICON_ADD), "Add").show(ui).clicked() {
            picker.add(card);
        }
        let copies = picker.copies(card);
        if copies > 0 {
            if theme::text(Some(ICON_REMOVE), "Remove").show(ui).clicked() {
                picker.remove(card);
            }
            ui.label(theme::label(format!("×{copies} in deck")).color(scheme(ui).primary));
        }
    });
}
