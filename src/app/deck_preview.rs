//! The cards of a deck as an open deck shows them: composition tiles, then the card
//! groups, with a detail pane that follows hover and clicks. Shared by the simulator
//! decks and the Encore Decks page.

use std::collections::{BTreeMap, HashMap};

use egui::{Align2, CornerRadius, Margin, Sense, Stroke, Ui, vec2};
use egui_material_icons::icons::{ICON_CHECK_CIRCLE, ICON_WARNING};
use simu_deck_parser::filters::{DECK_ORDER, sort_cards};
use simu_deck_parser::model::{Card, CardType, Color, Trigger};

use super::card_view::{CARD_RATIO, DetailPane};
use super::theme::{self, scheme};
use crate::assets;

const THUMB_WIDTH: f32 = 128.0;
const DECK_SIZE: usize = 50;
const MAX_CLIMAXES: usize = 8;

/// Deck cards grouped by code, in deck order, with composition counts.
pub(super) struct Summary {
    groups: Vec<(CardType, Vec<(Card, usize)>)>,
    total: usize,
    by_type: BTreeMap<CardType, usize>,
    by_level: [usize; 4],
    by_color: BTreeMap<Color, usize>,
    /// Cards with at least one trigger icon.
    triggers: usize,
    /// Trigger icons, counted once per card carrying them.
    by_trigger: BTreeMap<Trigger, usize>,
}

impl Summary {
    pub(super) fn new(cards: &[Card]) -> Self {
        let mut sorted = cards.to_vec();
        sort_cards(&mut sorted, DECK_ORDER);
        let mut groups: Vec<(CardType, Vec<(Card, usize)>)> = Vec::new();
        let mut index: HashMap<String, (usize, usize)> = HashMap::new();
        let mut by_type = BTreeMap::new();
        let mut by_level = [0; 4];
        let mut by_color = BTreeMap::new();
        let mut triggers = 0;
        let mut by_trigger = BTreeMap::new();
        for card in sorted {
            *by_type.entry(card.card_type).or_default() += 1;
            *by_color.entry(card.color).or_default() += 1;
            if !card.triggers.is_empty() {
                triggers += 1;
            }
            for &t in &card.triggers {
                *by_trigger.entry(t).or_default() += 1;
            }
            if card.card_type != CardType::Climax {
                by_level[(card.level as usize).min(3)] += 1;
            }
            if let Some(&(g, i)) = index.get(&card.key) {
                groups[g].1[i].1 += 1;
                continue;
            }
            if groups.last().is_none_or(|(t, _)| *t != card.card_type) {
                groups.push((card.card_type, Vec::new()));
            }
            let g = groups.len() - 1;
            index.insert(card.key.clone(), (g, groups[g].1.len()));
            groups[g].1.push((card, 1));
        }
        Self {
            groups,
            total: cards.len(),
            by_type,
            by_level,
            by_color,
            triggers,
            by_trigger,
        }
    }
}

pub(super) fn plural(t: CardType) -> &'static str {
    match t {
        CardType::Character => "Characters",
        CardType::Event => "Events",
        CardType::Climax => "Climaxes",
    }
}

/// The interactive part of an open deck: composition tiles and card groups, plus the
/// detail pane that hover and clicks feed.
#[derive(Default)]
pub(super) struct DeckPreview {
    pane: DetailPane,
}

impl DeckPreview {
    pub(super) fn clear(&mut self) {
        self.pane.clear();
    }

    pub(super) fn pane(&mut self) -> &mut DetailPane {
        &mut self.pane
    }

