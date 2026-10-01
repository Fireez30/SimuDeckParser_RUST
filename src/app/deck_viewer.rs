//! Decks: the decks saved in the simulator, their composition, and Encore Decks imports
//! and uploads.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{CornerRadius, Margin, RichText, ScrollArea, Stroke, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ADD, ICON_CONTENT_COPY, ICON_DOWNLOAD, ICON_EDIT, ICON_FOLDER_OPEN, ICON_LINK,
    ICON_OPEN_IN_NEW, ICON_PRINT, ICON_STACKS, ICON_TRANSLATE, ICON_UPLOAD, ICON_WARNING,
};
use simu_deck_parser::builder::DeckList;
use simu_deck_parser::codes::CardIndex;
use simu_deck_parser::decks::{self, DeckFile};
use simu_deck_parser::encore::{self, Account, SessionExpired, build_deck};
use simu_deck_parser::model::{Deck, Serie};
use simu_deck_parser::pdf::{self, Paper};

use super::card_list::{CardList, Picker};
use super::deck_editor::{DeckEditor, EditorAction};
use super::deck_preview::{DeckPreview, Summary};
use super::theme::{self, scheme};
use super::{Notice, Task};

type ImportResult = Result<(Deck, Vec<String>), String>;

#[derive(Default)]
struct Import {
    link: String,
    task: Option<Task<ImportResult>>,
}

/// The deck link, or the error and whether it came from an expired session.
type UploadResult = Result<String, (String, bool)>;

/// Publishing the open deck on Encore Decks: confirmation, then progress, then the link.
struct Upload {
    name: String,
    codes: Vec<String>,
    /// Account the deck was sent to (set when sending); anonymous when `None`.
    account: Option<Account>,
    task: Option<Task<UploadResult>>,
    /// Link of the published deck.
    done: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Printout {
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
            .join(format!("{} - {suffix}.pdf", decks::file_stem_for(name)))
    }
}

/// Writing the open deck as a PDF: options, then progress, then where it went.
struct Export {
    printout: Printout,
    deck: Deck,
    task: Option<Task<Result<PathBuf, String>>>,
    /// The written file.
    done: Option<PathBuf>,
}

#[derive(Default)]
pub struct DeckViewer {
    /// Deck files of the `Decks` folder; cards are resolved when a deck is picked.
    decks: Vec<DeckFile>,
    /// Picked deck (by file) and its resolved cards.
    picked: Option<(PathBuf, Deck)>,
    query: String,
    /// Codes of the picked deck that matched no card.
    missing: Vec<String>,
    summary: Option<Summary>,
    preview: DeckPreview,
    import: Option<Import>,
    upload: Option<Upload>,
    export: Option<Export>,
    /// Last paper picked for a PDF.
    paper: Paper,
    /// An upload found the linked account's session expired; taken by the app.
    session_expired: bool,
    /// Message for the user (import result, errors), taken by the app.
    notice: Option<Notice>,
    /// Deck being built or edited, with its own card browser.
    editor: Option<DeckEditor>,
    builder: CardList,
    confirm_discard: bool,
    /// Builder requested this frame.
    start: Option<Start>,
}

/// How the deck builder starts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Start {
    New,
    /// Edit the open deck in place.
    Edit,
    /// Start from a copy of the open deck.
    Duplicate,
    /// Not the builder: publish the open deck on Encore Decks.
    Upload,
    /// Not the builder: write the open deck as a PDF.
    Print(Printout),
}

impl DeckViewer {
    /// Re-reads the deck files, keeping the picked deck when it still exists.
    pub fn reload(&mut self, decks_dir: &Path) -> anyhow::Result<()> {
        self.decks = decks::list_decks(decks_dir)?;
        if let Some((path, _)) = &self.picked
            && !self.decks.iter().any(|d| &d.path == path)
        {
            self.clear_selection();
        }
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.picked = None;
        self.missing.clear();
        self.summary = None;
        self.preview.clear();
    }

