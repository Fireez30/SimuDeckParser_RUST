//! Text sizes and weights (the Material 3 "type scale").

use egui::{FontFamily, FontId, RichText};

/// Regular weight at `size` points.
/// Name of the font family of the medium (semi-bold) weight; see `style::install`.
pub(super) const MEDIUM: &str = "medium";

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Medium (semi-bold) weight at `size` points.
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