    /// Draws the deck's unmatched codes, composition tiles and card groups. Run inside
    /// the caller's scroll area; the pane on the right is drawn separately.
    pub(super) fn show(&mut self, ui: &mut Ui, summary: &Summary, missing: &[String]) {
        let s = scheme(ui);
        let pinned = self.pane.pinned_key().map(str::to_owned);
        let mut hovered = None;
        let mut clicked = None;
        if !missing.is_empty() {
            theme::banner(
                ui,
                ICON_WARNING,
                &format!(
                    "{} code(s) matched no card in the loaded simulator: {}",
                    missing.len(),
                    missing.join(", ")
                ),
            );
            ui.add_space(8.0);
        }
        stats(ui, summary);
        ui.add_space(12.0);

        let thumb = vec2(THUMB_WIDTH, THUMB_WIDTH * CARD_RATIO);
        for (card_type, cards) in &summary.groups {
            let count: usize = cards.iter().map(|(_, n)| n).sum();
            theme::section_header(ui, plural(*card_type), Some(count));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                for (card, copies) in cards {
                    let r = ui.add(
                        assets::card_image(card)
                            .fit_to_exact_size(thumb)
                            .corner_radius(CornerRadius::same(8))
                            .sense(Sense::click()),
                    );
                    let selected = pinned.as_deref() == Some(card.key.as_str());
                    if selected || r.hovered() {
                        ui.painter().rect_stroke(
                            r.rect,
                            CornerRadius::same(8),
                            Stroke::new(
                                if selected { 3.0 } else { 2.0 },
                                if selected { s.primary } else { s.outline },
                            ),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if *copies > 1 {
                        theme::count_badge(ui, r.rect, *copies);
                    }
                    let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if r.hovered() {
                        hovered = Some(card);
                    }
                    if r.clicked() {
                        clicked = Some(card);
                    }
                }
            });
            ui.add_space(12.0);
        }
        if let Some(card) = hovered {
            self.pane.hover(card);
        }
        if let Some(card) = clicked {
            self.pane.click(card);
        }
    }
}

