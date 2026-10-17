//! The app's look, following Material 3: colour roles, text styles, and the components egui
//! lacks (buttons, chips, navigation rail items, snackbar…).
//!
//! The pieces live in submodules; everything is re-exported here (`pub use`), so screens just
//! write `theme::filled(…)`, `theme::scheme(ui)`…

mod buttons;
mod colors;
mod style;
mod surfaces;
mod typography;
mod widgets;

pub use buttons::{
    chip, filled, icon_button, list_item, outlined, rail_item, small_icon_button, text, tonal,
};
pub use colors::{card_color, scheme};
pub use style::install;
pub use surfaces::{
    Snackbar, actions_row, confirm_dialog, detail_pane, dialog_frame, list_pane, surface_card,
};
pub use typography::{body_small, headline, label, medium, regular, title, title_medium};
pub use widgets::{
    banner, count_badge, empty_state, info_chip, search_bar, section_header, wrap_before,
};
