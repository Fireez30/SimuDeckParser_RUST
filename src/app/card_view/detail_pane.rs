//! The side pane showing one card in full: the hovered one, or the one pinned by a click.

use egui::{CornerRadius, ScrollArea, Ui, vec2};
use egui_material_icons::icons::{ICON_KEEP, ICON_KEEP_OFF, ICON_STYLE, ICON_TOUCH_APP};
use simu_deck_parser::model::{CARD_IMAGE_RATIO, Card};

use super::related::{pin_request_id, pinned_id, related_button};
use super::{card_summary, card_text, pick_buttons};
use crate::app::assets;
use crate::app::card_browser::Picker;
use crate::app::theme::{self, scheme};

/// Side pane showing the hovered card, or the pinned one (click pins, Esc or the pin button unpins).
/// While building a deck, `picker` turns it into a card panel that also adds and removes copies.
#[derive(Default)]
pub struct DetailPane {
    card: Option<Card>,
    pinned: bool,
}

impl DetailPane {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn pinned_key(&self) -> Option<&str> {
        self.card
            .as_ref()
            .filter(|_| self.pinned)
            .map(|c| c.key.as_str())
    }

    pub fn hover(&mut self, card: &Card) {
        if !self.pinned && self.card.as_ref().is_none_or(|c| c.key != card.key) {
            self.card = Some(card.clone());
        }
    }

    /// Clicking the pinned card again unpins it.
    pub fn click(&mut self, card: &Card) {
        if self.pinned_key() == Some(&card.key) {
            self.pinned = false;
        } else {
            self.card = Some(card.clone());
            self.pinned = true;
        }
    }

    /// The picker's object lifetime is its own (`'o`), not the reference's: reborrowing a
    /// `&mut dyn Picker` through a closure must not borrow it for the object's whole lifetime.
    pub fn show<'o>(&mut self, ui: &mut Ui, picker: Option<&mut (dyn Picker + 'o)>) {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pinned = false;
        }
        // A card clicked in the related cards window.
        let request = ui.ctx().data_mut(|d| {
            let card = d.get_temp::<Card>(pin_request_id());
            d.remove::<Card>(pin_request_id());
            card
        });
        if let Some(card) = request {
            self.card = Some(card);
            self.pinned = true;
        }
        let pinned = self.pinned_key().map(str::to_owned);
        ui.ctx().data_mut(|d| match pinned {
            Some(key) => {
                d.insert_temp(pinned_id(), key);
            }
            None => d.remove::<String>(pinned_id()),
        });
        let s = scheme(ui);
        let Some(card) = &self.card else {
            theme::empty_state(
                ui,
                ICON_TOUCH_APP,
                "No card selected",
                if picker.is_some() {
                    "Hover a card to preview it.\nAdd or remove copies from here."
                } else {
                    "Hover a card to preview it.\nClick to pin it here."
                },
            );
            return;
        };
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (text, icon) = if self.pinned {
                    ("Pinned", ICON_KEEP)
                } else {
                    ("Preview", ICON_STYLE)
                };
                ui.label(icon.rich_text().size(18.0).color(s.on_surface_variant));
                ui.label(theme::label(text).color(s.on_surface_variant));
                if self.pinned {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::small_icon_button(ui, ICON_KEEP_OFF, "Unpin (Esc)").clicked() {
                            self.pinned = false;
                        }
                    });
                }
            });
            ui.add_space(4.0);
            let width = ui.available_width().min(340.0);
            ui.vertical_centered(|ui| {
                ui.add(
                    assets::card_image(card)
                        .fit_to_exact_size(vec2(width, width * CARD_IMAGE_RATIO))
                        .corner_radius(CornerRadius::same(12)),
                );
            });
            ui.add_space(12.0);
            card_summary(ui, card);
            ui.add_space(8.0);
            if let Some(picker) = picker {
                pick_buttons(ui, picker, card);
            }
            ui.add_space(8.0);
            related_button(ui, card);
            card_text(ui, card);
        });
    }
}