    pub fn take_notice(&mut self) -> Option<Notice> {
        self.notice.take()
    }

    pub fn open_import(&mut self) {
        if self.import.is_none() {
            self.import = Some(Import::default());
        }
    }

    /// Asks to publish the open deck, with every code of its file (unmatched ones included).
    fn open_upload(&mut self) {
        let Some((path, deck)) = &self.picked else {
            return;
        };
        let codes = self
            .decks
            .iter()
            .find(|d| &d.path == path)
            .map(|d| d.codes.clone())
            .unwrap_or_else(|| deck.cards.iter().map(|c| c.key.clone()).collect());
        self.upload = Some(Upload {
            name: deck.name.clone(),
            codes,
            account: None,
            task: None,
            done: None,
        });
    }

    fn open_export(&mut self, printout: Printout) {
        if let Some((_, deck)) = &self.picked {
            self.export = Some(Export {
                printout,
                deck: deck.clone(),
                task: None,
                done: None,
            });
        }
    }

    /// Finishes a background export: shows where the file went, or closes the dialog on error.
    fn poll_export(&mut self) {
        let Some(export) = &mut self.export else {
            return;
        };
        let Some(result) = export.task.as_ref().and_then(Task::poll) else {
            return;
        };
        export.task = None;
        let error = match result {
            Some(Ok(path)) => {
                export.done = Some(path);
                return;
            }
            Some(Err(e)) => e,
            None => "The export stopped unexpectedly.".into(),
        };
        self.notice = Some(Notice::Error("Could not write the PDF".into(), error));
        self.export = None;
    }

    /// Finishes a background upload: shows the link, or closes the dialog on error.
    fn poll_upload(&mut self) {
        let Some(upload) = &mut self.upload else {
            return;
        };
        let Some(result) = upload.task.as_ref().and_then(Task::poll) else {
            return;
        };
        upload.task = None;
        match result {
            Some(Ok(url)) => upload.done = Some(url),
            Some(Err((e, expired))) => {
                self.upload = None;
                self.session_expired |= expired;
                self.notice = Some(Notice::Error("Upload failed".into(), e));
            }
            None => {
                self.upload = None;
                self.notice = Some(Notice::Error(
                    "Upload failed".into(),
                    "The upload stopped unexpectedly.".into(),
                ));
            }
        }
    }

    /// Whether the linked account must be forgotten (its session expired during an upload).
    pub fn take_session_expired(&mut self) -> bool {
        std::mem::take(&mut self.session_expired)
    }

    /// Finishes a background import and opens the new deck.
    pub fn poll_import(&mut self, decks_dir: &Path) {
        let Some(result) = self
            .import
            .as_ref()
            .and_then(|i| i.task.as_ref())
            .and_then(Task::poll)
        else {
            return;
        };
        self.import = None;
        self.notice = Some(self.import_finished(result, decks_dir));
    }

    /// A background import is done: refresh the deck list, open the new deck, tell the user.
    fn import_finished(&mut self, result: Option<ImportResult>, decks_dir: &Path) -> Notice {
        match result {
            Some(Ok((deck, missing))) => {
                let mut msg = format!(
                    "Imported \"{}\" ({} cards) into the simulator decks.",
                    deck.name,
                    deck.cards.len()
                );
                if !missing.is_empty() {
                    msg += &format!(" {} code(s) matched no card.", missing.len());
                }
                if let Err(e) = self.reload(decks_dir) {
                    msg += &format!(" Could not refresh the deck list: {e:#}");
                }
                let path = self
                    .decks
                    .iter()
                    .find(|d| !d.ai && d.name == deck.name)
                    .map(|d| d.path.clone());
                self.clear_selection();
                if let Some(path) = path {
                    self.summary = Some(Summary::new(&deck.cards));
                    self.picked = Some((path, deck));
                    self.missing = missing;
                }
                Notice::Info(msg)
            }
            Some(Err(e)) => Notice::Error("Import failed".into(), e),
            None => Notice::Error(
                "Import failed".into(),
                "The import stopped unexpectedly.".into(),
            ),
        }
    }

