//! Material 3 look for egui: colour roles, type scale, and the few components egui lacks
//! (buttons, chips, navigation rail items, snackbar).

use std::sync::Arc;

use egui::{
    Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Response,
    RichText, Sense, Shadow, Stroke, TextStyle, Theme, Ui, Vec2, WidgetInfo, WidgetText,
    WidgetType, vec2,
};
use egui_material_icons::MaterialIcon;
use simu_deck_parser::model::Color;

/// Material 3 colour roles (baseline scheme). Not every role is used yet.
#[allow(dead_code)]
pub struct Scheme {
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,
    pub tertiary_container: Color32,
    pub on_tertiary_container: Color32,
    pub error: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,
    pub surface: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub surface_container_lowest: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub inverse_surface: Color32,
    pub inverse_on_surface: Color32,
    pub inverse_primary: Color32,
    pub success: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const LIGHT: Scheme = Scheme {
    primary: hex(0x6750A4),
    on_primary: hex(0xFFFFFF),
    primary_container: hex(0xEADDFF),
    on_primary_container: hex(0x4F378B),
    secondary_container: hex(0xE8DEF8),
    on_secondary_container: hex(0x4A4458),
    tertiary_container: hex(0xFFD8E4),
    on_tertiary_container: hex(0x633B48),
    error: hex(0xB3261E),
    error_container: hex(0xF9DEDC),
    on_error_container: hex(0x8C1D18),
    surface: hex(0xFEF7FF),
    on_surface: hex(0x1D1B20),
    on_surface_variant: hex(0x49454F),
    surface_container_lowest: hex(0xFFFFFF),
    surface_container_low: hex(0xF7F2FA),
    surface_container: hex(0xF3EDF7),
    surface_container_high: hex(0xECE6F0),
    surface_container_highest: hex(0xE6E0E9),
    outline: hex(0x79747E),
    outline_variant: hex(0xCAC4D0),
    inverse_surface: hex(0x322F35),
    inverse_on_surface: hex(0xF5EFF7),
    inverse_primary: hex(0xD0BCFF),
    success: hex(0x2E7D32),
};

pub const DARK: Scheme = Scheme {
    primary: hex(0xD0BCFF),
    on_primary: hex(0x381E72),
    primary_container: hex(0x4F378B),
    on_primary_container: hex(0xEADDFF),
    secondary_container: hex(0x4A4458),
    on_secondary_container: hex(0xE8DEF8),
    tertiary_container: hex(0x633B48),
    on_tertiary_container: hex(0xFFD8E4),
    error: hex(0xF2B8B5),
    error_container: hex(0x8C1D18),
    on_error_container: hex(0xF9DEDC),
    surface: hex(0x141218),
    on_surface: hex(0xE6E0E9),
    on_surface_variant: hex(0xCAC4D0),
    surface_container_lowest: hex(0x0F0D13),
    surface_container_low: hex(0x1D1B20),
    surface_container: hex(0x211F26),
    surface_container_high: hex(0x2B2930),
    surface_container_highest: hex(0x36343B),
    outline: hex(0x938F99),
    outline_variant: hex(0x49454F),
    inverse_surface: hex(0xE6E0E9),
    inverse_on_surface: hex(0x322F35),
    inverse_primary: hex(0x6750A4),
    success: hex(0x81C995),
};

pub fn scheme(ui: &Ui) -> &'static Scheme {
    scheme_for(ui.visuals().dark_mode)
}

pub fn scheme_for(dark: bool) -> &'static Scheme {
    if dark { &DARK } else { &LIGHT }
}

/// `base` with `layer` laid over it at `opacity` (Material state layers).
pub fn overlay(base: Color32, layer: Color32, opacity: f32) -> Color32 {
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * opacity).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(base.r(), layer.r()),
        mix(base.g(), layer.g()),
        mix(base.b(), layer.b()),
        base.a().max((layer.a() as f32 * opacity) as u8),
    )
}

