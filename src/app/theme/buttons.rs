//! Clickable components: Material 3 buttons and chips, navigation rail items, list rows.
//!
//! egui's widgets are drawn "immediate mode": calling `show(ui)` draws the button for this
//! frame and returns a `Response` saying whether it was clicked, hovered…

use egui::{
    Align2, Color32, CornerRadius, FontId, Response, RichText, Sense, Stroke, TextStyle, Ui, Vec2,
    WidgetInfo, WidgetText, WidgetType, vec2,
};
use egui_material_icons::MaterialIcon;

use super::colors::{overlay, scheme};
use super::typography::{medium, regular};

/// The kinds of [`Button`].
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
