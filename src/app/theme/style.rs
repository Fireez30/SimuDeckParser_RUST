//! The global look: fonts, spacing and egui's own widgets, for the light and dark themes.

use std::sync::Arc;

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke,
    TextStyle, Theme, vec2,
};
use simu_deck_parser::fonts::{NOTO_SANS_MEDIUM, NOTO_SANS_REGULAR};

use super::colors::{DARK, LIGHT, Scheme, hex, overlay};
use super::typography::{MEDIUM, medium, regular};

/// Sets up fonts, icons and both themes' styles. Called once, when the window opens.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto".into(),
        Arc::new(FontData::from_static(NOTO_SANS_REGULAR)),
    );
    fonts.font_data.insert(
        "noto-medium".into(),
        Arc::new(FontData::from_static(NOTO_SANS_MEDIUM)),
    );
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "noto".into());
    let mut medium_family = vec!["noto-medium".to_string()];
    medium_family.extend(proportional.iter().skip(1).cloned());
    fonts
        .families
        .insert(FontFamily::Name(MEDIUM.into()), medium_family);
    ctx.set_fonts(fonts);
    egui_material_icons::initialize(ctx);

    ctx.set_style_of(Theme::Light, style(&LIGHT, false));
    ctx.set_style_of(Theme::Dark, style(&DARK, true));
}

/// egui's look for one theme: spacing, corner radii, and the colour of every widget state.
fn style(s: &Scheme, dark: bool) -> egui::Style {
    let mut style = egui::Style {
        text_styles: [
            (TextStyle::Small, regular(12.0)),
            (TextStyle::Body, regular(14.0)),
            (TextStyle::Button, medium(14.0)),
            (TextStyle::Heading, regular(22.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into(),
        ..Default::default()
    };

    let sp = &mut style.spacing;
    sp.item_spacing = vec2(8.0, 8.0);
    sp.button_padding = vec2(12.0, 6.0);
    sp.interact_size = vec2(40.0, 28.0);
    sp.menu_margin = Margin::same(8);
    sp.window_margin = Margin::same(24);
    sp.combo_height = 400.0;
    sp.scroll = egui::style::ScrollStyle::floating();
    sp.scroll.bar_width = 8.0;

    style.interaction.tooltip_delay = 0.4;

    let v = &mut style.visuals;
    *v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = s.surface;
    v.window_fill = s.surface_container_high;
    v.window_stroke = Stroke::NONE;
    v.window_corner_radius = CornerRadius::same(28);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 110 } else { 50 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 3],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 40 }),
    };
    v.extreme_bg_color = s.surface_container_highest;
    v.text_edit_bg_color = Some(s.surface_container_highest);
    v.faint_bg_color = s.surface_container_low;
    v.code_bg_color = s.surface_container_high;
    v.hyperlink_color = s.primary;
    v.warn_fg_color = if dark { hex(0xFFB74D) } else { hex(0xB26A00) };
    v.error_fg_color = s.error;
    v.selection.bg_fill = s.primary.gamma_multiply(if dark { 0.35 } else { 0.25 });
    v.selection.stroke = Stroke::new(1.5, s.primary);
    v.text_cursor.stroke = Stroke::new(2.0, s.primary);
    v.striped = false;
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = false;

    let radius = CornerRadius::same(20);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = s.surface;
    w.noninteractive.weak_bg_fill = s.surface_container;
    w.noninteractive.bg_stroke = Stroke::new(1.0, s.outline_variant);
    w.noninteractive.fg_stroke = Stroke::new(1.0, s.on_surface);
    w.noninteractive.corner_radius = CornerRadius::same(12);

    w.inactive.bg_fill = s.surface_container_highest;
    w.inactive.weak_bg_fill = s.surface_container_high;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, s.on_surface);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = overlay(s.surface_container_highest, s.on_surface, 0.08);
    w.hovered.weak_bg_fill = overlay(s.surface_container_high, s.on_surface, 0.08);
    w.hovered.bg_stroke = Stroke::NONE;
    w.hovered.fg_stroke = Stroke::new(1.5, s.on_surface);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = overlay(s.surface_container_highest, s.on_surface, 0.12);
    w.active.weak_bg_fill = overlay(s.surface_container_high, s.on_surface, 0.12);
    w.active.bg_stroke = Stroke::new(1.0, s.primary);
    w.active.fg_stroke = Stroke::new(2.0, s.primary);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open.bg_fill = s.surface_container_highest;
    w.open.weak_bg_fill = s.secondary_container;
    w.open.bg_stroke = Stroke::NONE;
    w.open.fg_stroke = Stroke::new(1.0, s.on_secondary_container);
    w.open.corner_radius = radius;
    style
}