    fn pick(&mut self, ctx: &egui::Context, path: PathBuf, series: &[Serie]) {
        ctx.forget_all_images();
        self.clear_selection();
        let Some(file) = self.decks.iter().find(|d| d.path == path) else {
            return;
        };
        let (deck, missing) =
            build_deck(&CardIndex::new(series), &file.name, &file.date, &file.codes);
        self.summary = Some(Summary::new(&deck.cards));
        self.missing = missing;
        self.picked = Some((path, deck));
    }

    /// Whether a deck is being built, with changes not saved yet.
    pub fn has_unsaved_deck(&self) -> bool {
        self.editor.as_ref().is_some_and(DeckEditor::is_dirty)
    }

    fn open_editor(&mut self, series: &[Serie], start: Start) {
        let picked = self.picked.as_ref();
        let (name, target, list) = match (start, picked) {
            (Start::Edit, Some((path, deck))) => (
                deck.name.clone(),
                Some(path.clone()),
                DeckList::new(&deck.cards, self.missing.clone()),
            ),
            (Start::Duplicate, Some((_, deck))) => (
                format!("{} (copy)", deck.name),
                None,
                DeckList::new(&deck.cards, self.missing.clone()),
            ),
            _ => (String::new(), None, DeckList::default()),
        };
        let taken = self
            .decks
            .iter()
            .filter(|d| Some(&d.path) != target.as_ref())
            .map(|d| d.name.clone())
            .collect();
        if let Some((card, _)) = list.entries().first() {
            self.builder.open_serie_of(series, &card.key);
        }
        self.editor = Some(DeckEditor::new(name, target, list, taken));
    }

