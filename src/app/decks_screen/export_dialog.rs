//! The PDF dialogs (Proxies, Translation): paper choice, progress, then where the file went.

use std::path::PathBuf;

use anyhow::Context as _;
use egui::RichText;
use egui_material_icons::icons::{
    ICON_DOWNLOAD, ICON_FOLDER_OPEN, ICON_OPEN_IN_NEW, ICON_PRINT, ICON_TRANSLATE, ICON_WARNING,
};
use simu_deck_parser::model::Deck;
use simu_deck_parser::pdf::{self, Paper};
use simu_deck_parser::simulator::deck_files;

use super::DecksScreen;
use crate::app::Notice;
use crate::app::background::{Task, finished};
use crate::app::theme::{self, scheme};

/// The two PDFs a deck can be printed as.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Printout {
    /// Every card at real size, to cut out and put in sleeves.
    Proxies,
    /// Each different card with its English text.
    Translation,
}

impl Printout {
    fn title(self) -> &'static str {
        match self {
            Printout::Proxies => "Print proxies",
            Printout::Translation => "Translation sheet",
        }
    }

    /// Where the PDF of the deck `name` is written: the Downloads folder.
    fn path(self, name: &str) -> PathBuf {
        let suffix = match self {
            Printout::Proxies => "proxies",
            Printout::Translation => "translation",
        };
        dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_default()
            .join(format!(
                "{} - {suffix}.pdf",
                deck_files::file_stem_for(name)
            ))
    }

    /// Writes the PDF of `deck` and returns where.
    fn write(self, deck: &Deck, paper: Paper) -> anyhow::Result<PathBuf> {
        let bytes = match self {
            Printout::Proxies => pdf::proxies(deck, paper)?,
            Printout::Translation => pdf::translation(deck, paper)?,
        };
        let path = self.path(&deck.name);
        std::fs::write(&path, bytes).with_context(|| format!("Writing {}", path.display()))?;
        Ok(path)
    }
}

pub(super) struct ExportDialog {
    printout: Printout,
    deck: Deck,
    /// The PDF being written; it ends with the file's path.
    task: Option<Task<PathBuf>>,
    /// The written file.
    done: Option<PathBuf>,
}

impl DecksScreen {
    pub(super) fn open_export(&mut self, printout: Printout) {
        if let Some(picked) = &self.picked {
            self.export = Some(ExportDialog {
                printout,
                deck: picked.deck.clone(),
                task: None,
                done: None,
            });
        }
    }

    /// Finishes a background export: shows where the file went, or closes the dialog on error.
    pub(super) fn poll_export(&mut self) {
        let Some(dialog) = &mut self.export else {
            return;
        };
        match finished(&mut dialog.task) {
            Some(Ok(path)) => dialog.done = Some(path),
            Some(Err(e)) => {
                self.notice = Some(Notice::from_error("Could not write the PDF", &e));
                self.export = None;
            }
            None => {}
        }
    }

    pub(super) fn export_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.export else {
            return;
        };
        let running = dialog.task.is_some();
        let mut close = false;
        let mut failed = None;
        let modal = egui::Modal::new(egui::Id::new("export"))
            .frame(theme::dialog_frame(ctx))
            .show(ctx, |ui| {
                let s = scheme(ui);
                ui.set_width(520.0);
                ui.vertical_centered(|ui| {
                    let icon = match dialog.printout {
                        Printout::Proxies => ICON_PRINT,
                        Printout::Translation => ICON_TRANSLATE,
                    };
                    ui.label(icon.rich_text().size(28.0).color(s.primary));
                    ui.label(theme::headline(dialog.printout.title()).color(s.on_surface));
                });
                ui.add_space(8.0);

                // Done: where the file is.
                if let Some(path) = &dialog.done {
                    ui.label(RichText::new("The PDF is saved in:").color(s.on_surface_variant));
                    ui.label(RichText::new(path.display().to_string()).color(s.on_surface));
                    ui.add_space(16.0);
                    theme::actions_row(ui, |ui| {
                        if theme::filled(Some(ICON_OPEN_IN_NEW), "Open")
                            .show(ui)
                            .clicked()
                        {
                            failed = open::that_detached(path).err();
                        }
                        let folder = theme::tonal(Some(ICON_FOLDER_OPEN), "Show folder").show(ui);
                        if folder.clicked()
                            && let Some(dir) = path.parent()
                        {
                            failed = open::that_detached(dir).err();
                        }
                        close = theme::text(None, "Close").show(ui).clicked();
                    });
                    return;
                }

                // Not written yet: what it will be, the paper, and the Save button.
                let count = dialog.deck.cards.len();
                let about = match dialog.printout {
                    Printout::Proxies => {
                        let (cols, rows) = pdf::proxy_grid(self.paper);
                        format!(
                            "Every card of \"{}\" ({count} cards) at Weiss Schwarz size \
                             (59 × 86 mm), {} per page, with crop marks. Print at actual size \
                             (100 %), not \"fit to page\".",
                            dialog.deck.name,
                            cols * rows
                        )
                    }
                    Printout::Translation => format!(
                        "Each different card of \"{}\" with its picture, stats and English \
                         effect text, to keep next to the deck while playing.",
                        dialog.deck.name
                    ),
                };
                ui.label(RichText::new(about).color(s.on_surface_variant));
                if count == 0 {
                    ui.add_space(8.0);
                    theme::banner(ui, ICON_WARNING, "This deck has no card to print.");
                }
                ui.add_space(12.0);
                ui.label(theme::label("Paper").color(s.on_surface_variant));
                ui.horizontal(|ui| {
                    for paper in Paper::ALL {
                        if theme::chip(paper.label(), self.paper == paper)
                            .show(ui)
                            .clicked()
                        {
                            self.paper = paper;
                        }
                    }
                });
                ui.add_space(16.0);
                theme::actions_row(ui, |ui| {
                    if running {
                        ui.label(RichText::new("Writing the PDF…").color(s.on_surface_variant));
                        ui.spinner();
                        return;
                    }
                    if theme::filled(Some(ICON_DOWNLOAD), "Save PDF")
                        .enabled(count > 0)
                        .show(ui)
                        .clicked()
                    {
                        let (printout, paper) = (dialog.printout, self.paper);
                        let deck = dialog.deck.clone();
                        dialog.task =
                            Some(Task::spawn(ui.ctx(), move || printout.write(&deck, paper)));
                    }
                    close = theme::text(None, "Cancel").show(ui).clicked();
                });
            });
        if let Some(e) = failed {
            self.notice = Some(Notice::error("Could not open the PDF", e.to_string()));
        }
        if !running && (close || modal.should_close()) {
            self.export = None;
        }
    }
}