/// Weiss Schwarz card colours, tuned for contrast on either theme.
pub fn card_color(c: Color, dark: bool) -> Color32 {
    match (c, dark) {
        (Color::Yellow, false) => hex(0xC79100),
        (Color::Yellow, true) => hex(0xFFD54F),
        (Color::Blue, false) => hex(0x1E63D6),
        (Color::Blue, true) => hex(0x82B1FF),
        (Color::Red, false) => hex(0xD32F2F),
        (Color::Red, true) => hex(0xFF8A80),
        (Color::Green, false) => hex(0x2E7D32),
        (Color::Green, true) => hex(0x81C995),
        (Color::Purple, false) => hex(0x8E24AA),
        (Color::Purple, true) => hex(0xE1A6F0),
        (Color::All | Color::None, _) => hex(0x9E9E9E),
    }
}

// ----------------------------------------------------------------------------------------------
// Typography

const MEDIUM: &str = "medium";

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

pub fn headline(text: impl Into<String>) -> RichText {
    RichText::new(text).font(regular(26.0))
}

pub fn title(text: impl Into<String>) -> RichText {
    RichText::new(text).font(regular(20.0))
}

pub fn title_medium(text: impl Into<String>) -> RichText {
    RichText::new(text).font(medium(15.0))
}

pub fn label(text: impl Into<String>) -> RichText {
    RichText::new(text).font(medium(13.0))
}

pub fn body_small(text: impl Into<String>) -> RichText {
    RichText::new(text).font(regular(12.5))
}

// ----------------------------------------------------------------------------------------------
// Installation

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto".into(),
        Arc::new(FontData::from_static(simu_deck_parser::pdf::FONT_REGULAR)),
    );
    fonts.font_data.insert(
        "noto-medium".into(),
        Arc::new(FontData::from_static(simu_deck_parser::pdf::FONT_MEDIUM)),
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

// ----------------------------------------------------------------------------------------------
// Components

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Filled,
    Tonal,
    Outlined,
    Text,
    /// Standard icon button; `selected` tints it with the primary colour.
    Icon,
    /// Filter chip: outlined, filled with a check mark when selected.
    Chip,
}

/// A Material 3 button. Build with the helpers ([`filled`], [`tonal`], …) and add with [`Button::show`].
pub struct Button<'a> {
    kind: Kind,
    icon: Option<MaterialIcon>,
    text: Option<&'a str>,
    selected: bool,
    enabled: bool,
    dot: Option<Color32>,
    tooltip: Option<&'a str>,
    min_width: f32,
}

pub fn filled<'a>(icon: Option<MaterialIcon>, text: &'a str) -> Button<'a> {
    Button::new(Kind::Filled, icon, Some(text))
}

pub fn tonal<'a>(icon: Option<MaterialIcon>, text: &'a str) -> Button<'a> {
    Button::new(Kind::Tonal, icon, Some(text))
}

pub fn outlined<'a>(icon: Option<MaterialIcon>, text: &'a str) -> Button<'a> {
    Button::new(Kind::Outlined, icon, Some(text))
}

pub fn text<'a>(icon: Option<MaterialIcon>, text: &'a str) -> Button<'a> {
    Button::new(Kind::Text, icon, Some(text))
}

pub fn icon_button(icon: MaterialIcon, tooltip: &str) -> Button<'_> {
    let mut b = Button::new(Kind::Icon, Some(icon), None);
    b.tooltip = Some(tooltip);
    b
}

pub fn chip(text: &str, selected: bool) -> Button<'_> {
    let mut b = Button::new(Kind::Chip, None, Some(text));
    b.selected = selected;
    b
}

