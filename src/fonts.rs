//! Fonts compiled into the program, shared by the window and the PDFs.
//!
//! `include_bytes!` copies the file into the executable at build time, so nothing needs to be
//! installed next to it.

pub const NOTO_SANS_REGULAR: &[u8] = include_bytes!("../fonts/NotoSans-Regular.ttf");
pub const NOTO_SANS_MEDIUM: &[u8] = include_bytes!("../fonts/NotoSans-Medium.ttf");
