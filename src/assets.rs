//! Images embedded in the binary (formerly `resources.qrc`).

use egui::{ImageSource, include_image};
use simu_deck_parser::model::Trigger;

pub const EMPTY_CARD: ImageSource<'static> = include_image!("../images/emptycard.jpg");
pub const SOUL: ImageSource<'static> = include_image!("../images/soul.png");
pub const AUTO: ImageSource<'static> = include_image!("../images/auto.jpg");
pub const CONT: ImageSource<'static> = include_image!("../images/cont.jpg");
pub const ACT: ImageSource<'static> = include_image!("../images/act.jpg");
pub const CX_COMBO: ImageSource<'static> = include_image!("../images/ccx.png");

/// Note: `gate.png` draws a door and `door.png` a gate, hence the apparent swap.
pub fn trigger_icon(t: Trigger) -> ImageSource<'static> {
    match t {
        Trigger::Bag => include_image!("../images/goldbag.png"),
        Trigger::Bar => include_image!("../images/goldbar.png"),
        Trigger::Book => include_image!("../images/book.png"),
        Trigger::Burn => include_image!("../images/shot.png"),
        Trigger::Choice => include_image!("../images/switch.png"),
        Trigger::Pant => include_image!("../images/door.png"),
        Trigger::Salvage => include_image!("../images/gate.png"),
        Trigger::Soul => SOUL,
        Trigger::Standby => include_image!("../images/standby.png"),
        Trigger::Wind => include_image!("../images/bounce.png"),
    }
}

/// Card artwork, or the embedded placeholder.
pub fn card_image(card: &simu_deck_parser::model::Card) -> egui::Image<'static> {
    match &card.image {
        Some(path) => egui::Image::from_uri(file_uri(path)),
        None => egui::Image::new(EMPTY_CARD),
    }
}

/// `file:///home/me/x.jpg` on Linux, `file:///C:\\Games\\x.jpg` on Windows: egui reads
/// `file://C:\\…` as a network share, so the third slash matters.
fn file_uri(path: &std::path::Path) -> String {
    let path = path.display().to_string();
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::file_uri;
    use std::path::Path;

    #[test]
    fn file_uris() {
        assert_eq!(
            file_uri(Path::new("/home/me/a b.jpg")),
            "file:///home/me/a b.jpg"
        );
        assert_eq!(
            file_uri(Path::new(r"C:\Games\x.jpg")),
            r"file:///C:\Games\x.jpg"
        );
    }
}