impl<'a> Button<'a> {
    fn new(kind: Kind, icon: Option<MaterialIcon>, text: Option<&'a str>) -> Self {
        Self {
            kind,
            icon,
            text,
            selected: false,
            enabled: true,
            dot: None,
            tooltip: None,
            min_width: 0.0,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Leading colour dot (chips).
    pub fn dot(mut self, color: Color32) -> Self {
        self.dot = Some(color);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let s = scheme(ui);
        let chip = self.kind == Kind::Chip;
        let icon_only = self.text.is_none();
        let height = if chip { 32.0 } else { 40.0 };
        let icon_size = if chip { 18.0 } else { 20.0 };

        let (bg, fg, stroke) = match (self.kind, self.selected) {
            (Kind::Filled, _) => (s.primary, s.on_primary, None),
            (Kind::Tonal, _) => (s.secondary_container, s.on_secondary_container, None),
            (Kind::Outlined, _) => (Color32::TRANSPARENT, s.primary, Some(s.outline)),
            (Kind::Text, _) => (Color32::TRANSPARENT, s.primary, None),
            (Kind::Icon, false) => (Color32::TRANSPARENT, s.on_surface_variant, None),
            (Kind::Icon, true) => (s.secondary_container, s.on_secondary_container, None),
            (Kind::Chip, false) => (
                Color32::TRANSPARENT,
                s.on_surface_variant,
                Some(s.outline_variant),
            ),
            (Kind::Chip, true) => (s.secondary_container, s.on_secondary_container, None),
        };
        let (bg, fg, stroke) = if self.enabled {
            (bg, fg, stroke)
        } else {
            let faded = s.on_surface.gamma_multiply(0.38);
            let bg = if bg == Color32::TRANSPARENT {
                bg
            } else {
                s.on_surface.gamma_multiply(0.12)
            };
            (bg, faded, stroke.map(|_| s.on_surface.gamma_multiply(0.12)))
        };

        let leading_icon = match (self.kind, self.selected) {
            (Kind::Chip, true) => Some(egui_material_icons::icons::ICON_CHECK),
            _ => self.icon,
        };
        let icon_galley = leading_icon.map(|i| {
            WidgetText::from(i.rich_text().size(icon_size)).into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Button,
            )
        });
        let text_galley = self.text.map(|t| {
            WidgetText::from(RichText::new(t).font(medium(if chip { 13.0 } else { 14.0 })))
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    TextStyle::Button,
                )
        });

        let pad = if icon_only {
            (height - icon_size) / 2.0
        } else if chip {
            if leading_icon.is_some() || self.dot.is_some() {
                8.0
            } else {
                12.0
            }
        } else if leading_icon.is_some() {
            16.0
        } else {
            20.0
        };
        let gap = 8.0;
        let mut width = pad * 2.0;
        if let Some(g) = &icon_galley {
            width += icon_size.max(g.size().x);
        }
        if self.dot.is_some() {
            width += 10.0;
        }
        if let Some(g) = &text_galley {
            if icon_galley.is_some() || self.dot.is_some() {
                width += gap;
            }
            width += g.size().x;
            if (chip && leading_icon.is_some()) || self.dot.is_some() {
                width += 4.0; // trailing padding of a chip with a leading element
            }
        }
        let size = vec2(width.max(self.min_width), height);
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);
        let label = self.text.or(self.tooltip).unwrap_or_default().to_string();
        response.widget_info(|| {
            if chip || self.kind == Kind::Icon {
                WidgetInfo::selected(WidgetType::Button, self.enabled, self.selected, &label)
            } else {
                WidgetInfo::labeled(WidgetType::Button, self.enabled, &label)
            }
        });

        if ui.is_rect_visible(rect) {
            let radius = if chip {
                CornerRadius::same(8)
            } else {
                CornerRadius::same((height / 2.0) as u8)
            };
            let painter = ui.painter();
            let mut fill = bg;
            if self.enabled {
                let layer = if response.is_pointer_button_down_on() {
                    0.12
                } else if response.hovered() || response.has_focus() {
                    0.08
                } else {
                    0.0
                };
                if layer > 0.0 {
                    fill = if bg == Color32::TRANSPARENT {
                        fg.gamma_multiply(layer)
                    } else {
                        overlay(bg, fg, layer)
                    };
                }
            }
            painter.rect(
                rect,
                radius,
                fill,
                stroke.map_or(Stroke::NONE, |c| Stroke::new(1.0, c)),
                egui::StrokeKind::Inside,
            );
            if response.has_focus() {
                painter.rect_stroke(
                    rect.expand(2.0),
                    radius,
                    Stroke::new(2.0, s.primary),
                    egui::StrokeKind::Outside,
                );
            }

            let content_width = width - pad * 2.0;
            let mut x = rect.center().x - content_width / 2.0;
            let cy = rect.center().y;
            if let Some(color) = self.dot {
                painter.circle_filled(egui::pos2(x + 5.0, cy), 5.0, color);
                x += 10.0 + gap;
            }
            if let Some(g) = icon_galley {
                let w = icon_size.max(g.size().x);
                painter.galley(
                    egui::pos2(x + (w - g.size().x) / 2.0, cy - g.size().y / 2.0),
                    g,
                    fg,
                );
                x += w + gap;
            }
            if let Some(g) = text_galley {
                painter.galley(egui::pos2(x, cy - g.size().y / 2.0), g, fg);
            }
        }

