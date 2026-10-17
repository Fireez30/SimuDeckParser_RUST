//! Program entry point: opens the window and hands it to [`app::App`].
//!
//! The window's code is in `src/app/`; the rest of the program (reading the simulator's files,
//! Encore Decks, PDFs…) is the library in `src/lib.rs`, used here as `simu_deck_parser::…`.

// Release builds on Windows open no console window next to the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SimuDeckParser")
            .with_inner_size([1600.0, 1000.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SimuDeckParser",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
