//! The first screen: asks where the simulator is installed, then loads its cards.
//!
//! The methods here belong to [`App`]: Rust lets an `impl App { … }` block live in any file of
//! the module, so each screen's code stays in its own file.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Margin, RichText, ScrollArea, Ui};
use egui_material_icons::icons::{
    ICON_CHECK_CIRCLE, ICON_ERROR, ICON_FOLDER, ICON_FOLDER_OPEN, ICON_STYLE,
};
use simu_deck_parser::card_index::NameIndex;
use simu_deck_parser::settings;
use simu_deck_parser::simulator::folder::{self, SimulatorFolder};
use simu_deck_parser::simulator::load_series;

use super::background::{Task, finished};
use super::card_browser::CardBrowser;
use super::decks_screen::DecksScreen;
use super::encore_screen::EncoreScreen;
use super::theme::{self, scheme};
use super::{App, Notice, Screen, card_view};

/// The simulator folder form.
#[derive(Default)]
pub(super) struct Setup {
    /// The path as typed.
    path_input: String,
    /// The simulator install `path_input` points to, if any.
    pub(super) resolved: Option<SimulatorFolder>,
    error: Option<String>,
    /// Simulator installs found on this computer, offered under the field.
    detected: Vec<PathBuf>,
    pub(super) detecting: Option<Task<Vec<PathBuf>>>,
}

impl Setup {
    pub(super) fn set_path(&mut self, path: String) {
        self.path_input = path;
        self.revalidate();
    }

    fn revalidate(&mut self) {
        self.error = None;
        let trimmed = self.path_input.trim();
        self.resolved = if trimmed.is_empty() {
            None
        } else {
            SimulatorFolder::find(Path::new(trimmed))
        };
    }

    /// Looks for simulator installs in the background, unless it was already done.
    pub(super) fn start_detecting(&mut self, ctx: &egui::Context) {
        if self.detected.is_empty() && self.detecting.is_none() {
            self.detecting = Some(Task::spawn(ctx, || Ok(folder::detect_installs())));
        }
    }
}

impl App {
    /// Starts reading the cards of the folder picked in the form; `save` remembers the folder
    /// for the next start.
    pub(super) fn load_simulator(&mut self, ctx: &egui::Context, save: bool) {
        let Some(folder) = self.setup.resolved.clone() else {
            self.setup.error = Some(
                "This is not the simulator folder. Pick the folder that contains \
                 CardContent and Decks."
                    .into(),
            );
            return;
        };
        if save {
            self.save_setting(
                ctx,
                settings::SIMULATOR_PATH,
                &folder.root.to_string_lossy(),
            );
        }
        let job_folder = folder.clone();
        let task = Task::spawn(ctx, move || Ok(load_series(&job_folder)?));
        self.loading = Some((folder, task));
    }

    /// Forgets the simulator ("Change folder…" in Settings) and goes back to the form.
    pub(super) fn unload_simulator(&mut self, ctx: &egui::Context) {
        self.delete_setting(ctx, settings::SIMULATOR_PATH);
        self.series = Arc::default();
        card_view::set_names(ctx, None);
        self.folder = None;
        self.card_browser = CardBrowser::default();
        self.decks_screen = DecksScreen::default();
        self.encore_screen = EncoreScreen::default();
        self.screen = Screen::Setup;
        self.setup.revalidate();
        self.setup.start_detecting(ctx);
        ctx.forget_all_images();
    }

    /// Finishes the background work of this screen: detecting installs, loading the cards.
    pub(super) fn poll_loading(&mut self, ctx: &egui::Context) {
        if let Some(result) = finished(&mut self.setup.detecting) {
            self.setup.detected = result.unwrap_or_default();
        }

        let Some((folder, task)) = &self.loading else {
            return;
        };
        let Some(result) = task.poll() else {
            return;
        };
        let folder = folder.clone();
        self.loading = None;
        match result {
            Ok(series) => {
                let reloaded = self.folder.as_ref() == Some(&folder);
                self.series = Arc::new(series);
                card_view::set_names(ctx, Some(Arc::new(NameIndex::new(&self.series))));
                self.folder = Some(folder);
                let serie = self.settings.get(settings::SERIE);
                self.saved_serie = serie.clone();
                self.card_browser = CardBrowser::default();
                self.card_browser.open_serie(&self.series, serie.as_deref());
                if reloaded {
                    let sets: usize = self.series.iter().map(|s| s.sets.len()).sum();
                    let message = format!("Reloaded {} series ({sets} sets).", self.series.len());
                    self.notify(ctx, Notice::Info(message));
                } else {
                    let start = self.start_screen;
                    self.screen = Screen::Setup;
                    self.switch_to(ctx, start);
                }
            }
            Err(e) => {
                self.setup.error = Some(format!("Could not load {}: {e:#}", folder.root.display()));
                self.screen = Screen::Setup;
            }
        }
    }

