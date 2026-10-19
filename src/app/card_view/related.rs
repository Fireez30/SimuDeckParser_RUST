//! "Related cards": the cards whose effect text names a card, and the ones its text names, in
//! a window of their own.
//!
//! The window is opened from anywhere a card is shown, so its state is kept in egui's memory
//! (`ctx.data`), a store any part of the UI can read, rather than in one screen's struct.

use std::sync::Arc;

use egui::{CornerRadius, Margin, ScrollArea, Sense, Ui, vec2};
use egui_material_icons::icons::ICON_HUB;
use simu_deck_parser::card_index::NameIndex;
use simu_deck_parser::model::{CARD_IMAGE_RATIO, Card};

use super::card_preview;
use crate::app::assets;
use crate::app::theme::{self, scheme};

fn names_id() -> egui::Id {
    egui::Id::new("card_names")
}

/// Makes the loaded cards' names known to effect text, so quoted names show their card on hover.
pub fn set_names(ctx: &egui::Context, names: Option<Arc<NameIndex>>) {
    ctx.data_mut(|d| match names {
        Some(names) => {
            d.insert_temp(names_id(), names);
        }
        None => {
            d.remove::<Arc<NameIndex>>(names_id());
            d.remove::<Arc<RelatedView>>(related_id());
        }
    });
}

pub(super) fn names(ctx: &egui::Context) -> Option<Arc<NameIndex>> {
    ctx.data(|d| d.get_temp::<Arc<NameIndex>>(names_id()))
}

/// The cards related to one card, shown in their own window.
struct RelatedView {
    target: Card,
    named: Vec<Card>,
    naming: Vec<Card>,
}

fn related_id() -> egui::Id {
    egui::Id::new("related_cards")
}

/// Card clicked in the related cards window, for the detail pane on screen to pin.
pub(super) fn pin_request_id() -> egui::Id {
    egui::Id::new("related_cards_pin")
}

/// A card clicked in the related cards window is waiting for a detail pane to show it.
pub fn pin_requested(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Card>(pin_request_id()).is_some())
}

/// Code of the card pinned in the detail pane on screen, highlighted in the window.
pub(super) fn pinned_id() -> egui::Id {
    egui::Id::new("detail_pane_pinned")
}

/// Shows the cards related to `target` in the related cards window.
fn open_related(ctx: &egui::Context, target: &Card) {
    let Some(names) = names(ctx) else {
        return;
    };
    let related = names.related(target);
    let owned = |cards: Vec<&Card>| cards.into_iter().cloned().collect();
    let view = RelatedView {
        target: target.clone(),
        named: owned(related.named),
        naming: owned(related.naming),
    };
    ctx.data_mut(|d| d.insert_temp(related_id(), Arc::new(view)));
}

/// "Related cards" button: opens the window listing the cards that react with `card`.
/// Returns whether it was clicked.
pub(super) fn related_button(ui: &mut Ui, card: &Card) -> bool {
    let clicked = theme::tonal(Some(ICON_HUB), "Related cards")
        .show(ui)
        .on_hover_text("Cards that name this one, and the ones it names")
        .clicked();
    if clicked {
        open_related(ui.ctx(), card);
    }
    clicked
}

/// Window listing the cards that react with the card whose "Related cards" was clicked: the
/// ones it names and the ones that name it. Clicking one shows it in the detail pane.
pub fn related_window(ctx: &egui::Context) {
    let Some(view) = ctx.data(|d| d.get_temp::<Arc<RelatedView>>(related_id())) else {
        return;
    };
    let target = &view.target;
    let pinned = ctx.data(|d| d.get_temp::<String>(pinned_id()));
    let mut open = true;
    let mut clicked = None;
    egui::Window::new("Related cards")
        .id(related_id())
        .open(&mut open)
        .collapsible(false)
        .default_size([760.0, 600.0])
        .frame(theme::dialog_frame(ctx).inner_margin(Margin::same(20)))
        .show(ctx, |ui| {
            let s = scheme(ui);
            ui.horizontal(|ui| {
                ui.add(
                    assets::card_image(target)
                        .fit_to_exact_size(vec2(40.0, 40.0 * CARD_IMAGE_RATIO))
                        .corner_radius(CornerRadius::same(4)),
                )
                .on_hover_ui(|ui| card_preview(ui, target));
                ui.vertical(|ui| {
                    ui.label(theme::title_medium(&target.name).color(s.on_surface));
                    ui.label(theme::label(&target.key).color(s.on_surface_variant));
                });
            });
            ui.label(
                theme::body_small("Click a card to see its details in the side panel.")
                    .color(s.on_surface_variant),
            );
            ui.add_space(8.0);
            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                for (title, cards, empty) in [
                    (
                        "Named in its text",
                        &view.named,
                        "Its text names no other card.",
                    ),
                    ("Naming it", &view.naming, "No card of its series names it."),
                ] {
                    theme::section_header(ui, title, Some(cards.len()));
                    if cards.is_empty() {
                        ui.label(theme::body_small(empty).color(s.on_surface_variant));
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                        for card in cards {
                            let selected = pinned.as_deref() == Some(card.key.as_str());
                            if related_thumbnail(ui, card, selected).clicked() {
                                clicked = Some(card.clone());
                            }
                        }
                    });
                    ui.add_space(12.0);
                }
            });
        });
    if !open {
        ctx.data_mut(|d| d.remove::<Arc<RelatedView>>(related_id()));
    } else if let Some(card) = clicked {
        ctx.data_mut(|d| d.insert_temp(pin_request_id(), card));
    }
}

/// Picture and name of a related card; hover previews it. `selected`: shown in the detail pane.
fn related_thumbnail(ui: &mut Ui, card: &Card, selected: bool) -> egui::Response {
    let width = 104.0;
    let s = scheme(ui);
    let inner = ui.allocate_ui(vec2(width, width * CARD_IMAGE_RATIO + 40.0), |ui| {
        ui.vertical(|ui| {
            ui.set_width(width);
            ui.add(
                assets::card_image(card)
                    .fit_to_exact_size(vec2(width, width * CARD_IMAGE_RATIO))
                    .corner_radius(CornerRadius::same(6)),
            );
            ui.add(egui::Label::new(theme::body_small(&card.name).color(s.on_surface)).truncate());
            ui.add(
                egui::Label::new(theme::body_small(&card.key).color(s.on_surface_variant))
                    .truncate(),
            );
        })
    });
    let r = ui
        .interact(inner.response.rect, ui.id().with(&card.key), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_ui(|ui| card_preview(ui, card));
    if selected || r.hovered() {
        let (width, color) = if selected {
            (3.0, s.primary)
        } else {
            (2.0, s.outline)
        };
        ui.painter().rect_stroke(
            inner.response.rect.expand(3.0),
            CornerRadius::same(8),
            egui::Stroke::new(width, color),
            egui::StrokeKind::Outside,
        );
    }
    r
}