/// Composition tiles: size check, types, triggers, level curve and colours.
fn stats(ui: &mut Ui, summary: &Summary) {
    let s = scheme(ui);
    let dark = ui.visuals().dark_mode;
    let climaxes = summary.by_type.get(&CardType::Climax).copied().unwrap_or(0);
    let top_wrapped = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
    ui.with_layout(top_wrapped, |ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
        // Fixed-size tiles, so the row wraps on narrow windows.
        let tile = |ui: &mut Ui, width: f32, add: &mut dyn FnMut(&mut Ui)| {
            ui.allocate_ui(vec2(width, 112.0), |ui| {
                theme::surface_card(s.surface_container_low)
                    .inner_margin(Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_width(width - 32.0);
                        ui.set_height(112.0 - 24.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            add(ui);
                        });
                    });
            });
        };

        // Deck size & legality hints.
        tile(ui, 180.0, &mut |ui| {
            ui.label(theme::label("Cards").color(s.on_surface_variant));
            ui.label(
                theme::headline(format!("{} / {DECK_SIZE}", summary.total))
                    .color(s.on_surface),
            );
            let ok = summary.total == DECK_SIZE && climaxes <= MAX_CLIMAXES;
            let (icon, text, color) = if ok {
                (ICON_CHECK_CIRCLE, "Legal size".to_string(), s.success)
            } else if climaxes > MAX_CLIMAXES {
                (
                    ICON_WARNING,
                    format!("{climaxes} climaxes (max {MAX_CLIMAXES})"),
                    s.error,
                )
            } else {
                (ICON_WARNING, format!("Needs exactly {DECK_SIZE}"), s.error)
            };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(icon.rich_text().size(16.0).color(color));
                ui.label(theme::body_small(text).color(color));
            });
        });

        // Types.
        tile(ui, 180.0, &mut |ui| {
            ui.label(theme::label("Types").color(s.on_surface_variant));
            egui::Grid::new("deck_types")
                .num_columns(2)
                .spacing(vec2(24.0, 0.0))
                .show(ui, |ui| {
                    for t in [CardType::Character, CardType::Event, CardType::Climax] {
                        let n = summary.by_type.get(&t).copied().unwrap_or(0);
                        ui.label(theme::body_small(plural(t)).color(s.on_surface_variant));
                        ui.label(theme::label(n.to_string()).color(s.on_surface));
                        ui.end_row();
                    }
                });
        });

        // Triggers: cards with and without a trigger icon, breakdown on hover.
        tile(ui, 180.0, &mut |ui| {
            ui.label(theme::label("Triggers").color(s.on_surface_variant));
            let r = egui::Grid::new("deck_triggers")
                .num_columns(2)
                .spacing(vec2(24.0, 0.0))
                .show(ui, |ui| {
                    let rows = [
                        ("Triggers", summary.triggers),
                        ("Non-triggers", summary.total - summary.triggers),
                    ];
                    for (name, n) in rows {
                        ui.label(theme::body_small(name).color(s.on_surface_variant));
                        ui.label(theme::label(n.to_string()).color(s.on_surface));
                        ui.end_row();
                    }
                })
                .response;
            if !summary.by_trigger.is_empty() {
                let breakdown = summary
                    .by_trigger
                    .iter()
                    .map(|(t, n)| format!("{}: {n}", t.label()))
                    .collect::<Vec<_>>()
                    .join("\n");
                r.on_hover_text(breakdown);
            }
        });

        // Level curve (characters and events): one hue, direct labels, hover tooltips.
        tile(ui, 180.0, &mut |ui| {
            ui.label(theme::label("Level curve").color(s.on_surface_variant));
            let max = summary.by_level.iter().copied().max().unwrap_or(0).max(1);
            let (rect, _) = ui.allocate_exact_size(vec2(4.0 * 36.0, 64.0), Sense::hover());
            let painter = ui.painter();
            let baseline = rect.bottom() - 16.0;
            let chart_height = rect.height() - 32.0;
            for (level, &n) in summary.by_level.iter().enumerate() {
                let x = rect.left() + level as f32 * 36.0;
                let bar = egui::Rect::from_min_max(
                    egui::pos2(x + 6.0, baseline - chart_height * n as f32 / max as f32),
                    egui::pos2(x + 30.0, baseline),
                );
                if n > 0 {
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
                painter.text(
                    egui::pos2(bar.center().x, bar.top() - 2.0),
                    Align2::CENTER_BOTTOM,
                    n.to_string(),
                    theme::medium(12.0),
                    s.on_surface,
                );
                painter.text(
                    egui::pos2(bar.center().x, baseline + 2.0),
                    Align2::CENTER_TOP,
                    format!("L{level}"),
                    theme::regular(11.0),
                    s.on_surface_variant,
                );
                let hit = egui::Rect::from_min_max(
                    egui::pos2(x, rect.top()),
                    egui::pos2(x + 36.0, rect.bottom()),
                );
                let r = ui.interact(hit, ui.id().with(("level", level)), Sense::hover());
                r.on_hover_text(format!("Level {level}: {n} cards"));
            }
            painter.line_segment(
                [
                    egui::pos2(rect.left(), baseline),
                    egui::pos2(rect.right(), baseline),
                ],
                Stroke::new(1.0, s.outline_variant),
            );
        });

        // Colours: proportional bar with a labelled legend.
        tile(ui, 240.0, &mut |ui| {
            ui.label(theme::label("Colors").color(s.on_surface_variant));
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 12.0), Sense::hover());
            let total = summary.total.max(1) as f32;
            let mut x = rect.left();
            for (&color, &n) in &summary.by_color {
                let w = rect.width() * n as f32 / total;
                let seg = egui::Rect::from_min_max(
                    egui::pos2(x, rect.top()),
                    egui::pos2(x + w - 2.0, rect.bottom()),
                );
                ui.painter()
                    .rect_filled(seg, CornerRadius::same(4), theme::card_color(color, dark));
                x += w;
            }
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 2.0);
                for (&color, &n) in &summary.by_color {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                        ui.painter()
                            .circle_filled(r.center(), 5.0, theme::card_color(color, dark));
                        ui.label(
                            theme::body_small(format!("{} {n}", color.label()))
                                .color(s.on_surface_variant),
                        );
                    });
                }
            });
        });
    });
}