    pub(super) fn setup_screen(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(((ui.available_height() - 520.0) / 2.0).max(32.0));
                let width = ui.available_width().min(640.0);
                theme::surface_card(s.surface_container_low)
                    .inner_margin(Margin::same(32))
                    .show(ui, |ui| {
                        ui.set_width(width - 64.0);
                        ui.vertical_centered(|ui| {
                            ui.label(ICON_STYLE.rich_text().size(44.0).color(s.primary));
                            ui.add_space(4.0);
                            ui.label(theme::headline("SimuDeckParser").color(s.on_surface));
                            ui.label(
                                RichText::new(
                                    "Browse the Weiss Schwarz Simulator cards, review your decks \
                                     and import decks from Encore Decks.",
                                )
                                .color(s.on_surface_variant),
                            );
                        });
                        ui.add_space(24.0);
                        if let Some((folder, _)) = &self.loading {
                            ui.vertical_centered(|ui| {
                                ui.add(egui::Spinner::new().size(36.0).color(s.primary));
                                ui.add_space(8.0);
                                ui.label(theme::title_medium("Loading cards…").color(s.on_surface));
                                ui.label(
                                    RichText::new(folder.root.display().to_string())
                                        .color(s.on_surface_variant)
                                        .small(),
                                );
                            });
                            return;
                        }
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            self.setup_form(ui);
                        });
                    });
            });
        });
    }

    /// The folder field, its status line, the detected installs and the Load button.
    fn setup_form(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ui.label(theme::title_medium("Simulator folder").color(s.on_surface));
        ui.label(
            RichText::new(
                "The folder where the simulator is installed, the one that contains \
                 CardContent and Decks. You only need to set it once.",
            )
            .color(s.on_surface_variant),
        );
        ui.add_space(8.0);

        let typed_nothing = self.setup.path_input.trim().is_empty();
        let status_color = match (&self.setup.resolved, &self.setup.error) {
            (_, Some(_)) => s.error,
            (Some(_), None) => s.success,
            (None, None) if typed_nothing => s.outline,
            (None, None) => s.error,
        };
        let edit = egui::Frame::new()
            .stroke(egui::Stroke::new(1.5, status_color))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        ICON_FOLDER
                            .rich_text()
                            .size(20.0)
                            .color(s.on_surface_variant),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.setup.path_input)
                            .hint_text(if cfg!(windows) {
                                r"C:\Games\SimuWeiss"
                            } else {
                                "/home/me/Games/SimuWeiss"
                            })
                            .frame(egui::Frame::NONE)
                            .background_color(egui::Color32::TRANSPARENT)
                            .margin(Margin::symmetric(4, 8))
                            .desired_width(f32::INFINITY),
                    )
                })
                .inner
            })
            .inner;
        if edit.changed() {
            self.setup.revalidate();
        }
        let (icon, message) = match (&self.setup.resolved, &self.setup.error) {
            (_, Some(e)) => (Some(ICON_ERROR), e.clone()),
            (Some(folder), None) => (
                Some(ICON_CHECK_CIRCLE),
                if Path::new(self.setup.path_input.trim()) == folder.root {
                    "Simulator found.".to_string()
                } else {
                    format!("Simulator found in {}", folder.root.display())
                },
            ),
            (None, None) if typed_nothing => (
                None,
                "Paste the folder path, or pick a detected one below.".into(),
            ),
            (None, None) => (Some(ICON_ERROR), "No CardContent/Cards folder here.".into()),
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let color = if icon.is_some() {
                status_color
            } else {
                s.on_surface_variant
            };
            if let Some(icon) = icon {
                ui.label(icon.rich_text().size(16.0).color(color));
            }
            ui.add(egui::Label::new(theme::body_small(message).color(color)).wrap());
        });

        // The detected installs, except the one already in the field.
        let detected: Vec<PathBuf> = self
            .setup
            .detected
            .iter()
            .filter(|path| {
                self.setup
                    .resolved
                    .as_ref()
                    .is_none_or(|f| &f.root != *path)
            })
            .cloned()
            .collect();
        if self.setup.detecting.is_some() || !detected.is_empty() {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(theme::label("Found on this computer").color(s.on_surface_variant));
                if self.setup.detecting.is_some() {
                    ui.add(egui::Spinner::new().size(14.0));
                }
            });
            for path in detected {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let parent = path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                if theme::list_item(ui, &name, &parent, false).clicked() {
                    self.setup.set_path(path.display().to_string());
                }
            }
        }

        ui.add_space(20.0);
        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        theme::actions_row(ui, |ui| {
            let load = theme::filled(Some(ICON_FOLDER_OPEN), "Load simulator")
                .enabled(self.setup.resolved.is_some())
                .show(ui);
            if load.clicked() || enter {
                self.load_simulator(ui.ctx(), true);
            }
        });
    }
}