    fn save_editor(&mut self, ctx: &egui::Context, series: &[Serie], decks_dir: &Path) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let codes = editor.list.codes();
        let date = decks::now_date();
        let result = match &editor.target {
            Some(path) => decks::update_deck(path, &editor.name, &date, &codes),
            None => decks::save_new_deck(decks_dir, editor.name.trim(), &date, &codes),
        };
        match result {
            Ok(saved) => {
                self.editor = None;
                if let Err(e) = self.reload(decks_dir) {
                    self.notice = Some(Notice::Error(
                        "Could not read the simulator decks".into(),
                        format!("{e:#}"),
                    ));
                    return;
                }
                self.pick(ctx, saved.path, series);
                self.notice = Some(Notice::Info(format!(
                    "Saved \"{}\" ({} cards) to the simulator decks.",
                    saved.name,
                    codes.len()
                )));
            }
            Err(e) => editor.error = Some(format!("Could not save: {e:#}")),
        }
    }

    fn discard_dialog(&mut self, ctx: &egui::Context) {
        if !self.confirm_discard {
            return;
        }
        let name = self
            .editor
            .as_ref()
            .map(|e| e.name.trim().to_string())
            .unwrap_or_default();
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("discard_deck"))
            .frame(theme::dialog_frame(ctx))
            .show(ctx, |ui| {
                let s = scheme(ui);
                ui.set_width(400.0);
                ui.label(theme::headline("Discard changes?").color(s.on_surface));
                ui.add_space(8.0);
                let what = if name.is_empty() {
                    "This deck".to_string()
                } else {
                    format!("\"{name}\"")
                };
                ui.label(
                    RichText::new(format!("{what} has changes that are not saved."))
                        .color(s.on_surface_variant),
                );
                ui.add_space(20.0);
                super::actions_row(ui, |ui| {
                    if theme::filled(None, "Discard").show(ui).clicked() {
                        choice = Some(true);
                    }
                    if theme::text(None, "Keep editing").show(ui).clicked() {
                        choice = Some(false);
                    }
                });
            });
        if modal.should_close() {
            choice = choice.or(Some(false));
        }
        match choice {
            Some(true) => {
                self.editor = None;
                self.confirm_discard = false;
            }
            Some(false) => self.confirm_discard = false,
            None => {}
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        series: &Arc<Vec<Serie>>,
        decks_dir: &Path,
        account: Option<&Account>,
        // The linked account's session is still being read: no upload yet, it would be
        // anonymous.
        account_pending: bool,
    ) {
        self.poll_import(decks_dir);
        self.poll_upload();
        self.poll_export();
        if let Some(editor) = &mut self.editor {
            let title = editor.title();
            self.builder
                .show(ui, series, title, Some(editor as &mut dyn Picker));
            match editor.take_action() {
                Some(EditorAction::Save) => self.save_editor(ui.ctx(), series, decks_dir),
                Some(EditorAction::Cancel) if editor.is_dirty() => self.confirm_discard = true,
                Some(EditorAction::Cancel) => self.editor = None,
                None => {}
            }
            self.discard_dialog(ui.ctx());
            return;
        }
        let s = scheme(ui);
        let mut pick = None;
        egui::Panel::left("deck_list")
            .resizable(true)
            .default_size(300.0)
            .size_range(240.0..=420.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                left: 16,
                right: 8,
                top: 16,
                bottom: 16,
            }))
            .show(ui, |ui| {
                theme::surface_card(s.surface_container_low)
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_min_height(ui.available_height());
                        pick = self.deck_list(ui, decks_dir);
                    });
            });
        if let Some(path) = pick {
            self.pick(ui.ctx(), path, series);
        }

        if self.summary.is_some() {
            egui::Panel::right("deck_card_panel")
                .resizable(true)
                .default_size(360.0)
                .size_range(280.0..=560.0)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                    left: 8,
                    right: 16,
                    top: 16,
                    bottom: 16,
                }))
                .show(ui, |ui| {
                    theme::surface_card(s.surface_container_low).show(ui, |ui| {
                        ui.set_min_height(ui.available_height());
                        self.preview.pane().show(ui);
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                left: 12,
                right: 8,
                top: 16,
                bottom: 0,
            }))
            .show(ui, |ui| self.deck_content(ui));
        if let Some(start) = self.start.take() {
            match start {
                Start::Upload => self.open_upload(),
                Start::Print(printout) => self.open_export(printout),
                _ => self.open_editor(series, start),
            }
        }

        self.import_dialog(ui.ctx(), series, decks_dir);
        self.upload_dialog(ui.ctx(), account, account_pending);
        self.export_dialog(ui.ctx());
    }

    /// Returns the deck file the user clicked.
    fn deck_list(&mut self, ui: &mut Ui, decks_dir: &Path) -> Option<PathBuf> {
        let s = scheme(ui);
        let mut pick = None;
        let own = self.decks.iter().filter(|d| !d.ai).count();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(theme::headline("Decks").color(s.on_surface));
            ui.label(theme::label(own.to_string()).color(s.on_surface_variant));
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let half = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            if theme::filled(Some(ICON_ADD), "New deck")
                .min_width(half)
                .show(ui)
                .clicked()
            {
                self.start = Some(Start::New);
            }
            let import = theme::tonal(Some(ICON_DOWNLOAD), "Import")
                .min_width(half)
                .show(ui)
                .on_hover_text("Import a deck from Encore Decks");
            if import.clicked() {
                self.open_import();
            }
        });
        ui.add_space(4.0);
        if self.decks.is_empty() {
            ui.add_space(24.0);
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "No decks yet.\nDecks saved in the simulator appear here.\n\n{}",
                        decks_dir.display()
                    ))
                    .color(s.on_surface_variant),
                )
                .wrap(),
            );
            return None;
        }
        if self.decks.len() > 8 {
            theme::search_bar(
                ui,
                &mut self.query,
                "Find a deck",
                ui.available_width() - 16.0,
            );
            ui.add_space(4.0);
        }
        let query = self.query.trim().to_lowercase();
        let picked = self.picked.as_ref().map(|(p, _)| p);
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for ai in [false, true] {
                let decks: Vec<&DeckFile> = self
                    .decks
                    .iter()
                    .filter(|d| d.ai == ai)
                    .filter(|d| query.is_empty() || d.name.to_lowercase().contains(&query))
                    .collect();
                if decks.is_empty() {
                    continue;
                }
                if ai {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(theme::label("AI opponents").color(s.on_surface_variant));
                    });
                    ui.add_space(2.0);
                }
                for deck in decks {
                    let selected = picked == Some(&deck.path);
                    let name = if ai {
                        deck.name.strip_prefix("AI_").unwrap_or(&deck.name)
                    } else {
                        &deck.name
                    };
                    let supporting = if ai || deck.date.trim().is_empty() {
                        format!("{} cards", deck.codes.len())
                    } else {
                        format!("{} cards · {}", deck.codes.len(), deck.date.trim())
                    };
                    if theme::list_item(ui, name, &supporting, selected).clicked() && !selected {
                        pick = Some(deck.path.clone());
                    }
                }
            }
        });
        pick
    }

    fn deck_content(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        let (Some((_, deck)), Some(summary)) = (&self.picked, &self.summary) else {
            theme::empty_state(
                ui,
                ICON_STACKS,
                "Pick a deck",
                "Choose a deck on the left, build a new one, or import one from Encore Decks.",
            );
            return;
        };

        let ai = self
            .decks
            .iter()
            .any(|d| d.ai && Some(&d.path) == self.picked.as_ref().map(|(p, _)| p));
        let mut start = None;
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add(egui::Label::new(theme::headline(&deck.name).color(s.on_surface)).wrap());
            if !deck.date.trim().is_empty() {
                ui.label(
                    RichText::new(format!("Saved {}", deck.date.trim()))
                        .color(s.on_surface_variant),
                );
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                if !ai && theme::tonal(Some(ICON_EDIT), "Edit").show(ui).clicked() {
                    start = Some(Start::Edit);
                }
                let duplicate = theme::outlined(Some(ICON_CONTENT_COPY), "Duplicate")
                    .show(ui)
                    .on_hover_text("Build a new deck starting from this one");
                if duplicate.clicked() {
                    start = Some(Start::Duplicate);
                }
                let upload = theme::outlined(Some(ICON_UPLOAD), "Upload")
                    .show(ui)
                    .on_hover_text("Publish this deck on Encore Decks");
                if upload.clicked() {
                    start = Some(Start::Upload);
                }
                ui.add_space(8.0);
                let proxies = theme::outlined(Some(ICON_PRINT), "Proxies")
                    .show(ui)
                    .on_hover_text("PDF of the deck's cards at real size, to print and cut out");
                if proxies.clicked() {
                    start = Some(Start::Print(Printout::Proxies));
                }
                let translation = theme::outlined(Some(ICON_TRANSLATE), "Translation")
                    .show(ui)
                    .on_hover_text("PDF of the cards with their English text");
                if translation.clicked() {
                    start = Some(Start::Print(Printout::Translation));
                }
            });
            ui.add_space(8.0);
            self.preview.show(ui, summary, &self.missing);
        });
        if start.is_some() {
            self.start = start;
        }
    }

    fn import_dialog(&mut self, ctx: &egui::Context, series: &Arc<Vec<Serie>>, decks_dir: &Path) {
        let Some(import) = &mut self.import else {
            return;
        };
        let running = import.task.is_some();
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
                let valid = encore::deck_id_from_link(&import.link).is_some();
                let edit = egui::Frame::new()
                    .stroke(Stroke::new(
                        1.0,
                        if !valid && !import.link.trim().is_empty() {
                            s.error
                        } else {
                            s.outline
                        },
                    ))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(12, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(ICON_LINK.rich_text().size(20.0).color(s.on_surface_variant));
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut import.link)
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
                if !running && import.link.is_empty() && !edit.has_focus() {
                    edit.request_focus();
                }
                let hint = if import.link.trim().is_empty() {
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
                let submit = valid
                    && !running
                    && edit.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                super::actions_row(ui, |ui| {
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
                        let link = import.link.trim().to_string();
                        let series = Arc::clone(series);
                        let decks_dir: PathBuf = decks_dir.to_path_buf();
                        import.task = Some(Task::spawn(ui.ctx(), move || {
                            encore::import(&link, &series, &decks_dir).map_err(|e| format!("{e:#}"))
                        }));
                    }
                });
            });
        if !running && (close || modal.should_close()) {
            self.import = None;
        }
    }

    /// `account` is the one linked right now: the deck goes to it when Upload is clicked.
    fn upload_dialog(
        &mut self,
        ctx: &egui::Context,
        account: Option<&Account>,
        account_pending: bool,
    ) {
        let Some(upload) = &mut self.upload else {
            return;
        };
        let running = upload.task.is_some();
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("upload"))
            .frame(theme::dialog_frame(ctx))
            .show(ctx, |ui| {
                let s = scheme(ui);
                ui.set_width(520.0);
                ui.vertical_centered(|ui| {
                    ui.label(ICON_UPLOAD.rich_text().size(28.0).color(s.primary));
                    let title = if upload.done.is_some() {
                        "Uploaded to Encore Decks"
                    } else {
                        "Upload to Encore Decks"
                    };
                    ui.label(theme::headline(title).color(s.on_surface));
                });
                ui.add_space(8.0);

                if let Some(done) = &upload.done {
                    let where_to = match &upload.account {
                        Some(account) => format!(" in the account {}", account.name),
                        None => String::new(),
                    };
                    ui.label(
                        RichText::new(format!("\"{}\" is online{where_to}:", upload.name))
                            .color(s.on_surface_variant),
                    );
                    ui.hyperlink(done);
                    ui.add_space(16.0);
                    super::actions_row(ui, |ui| {
                        if theme::filled(Some(ICON_OPEN_IN_NEW), "Open")
                            .show(ui)
                            .clicked()
                        {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(done));
                        }
                        if theme::tonal(Some(ICON_CONTENT_COPY), "Copy link")
                            .show(ui)
                            .clicked()
                        {
                            ui.ctx().copy_text(done.clone());
                        }
                        close = theme::text(None, "Close").show(ui).clicked();
                    });
                    return;
                }

                let destination = match account {
                    Some(account) => format!(
                        "as a new public deck of your account {}, where you can edit or \
                         delete it.",
                        account.name
                    ),
                    None => "as a new public, anonymous deck: it cannot be edited or deleted \
                             there afterwards. Link your Encore Decks account in Settings to \
                             upload to it instead."
                        .to_string(),
                };
                ui.label(
                    RichText::new(format!(
                        "\"{}\" ({} cards) is published on encoredecks.com {destination}",
                        upload.name,
                        upload.codes.len()
                    ))
                    .color(s.on_surface_variant),
                );
                ui.add_space(4.0);
                ui.label(
                    theme::body_small(
                        "Codes are matched to Encore Decks cards (JP/EN, foil or not). If any \
                         card is not found there, nothing is uploaded.",
                    )
                    .color(s.on_surface_variant),
                );
                let size_error = encore::check_deck_size(&upload.codes).err();
                if let Some(e) = &size_error {
                    ui.add_space(8.0);
                    theme::banner(ui, ICON_WARNING, &format!("Cannot upload: {e}."));
                }
                if account_pending {
                    ui.add_space(8.0);
                    theme::banner(
                        ui,
                        ICON_WARNING,
                        "Reading your Encore Decks session from the system keyring…",
                    );
                }
                ui.add_space(16.0);
                super::actions_row(ui, |ui| {
                    if running {
                        ui.label(RichText::new("Uploading…").color(s.on_surface_variant));
                        ui.spinner();
                        return;
                    }
                    if theme::filled(None, "Upload")
                        .enabled(size_error.is_none() && !account_pending)
                        .show(ui)
                        .clicked()
                    {
                        let name = upload.name.clone();
                        let codes = upload.codes.clone();
                        upload.account = account.cloned();
                        let session = account.map(|a| a.session.clone());
                        upload.task = Some(Task::spawn(ui.ctx(), move || {
                            let session = session.as_ref().map(|s| s.as_str());
                            encore::upload(&name, &codes, session)
                                .map_err(|e| (format!("{e:#}"), e.is::<SessionExpired>()))
                        }));
                    }
                    close = theme::text(None, "Cancel").show(ui).clicked();
                });
            });
        if !running && (close || modal.should_close()) {
            self.upload = None;
        }
    }
}

