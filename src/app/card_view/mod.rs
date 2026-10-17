//! Showing a card: its summary (name, stats, traits), its effect text, the pane that shows the
//! hovered card in full, and the related cards window.

mod detail_pane;
mod effect_text;
mod related;

use egui::{CornerRadius, Image, Margin, RichText, Ui, vec2};
use egui_material_icons::icons::{ICON_ADD, ICON_CONTENT_COPY, ICON_REMOVE};
use simu_deck_parser::model::{CARD_IMAGE_RATIO, Card, CardType};

use super::assets;
use super::card_browser::Picker;
use super::theme::{self, scheme};

pub use detail_pane::DetailPane;
pub use related::{pin_requested, related_window, set_names};

/// Name, code, stats as chips and traits.
pub fn card_summary(ui: &mut Ui, card: &Card) {
    summary(ui, card, true);
}

/// Hover preview (deck builder): picture, summary and text, with nothing clickable, so the
/// tooltip never catches clicks meant for the cards around it.
pub fn card_preview(ui: &mut Ui, card: &Card) {
    ui.set_max_width(300.0);
    ui.add(
        assets::card_image(card)
            .fit_to_exact_size(vec2(260.0, 260.0 * CARD_IMAGE_RATIO))
            .corner_radius(CornerRadius::same(10)),
    );
    ui.add_space(6.0);
    summary(ui, card, false);
    effect_text::effect_text(ui, card, false);
}

fn summary(ui: &mut Ui, card: &Card, copy_button: bool) {
    let s = scheme(ui);
    let dark = ui.visuals().dark_mode;
    ui.add(egui::Label::new(theme::title(&card.name).color(s.on_surface)).wrap());
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&card.key)
                .font(theme::medium(13.0))
                .color(s.on_surface_variant),
        );
        if copy_button
            && theme::small_icon_button(ui, ICON_CONTENT_COPY, "Copy card code").clicked()
        {
            ui.ctx().copy_text(card.key.clone());
        }
    });
    ui.add_space(4.0);

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        theme::info_chip(
            ui,
            None,
            card.color.label(),
            Some(theme::card_color(card.color, dark)),
        );
        theme::info_chip(ui, None, card.card_type.label(), None);
        if card.card_type != CardType::Climax {
            theme::info_chip(ui, None, &format!("Level {}", card.level), None);
            theme::info_chip(ui, None, &format!("Cost {}", card.cost), None);
        }
        if card.card_type == CardType::Character {
            theme::info_chip(ui, None, &format!("Power {}", card.power), None);
        }
    });

    if !card.triggers.is_empty() || card.soul_count > 0 {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if !card.triggers.is_empty() {
                ui.label(theme::label("Triggers").color(s.on_surface_variant));
                for t in &card.triggers {
                    ui.add(
                        Image::new(assets::trigger_icon(*t)).fit_to_exact_size(vec2(16.0, 20.0)),
                    )
                    .on_hover_text(t.label());
                }
                ui.add_space(12.0);
            }
            if card.soul_count > 0 {
                ui.label(theme::label("Soul").color(s.on_surface_variant));
                for _ in 0..card.soul_count {
                    ui.add(Image::new(assets::SOUL).fit_to_exact_size(vec2(16.0, 20.0)));
                }
            }
        });
    }

    if !card.traits.is_empty() {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for t in &card.traits {
                egui::Frame::new()
                    .stroke(egui::Stroke::new(1.0, s.outline_variant))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.label(theme::label(t).color(s.on_surface_variant));
                    });
            }
        });
    }
}

/// Effect text, with keyword icons and hoverable card names.
pub fn card_text(ui: &mut Ui, card: &Card) {
    effect_text::effect_text(ui, card, true);
}

/// "Add" / "Remove" buttons while building a deck: the detail pane and the list view.
pub fn pick_buttons(ui: &mut Ui, picker: &mut dyn Picker, card: &Card) {
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