        let response = if self.enabled {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response
        };
        match self.tooltip {
            Some(t) if self.text.is_none() || self.tooltip != self.text => {
                response.on_hover_text(t)
            }
            _ => response,
        }
    }
}

/// Navigation rail destination: pill indicator around the icon, label underneath.
pub fn rail_item(ui: &mut Ui, icon: MaterialIcon, text: &str, selected: bool) -> Response {
    let s = scheme(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(80.0, 60.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, text));
    let indicator = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 18.0),
        vec2(56.0, 32.0),
    );
    let painter = ui.painter();
    let mut fill = if selected {
        s.secondary_container
    } else {
        Color32::TRANSPARENT
    };
    if response.hovered() {
        fill = if selected {
            overlay(fill, s.on_secondary_container, 0.08)
        } else {
            s.on_surface.gamma_multiply(0.08)
        };
    }
    painter.rect_filled(indicator, CornerRadius::same(16), fill);
    let (icon_color, icon) = if selected {
        (s.on_secondary_container, icon.filled())
    } else {
        (s.on_surface_variant, icon.outlined())
    };
    painter.text(
        indicator.center(),
        Align2::CENTER_CENTER,
        icon.codepoint,
        FontId::new(22.0, icon.font_family()),
        icon_color,
    );
    painter.text(
        egui::pos2(rect.center().x, indicator.bottom() + 4.0),
        Align2::CENTER_TOP,
        text,
        medium(12.0),
        if selected {
            s.on_surface
        } else {
            s.on_surface_variant
        },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Selectable list row (navigation drawer style): a headline and optional supporting text.
pub fn list_item(ui: &mut Ui, headline: &str, supporting: &str, selected: bool) -> Response {
    let s = scheme(ui);
    let width = ui.available_width();
    let height = if supporting.is_empty() { 44.0 } else { 56.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, headline));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    let (fill, fg) = if selected {
        (s.secondary_container, s.on_secondary_container)
    } else {
        (Color32::TRANSPARENT, s.on_surface)
    };
    let fill = if response.hovered() {
        if selected {
            overlay(fill, fg, 0.08)
        } else {
            s.on_surface.gamma_multiply(0.08)
        }
    } else {
        fill
    };
    painter.rect_filled(rect, CornerRadius::same((height / 2.0) as u8), fill);
    let text_rect = rect.shrink2(vec2(16.0, 0.0));
    let clip = painter.with_clip_rect(text_rect.intersect(painter.clip_rect()));
    let headline_galley = painter.layout_no_wrap(headline.to_string(), medium(14.0), fg);
    if supporting.is_empty() {
        clip.galley(
            egui::pos2(
                text_rect.left(),
                rect.center().y - headline_galley.size().y / 2.0,
            ),
            headline_galley,
            fg,
        );
    } else {
        let supporting_color = if selected {
            fg.gamma_multiply(0.8)
        } else {
            s.on_surface_variant
        };
        clip.galley(
            egui::pos2(text_rect.left(), rect.top() + 8.0),
            headline_galley,
            fg,
        );
        clip.text(
            egui::pos2(text_rect.left(), rect.bottom() - 8.0),
            Align2::LEFT_BOTTOM,
            supporting,
            regular(12.0),
            supporting_color,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

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

/// Compact 32px icon button for dense spots (search clear, pane headers).
pub fn small_icon_button(ui: &mut Ui, icon: MaterialIcon, tooltip: &str) -> Response {
    let s = scheme(ui);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, tooltip));
    if response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), 16.0, s.on_surface.gamma_multiply(0.08));
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon.codepoint,
        FontId::new(20.0, icon.font_family()),
        s.on_surface_variant,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

/// Rounded surface (Material "card").
pub fn surface_card(fill: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(16))
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