impl DeckViewer {
    fn export_dialog(&mut self, ctx: &egui::Context) {
        let Some(export) = &mut self.export else {
            return;
        };
        let running = export.task.is_some();
        let mut close = false;
        let mut failed = None;
        let modal = egui::Modal::new(egui::Id::new("export"))
            .frame(theme::dialog_frame(ctx))
            .show(ctx, |ui| {
                let s = scheme(ui);
                ui.set_width(520.0);
                ui.vertical_centered(|ui| {
                    let icon = match export.printout {
                        Printout::Proxies => ICON_PRINT,
                        Printout::Translation => ICON_TRANSLATE,
                    };
                    ui.label(icon.rich_text().size(28.0).color(s.primary));
                    ui.label(theme::headline(export.printout.title()).color(s.on_surface));
                });
                ui.add_space(8.0);

                if let Some(path) = &export.done {
                    ui.label(RichText::new("The PDF is saved in:").color(s.on_surface_variant));
                    ui.label(RichText::new(path.display().to_string()).color(s.on_surface));
                    ui.add_space(16.0);
                    super::actions_row(ui, |ui| {
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

                let count = export.deck.cards.len();
                let about = match export.printout {
                    Printout::Proxies => {
                        let (cols, rows) = pdf::proxy_grid(self.paper);
                        format!(
                            "Every card of \"{}\" ({count} cards) at Weiss Schwarz size \
                             (59 × 86 mm), {} per page, with crop marks. Print at actual size \
                             (100 %), not \"fit to page\".",
                            export.deck.name,
                            cols * rows
                        )
                    }
                    Printout::Translation => format!(
                        "Each different card of \"{}\" with its picture, stats and English \
                         effect text, to keep next to the deck while playing.",
                        export.deck.name
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
                super::actions_row(ui, |ui| {
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
                        let (printout, paper) = (export.printout, self.paper);
                        let deck = export.deck.clone();
                        export.task = Some(Task::spawn(ui.ctx(), move || {
                            let bytes = match printout {
                                Printout::Proxies => pdf::proxies(&deck, paper),
                                Printout::Translation => pdf::translation(&deck, paper),
                            }
                            .map_err(|e| format!("{e:#}"))?;
                            let path = printout.path(&deck.name);
                            std::fs::write(&path, bytes)
                                .map_err(|e| format!("Writing {}: {e}", path.display()))?;
                            Ok(path)
                        }));
                    }
                    close = theme::text(None, "Cancel").show(ui).clicked();
                });
            });
        if let Some(e) = failed {
            self.notice = Some(Notice::Error(
                "Could not open the PDF".into(),
                e.to_string(),
            ));
        }
        if !running && (close || modal.should_close()) {
            self.export = None;
        }
    }
}

