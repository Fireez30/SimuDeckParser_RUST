//! Colours: the Material 3 colour roles for the light and dark themes, and the cards' colours.
//!
//! Screens never hard-code colours: they ask for a role, e.g. `scheme(ui).primary`, and get
//! the right one for the current theme.

use egui::{Color32, Ui};
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

/// `0x6750A4` -> the colour with red 0x67, green 0x50 and blue 0xA4.
pub(super) const fn hex(rgb: u32) -> Color32 {
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
