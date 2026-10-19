//! Small display components: search bar, chips, banners, headers, badges.

use egui::{Align2, Color32, CornerRadius, Margin, Response, RichText, Sense, Ui, Vec2, vec2};
use egui_material_icons::MaterialIcon;

use super::buttons::small_icon_button;
use super::colors::scheme;
use super::typography::{label, medium, title, title_medium};

/// Pill-shaped search field with a leading icon and a clear button. Returns the text edit response.
pub fn search_bar(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let s = scheme(ui);
    let mut edit_response = None;
    egui::Frame::new()
        .fill(s.surface_container_high)
        .corner_radius(CornerRadius::same(24))
        .inner_margin(Margin {
            left: 12,
            right: 4,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui_material_icons::icons::ICON_SEARCH
                        .rich_text()
                        .size(20.0)
                        .color(s.on_surface_variant),
                );
                let clear_width = if text.is_empty() { 0.0 } else { 36.0 };
                let r = ui.add(
                    egui::TextEdit::singleline(text)
                        .hint_text(hint)
                        .frame(egui::Frame::NONE)
                        .background_color(Color32::TRANSPARENT)
                        .margin(Margin::symmetric(4, 8))
                        .desired_width(ui.available_width() - clear_width),
                );
                if !text.is_empty()
                    && small_icon_button(ui, egui_material_icons::icons::ICON_CLOSE, "Clear search")
                        .clicked()
                {
                    text.clear();
                    let mut r = r.clone();
                    r.mark_changed();
                    edit_response = Some(r);
                    return;
                }
                edit_response = Some(r);
            });
        });
    edit_response.expect("search bar content always runs")
}

/// Wrapping layouts don't wrap framed widgets: starts a new row when a chip showing `text`
/// (as a [`label`]) plus `extra` width would overflow the current one.
pub fn wrap_before(ui: &mut Ui, text: &str, extra: f32) {
    if !ui.layout().main_wrap {
        return;
    }
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), medium(13.0), Color32::WHITE);
    let row_start = ui.cursor().min.x <= ui.max_rect().min.x + 1.0;
    if !row_start && ui.available_size_before_wrap().x < galley.size().x + extra {
        ui.end_row();
    }
}

/// Small non-interactive label chip (e.g. "Level 2").
pub fn info_chip(ui: &mut Ui, icon: Option<MaterialIcon>, text: &str, dot: Option<Color32>) {
    let s = scheme(ui);
    let extras = [dot.is_some().then_some(16.0), icon.map(|_| 22.0)];
    wrap_before(ui, text, 20.0 + extras.iter().flatten().sum::<f32>());
    egui::Frame::new()
        .fill(s.surface_container_high)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                if let Some(color) = dot {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 5.0, color);
                }
                if let Some(i) = icon {
                    ui.label(i.rich_text().size(16.0).color(s.on_surface_variant));
                }
                ui.label(label(text).color(s.on_surface));
            });
        });
}

/// Banner for warnings and errors inside a page.
pub fn banner(ui: &mut Ui, icon: MaterialIcon, text: &str) {
    let s = scheme(ui);
    egui::Frame::new()
        .fill(s.error_container)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icon.rich_text().size(20.0).color(s.on_error_container));
                ui.add(egui::Label::new(RichText::new(text).color(s.on_error_container)).wrap());
            });
        });
}

/// Centered empty state: large icon, title and hint.
pub fn empty_state(ui: &mut Ui, icon: MaterialIcon, title_text: &str, hint: &str) {
    let s = scheme(ui);
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() / 2.0 - 90.0).max(16.0));
        ui.label(icon.outlined().rich_text().size(56.0).color(s.outline));
        ui.add_space(8.0);
        ui.label(title(title_text).color(s.on_surface));
        ui.add_space(4.0);
        ui.label(RichText::new(hint).color(s.on_surface_variant));
    });
}

/// Section heading with an optional trailing count.
pub fn section_header(ui: &mut Ui, text: &str, count: Option<usize>) {
    let s = scheme(ui);
    ui.horizontal(|ui| {
        ui.label(title_medium(text).color(s.on_surface));
        if let Some(n) = count {
            ui.label(label(n.to_string()).color(s.on_surface_variant));
        }
    });
}

/// "×4" badge in the bottom-right corner of a card picture.
pub fn count_badge(ui: &Ui, card_rect: egui::Rect, copies: usize) {
    let s = scheme(ui);
    let center = card_rect.right_bottom() + vec2(-18.0, -18.0);
    let rect = egui::Rect::from_center_size(center, vec2(32.0, 24.0));
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(12), s.primary);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("×{copies}"),
        medium(13.0),
        s.on_primary,
    );
}
