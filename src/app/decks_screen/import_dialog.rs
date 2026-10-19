//! The Import dialog: paste an Encore Decks link, and the deck is saved in the simulator.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{CornerRadius, Margin, RichText, Stroke};
use egui_material_icons::icons::{ICON_DOWNLOAD, ICON_LINK};
use simu_deck_parser::encoredecks::decks::{deck_id_from_link, import};
use simu_deck_parser::model::{Deck, Serie};

use super::{DecksScreen, PickedDeck};
use crate::app::Notice;
use crate::app::background::{Task, finished};
use crate::app::deck_preview::Summary;
use crate::app::theme::{self, scheme};

#[derive(Default)]
pub(super) struct ImportDialog {
    /// The link, as typed.
    link: String,
    /// The import, once started.
    task: Option<Task<Deck>>,
}

/// What to tell the user once `deck` is imported.
pub(in crate::app) fn import_message(deck: &Deck) -> String {
    let mut message = format!(
        "Imported \"{}\" ({} cards) into the simulator decks.",
        deck.name,
        deck.cards.len()
    );
    if !deck.unmatched.is_empty() {
        message += &format!(" {} code(s) matched no card.", deck.unmatched.len());
    }
    message
}

impl DecksScreen {
    pub(super) fn open_import(&mut self) {
        if self.import.is_none() {
            self.import = Some(ImportDialog::default());
        }
    }

    /// Finishes a background import: refreshes the deck list, opens the new deck, and tells
    /// the user.
    pub(super) fn poll_import(&mut self, decks_dir: &Path) {
        let Some(dialog) = &mut self.import else {
            return;
        };
        let Some(result) = finished(&mut dialog.task) else {
            return;
        };
        self.import = None;
        let deck = match result {
            Ok(deck) => deck,
            Err(e) => {
                self.notice = Some(Notice::from_error("Import failed", &e));
                return;
            }
        };
        let mut message = import_message(&deck);
        if let Err(e) = self.reload(decks_dir) {
            message += &format!(" Could not refresh the deck list: {e:#}");
        }
        // The new file: the import gave the deck a free name.
        let file = self
            .decks
            .iter()
            .find(|d| !d.ai && d.name == deck.name)
            .cloned();
        self.clear_selection();
        if let Some(file) = file {
            self.picked = Some(PickedDeck {
                file,
                summary: Summary::new(&deck.cards),
                deck,
            });
        }
        self.notice = Some(Notice::Info(message));
    }

    pub(super) fn import_dialog(
        &mut self,
        ctx: &egui::Context,
        series: &Arc<Vec<Serie>>,
        decks_dir: &Path,
    ) {
        let Some(dialog) = &mut self.import else {
            return;
        };
        let running = dialog.task.is_some();
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("import"))
            .frame(theme::dialog_frame(ctx))
            .show(ctx, |ui| {
                let s = scheme(ui);
                ui.set_width(520.0);
                ui.vertical_centered(|ui| {
                    ui.label(ICON_DOWNLOAD.rich_text().size(28.0).color(s.primary));
                    ui.label(theme::headline("Import from Encore Decks").color(s.on_surface));
                });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Paste the link of an Encore Decks deck. It is added to the simulator's \
                         deck list; codes are matched to the loaded cards (JP/EN, foil or not).",
                    )
                    .color(s.on_surface_variant),
                );
                ui.add_space(12.0);
                let valid = deck_id_from_link(&dialog.link).is_some();
                let typed_something = !dialog.link.trim().is_empty();
                let border = if !valid && typed_something {
                    s.error
                } else {
                    s.outline
                };
                let edit = egui::Frame::new()
                    .stroke(Stroke::new(1.0, border))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(12, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(ICON_LINK.rich_text().size(20.0).color(s.on_surface_variant));
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut dialog.link)
                                    .hint_text("https://www.encoredecks.com/deck/…")
                                    .frame(egui::Frame::NONE)
                                    .background_color(egui::Color32::TRANSPARENT)
                                    .margin(Margin::symmetric(4, 8))
                                    .desired_width(f32::INFINITY),
                            )
                        })
                        .inner
                    })
                    .inner;
                if !running && dialog.link.is_empty() && !edit.has_focus() {
                    edit.request_focus();
                }
                let hint = if !typed_something {
                    RichText::new("e.g. https://www.encoredecks.com/deck/PPyvcLuvt")
                        .color(s.on_surface_variant)
                } else if valid {
                    RichText::new("Link looks good").color(s.success)
                } else {
                    RichText::new("This is not an Encore Decks deck link").color(s.error)
                };
                ui.label(hint.small());
                ui.add_space(4.0);
                ui.label(
                    theme::body_small(
                        "The deck is saved as a new file in the simulator's Decks folder; \
                         existing decks are never overwritten.",
                    )
                    .color(s.on_surface_variant),
                );
                ui.add_space(16.0);
                let pressed_enter =
                    edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let submit = valid && !running && pressed_enter;
                theme::actions_row(ui, |ui| {
                    if running {
                        ui.label(RichText::new("Importing…").color(s.on_surface_variant));
                        ui.spinner();
                        return;
                    }
                    let import_clicked = theme::filled(None, "Import")
                        .enabled(valid)
                        .show(ui)
                        .clicked();
                    close = theme::text(None, "Cancel").show(ui).clicked();
                    if import_clicked || submit {
                        let link = dialog.link.trim().to_string();
                        // The thread gets its own handles on the cards and the folder.
                        let series = Arc::clone(series);
                        let decks_dir: PathBuf = decks_dir.to_path_buf();
                        dialog.task = Some(Task::spawn(ui.ctx(), move || {
                            import(&link, &series, &decks_dir)
                        }));
                    }
                });
            });
        if !running && (close || modal.should_close()) {
            self.import = None;
        }
    }
}
