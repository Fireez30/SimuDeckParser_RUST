//! The Settings screen: theme, simulator folder, decks folder, Encore Decks account, version.
//!
//! The methods here belong to [`App`] (see `setup_screen.rs` for why that works).

use egui::{RichText, ScrollArea, Ui};
use egui_material_icons::icons::{
    ICON_BRIGHTNESS_AUTO, ICON_DARK_MODE, ICON_FOLDER_OPEN, ICON_LIGHT_MODE, ICON_REFRESH,
};

use super::theme::{self, scheme};
use super::{App, ThemeChoice};

impl App {
    pub(super) fn settings_screen(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let width = ui.available_width().min(760.0);
            ui.set_max_width(width);
            ui.label(theme::headline("Settings").color(s.on_surface));
            ui.add_space(16.0);
            for section in [
                Self::appearance_card,
                Self::simulator_card,
                Self::decks_folder_card,
                Self::account_card,
                Self::about_card,
            ] {
                theme::surface_card(s.surface_container_low).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section(self, ui);
                });
                ui.add_space(12.0);
            }
        });
    }

    fn appearance_card(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ui.label(theme::title_medium("Appearance").color(s.on_surface));
        ui.label(RichText::new("Theme").color(s.on_surface_variant));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (choice, _, label) in ThemeChoice::ALL {
                let icon = match choice {
                    ThemeChoice::System => ICON_BRIGHTNESS_AUTO,
                    ThemeChoice::Light => ICON_LIGHT_MODE,
                    ThemeChoice::Dark => ICON_DARK_MODE,
                };
                let button = if self.theme == choice {
                    theme::tonal(Some(icon), label)
                } else {
                    theme::outlined(Some(icon), label)
                };
                if button.min_width(120.0).show(ui).clicked() {
                    self.set_theme(ui.ctx(), choice);
                }
            }
        });
    }

    /// The loaded simulator, with buttons to read its cards again or pick another folder.
    fn simulator_card(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ui.label(theme::title_medium("Simulator").color(s.on_surface));
        let path = self
            .folder
            .as_ref()
            .map(|f| f.root.display().to_string())
            .unwrap_or_default();
        ui.add(
            egui::Label::new(RichText::new(&path).monospace().color(s.on_surface_variant))
                .wrap()
                .selectable(true),
        );
        let sets: usize = self.series.iter().map(|s| s.sets.len()).sum();
        let cards: usize = self
            .series
            .iter()
            .flat_map(|s| &s.sets)
            .map(|s| s.cards.len())
            .sum();
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            theme::info_chip(ui, None, &format!("{} series", self.series.len()), None);
            theme::info_chip(ui, None, &format!("{sets} sets"), None);
            theme::info_chip(ui, None, &format!("{cards} cards"), None);
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let reload = theme::tonal(Some(ICON_REFRESH), "Reload cards")
                .enabled(self.loading.is_none())
                .show(ui)
                .on_hover_text("Read the card data again, e.g. after a simulator update");
            if reload.clicked() {
                self.setup.set_path(path.clone());
                self.load_simulator(ui.ctx(), false);
            }
            if self.loading.is_some() {
                ui.spinner();
            }
            let change = theme::outlined(Some(ICON_FOLDER_OPEN), "Change folder…")
                .show(ui)
                .on_hover_text("Forget this folder and pick another one");
            if change.clicked() {
                self.unload_simulator(ui.ctx());
            }
        });
    }

    fn decks_folder_card(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ui.label(theme::title_medium("Simulator decks").color(s.on_surface));
        let decks = self.folder.as_ref().map(|f| f.decks()).unwrap_or_default();
        ui.add(
            egui::Label::new(
                RichText::new(decks.display().to_string())
                    .monospace()
                    .color(s.on_surface_variant),
            )
            .wrap()
            .selectable(true),
        );
        ui.add_space(4.0);
        let text = if decks.is_dir() {
            "One text file per deck. Imports are saved here as new files; \
             existing decks are never overwritten."
        } else {
            "Not found yet: it is created by the simulator, or by the first import."
        };
        ui.label(theme::body_small(text).color(s.on_surface_variant));
    }

    fn about_card(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ui.label(theme::title_medium("About").color(s.on_surface));
        ui.label(
            RichText::new(format!(
                "SimuDeckParser {} — settings in {}",
                env!("CARGO_PKG_VERSION"),
                self.settings.path().display()
            ))
            .color(s.on_surface_variant),
        );
    }
}
