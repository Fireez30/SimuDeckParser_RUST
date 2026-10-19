//! Containers: cards, side panes, dialogs and the snackbar.

use egui::{Align2, Color32, CornerRadius, Margin, RichText, Shadow, Theme, Ui, vec2};

use super::buttons::{filled, text};
use super::colors::{scheme, scheme_for};
use super::typography::{headline, label};

/// Rounded surface (Material "card").
pub fn surface_card(fill: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(16))
}

/// The list on the left of the Decks and Encore screens: a resizable pane holding a rounded
/// card as tall as the window.
pub fn list_pane(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    let s = scheme(ui);
    egui::Panel::left(egui::Id::new(id))
        .resizable(true)
        .default_size(300.0)
        .size_range(240.0..=420.0)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
            left: 16,
            right: 8,
            top: 16,
            bottom: 16,
        }))
        .show(ui, |ui| {
            surface_card(s.surface_container_low)
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_min_height(ui.available_height());
                    add(ui);
                });
        });
}

/// The pane on the right that shows a card's details: resizable, holding a rounded card as tall
/// as the window. `top` is the margin above it (none under the card browser's top bar).
pub fn detail_pane(ui: &mut Ui, id: &str, top: i8, add: impl FnOnce(&mut Ui)) {
    let s = scheme(ui);
    egui::Panel::right(egui::Id::new(id))
        .resizable(true)
        .default_size(360.0)
        .size_range(280.0..=560.0)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
            left: 8,
            right: 16,
            top,
            bottom: 16,
        }))
        .show(ui, |ui| {
            surface_card(s.surface_container_low).show(ui, |ui| {
                ui.set_min_height(ui.available_height());
                add(ui);
            });
        });
}

/// Right-aligned row of dialog or form buttons. The first button added is the rightmost.
pub fn actions_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 40.0),
        egui::Layout::right_to_left(egui::Align::Center),
        add,
    );
}

/// A dialog asking to confirm leaving unsaved work: a title, a message, a `confirm` button
/// and "Keep editing". Call it on every frame while it should show. Returns `Some(true)` once
/// confirmed, `Some(false)` once cancelled (also by Esc or a click outside), `None` meanwhile.
pub fn confirm_dialog(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    message: &str,
    confirm: &str,
) -> Option<bool> {
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::new(id))
        .frame(dialog_frame(ctx))
        .show(ctx, |ui| {
            let s = scheme(ui);
            ui.set_width(400.0);
            ui.label(headline(title).color(s.on_surface));
            ui.add_space(8.0);
            ui.label(RichText::new(message).color(s.on_surface_variant));
            ui.add_space(20.0);
            actions_row(ui, |ui| {
                if filled(None, confirm).show(ui).clicked() {
                    answer = Some(true);
                }
                if text(None, "Keep editing").show(ui).clicked() {
                    answer = Some(false);
                }
            });
        });
    if modal.should_close() {
        answer = answer.or(Some(false));
    }
    answer
}

/// Frame for a dialog (Material "basic dialog").
pub fn dialog_frame(ui_ctx: &egui::Context) -> egui::Frame {
    let dark = ui_ctx.theme() == Theme::Dark;
    let s = scheme_for(dark);
    egui::Frame::new()
        .fill(s.surface_container_high)
        .corner_radius(CornerRadius::same(28))
        .inner_margin(Margin::same(24))
        .shadow(Shadow {
            offset: [0, 8],
            blur: 32,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 120 } else { 60 }),
        })
}

/// Transient message at the bottom of the window.
pub struct Snackbar {
    text: String,
    /// Time of the first frame it was shown.
    since: Option<f64>,
}

impl Snackbar {
    const DURATION: f64 = 6.0;

    pub fn new(text: String) -> Self {
        Self { text, since: None }
    }

    /// Draws the snackbar; returns `false` once it should be dropped.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let now = ctx.input(|i| i.time);
        let since = *self.since.get_or_insert(now);
        if now - since > Self::DURATION {
            return false;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        let s = scheme_for(ctx.theme() == Theme::Dark);
        let mut keep = true;
        egui::Area::new(egui::Id::new("snackbar"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -24.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(s.inverse_surface)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin {
                        left: 16,
                        right: 8,
                        top: 6,
                        bottom: 6,
                    })
                    .shadow(ui.visuals().popup_shadow)
                    .show(ui, |ui| {
                        ui.set_max_width(560.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&self.text).color(s.inverse_on_surface),
                                )
                                .wrap(),
                            );
                            let r = ui.add(
                                egui::Button::new(label("Dismiss").color(s.inverse_primary))
                                    .frame(false)
                                    .min_size(vec2(0.0, 36.0)),
                            );
                            if r.clicked() {
                                keep = false;
                            }
                        });
                    });
            });
        keep
    }
}
