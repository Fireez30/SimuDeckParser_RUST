//! The Decks screen: the decks saved in the simulator on the left, the open deck in the middle
//! (its stats and cards), the hovered card on the right.
//!
//! From here the user can also build or edit a deck (the screen then shows the deck editor
//! instead), and open the dialogs of the other files of this folder: importing from Encore
//! Decks, uploading to it, and writing PDFs.

mod export_dialog;
mod import_dialog;
mod upload_dialog;

use std::cmp::Reverse;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{CornerRadius, Margin, RichText, ScrollArea, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ADD, ICON_CONTENT_COPY, ICON_DOWNLOAD, ICON_EDIT, ICON_PRINT, ICON_SORT, ICON_STACKS,
    ICON_TRANSLATE, ICON_UPLOAD,
};
use simu_deck_parser::card_index::CardIndex;
use simu_deck_parser::deck_builder::DeckList;
use simu_deck_parser::encoredecks::account::Account;
use simu_deck_parser::model::{CARD_IMAGE_RATIO, Deck, Serie};
use simu_deck_parser::pdf::Paper;
use simu_deck_parser::simulator::deck_files::{self, DeckFile};
use simu_deck_parser::simulator::sleeves::{Sleeve, list_sleeves};

use super::Notice;
use super::assets;
use super::card_browser::{CardBrowser, Picker};
use super::deck_editor::{DeckEditor, EditorAction};
use super::deck_preview::{DeckPreview, Summary};
use super::sleeve_picker::{SleeveAdder, sleeve_combo};
use super::theme::{self, scheme};
use export_dialog::{ExportDialog, Printout};
use import_dialog::ImportDialog;
pub(super) use import_dialog::import_message;
use upload_dialog::UploadDialog;

/// The deck open in the middle of the screen.
struct PickedDeck {
    /// The deck file, as listed on the left.
    file: DeckFile,
    /// Its cards, matched to the loaded simulator.
    deck: Deck,
    summary: Summary,
}

/// The order of the deck list.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DeckSort {
    #[default]
    Name,
    /// Most recently saved first; decks without a date last.
    Date,
}

impl DeckSort {
    /// The value saved in the settings, to keep the order at the next start.
    pub fn key(self) -> &'static str {
        match self {
            DeckSort::Name => "name",
            DeckSort::Date => "date",
        }
    }

    pub fn from_key(key: Option<&str>) -> Self {
        match key {
            Some("date") => DeckSort::Date,
            _ => DeckSort::Name,
        }
    }

    fn other(self) -> Self {
        match self {
            DeckSort::Name => DeckSort::Date,
            DeckSort::Date => DeckSort::Name,
        }
    }

    fn label(self) -> &'static str {
        match self {
            DeckSort::Name => "Name",
            DeckSort::Date => "Date",
        }
    }

    /// Sorts `decks` in this order (the files come sorted by name).
    fn apply(self, decks: &mut [DeckFile]) {
        decks.sort_by_key(|d| (d.ai, d.name.to_lowercase()));
        if self == DeckSort::Date {
            // Stable: decks saved at the same time stay sorted by name.
            decks.sort_by_key(|d| (d.ai, Reverse(d.saved_at())));
        }
    }
}

/// A button of the open deck (or "New deck") was clicked; carried out after drawing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DeckAction {
    NewDeck,
    /// Edit the open deck in place.
    Edit,
    /// Start a new deck from a copy of the open deck.
    Duplicate,
    /// Publish the open deck on Encore Decks.
    Upload,
    /// Write the open deck as a PDF.
    Print(Printout),
}

#[derive(Default)]
pub struct DecksScreen {
    /// Deck files of the `Decks` folder; cards are only matched when a deck is opened.
    decks: Vec<DeckFile>,
    picked: Option<PickedDeck>,
    /// The deck list's search field.
    query: String,
    sort: DeckSort,
    /// The user picked another order this frame: the app saves it in the settings.
    sort_changed: bool,
    /// Sleeves of the simulator's `Sleeves` folder, for the sleeve picker.
    sleeves: Vec<Sleeve>,
    /// Where the sleeves live; the list is read again whenever a deck is opened.
    sleeves_dir: PathBuf,
    /// Adding a sleeve image from the open deck's sleeve picker.
    sleeve_adder: SleeveAdder,
    /// The open deck's card groups, and the pane showing the hovered card.
    preview: DeckPreview,
    import: Option<ImportDialog>,
    upload: Option<UploadDialog>,
    export: Option<ExportDialog>,
    /// Last paper picked for a PDF.
    paper: Paper,
    /// Message for the user (import result, errors), taken by the app.
    notice: Option<Notice>,
    /// The deck being built or edited. While there is one, the screen shows the editor.
    editor: Option<DeckEditor>,
    /// The card browser next to the editor.
    editor_browser: CardBrowser,
    /// Cancel was clicked in the editor with unsaved changes: ask first.
    confirm_discard: bool,
    /// Button clicked this frame.
    action: Option<DeckAction>,
}

impl DecksScreen {
    /// Re-reads the deck files. The open deck stays open if its file still exists.
    pub fn reload(&mut self, decks_dir: &Path) -> anyhow::Result<()> {
        self.decks = deck_files::list_decks(decks_dir)?;
        self.sort.apply(&mut self.decks);
        if let Some(picked) = &mut self.picked {
            match self.decks.iter().find(|d| d.path == picked.file.path) {
                Some(file) => picked.file = file.clone(),
                None => self.clear_selection(),
            }
        }
        Ok(())
    }

    /// Sets the order of the deck list (the one saved in the settings, at startup).
    pub fn set_sort(&mut self, sort: DeckSort) {
        self.sort = sort;
        self.sort.apply(&mut self.decks);
    }

    /// The order the user just picked, if they picked one since the last call.
    pub fn take_sort_change(&mut self) -> Option<DeckSort> {
        std::mem::take(&mut self.sort_changed).then_some(self.sort)
    }

    fn clear_selection(&mut self) {
        self.picked = None;
        self.preview.clear();
    }

    pub fn take_notice(&mut self) -> Option<Notice> {
        self.notice.take()
    }

    /// Whether a deck is being built, with changes not saved yet.
    pub fn has_unsaved_deck(&self) -> bool {
        self.editor.as_ref().is_some_and(DeckEditor::is_dirty)
    }

    /// Re-reads the sleeve images: some may have been added since the last time.
    fn refresh_sleeves(&mut self) {
        self.sleeves = list_sleeves(&self.sleeves_dir);
    }

    /// Opens the deck file at `path`, matching its codes to the loaded cards.
    fn pick(&mut self, ctx: &egui::Context, path: &Path, series: &[Serie]) {
        ctx.forget_all_images();
        self.clear_selection();
        self.refresh_sleeves();
        let Some(file) = self.decks.iter().find(|d| d.path == path) else {
            return;
        };
        let deck = CardIndex::new(series).resolve_deck(&file.name, &file.date, &file.codes);
        self.picked = Some(PickedDeck {
            file: file.clone(),
            summary: Summary::new(&deck.cards),
            deck,
        });
    }

    /// Re-reads the deck files and opens the one at `path`. On failure, tells the user and
    /// returns `false`.
    fn reload_and_pick(&mut self, ctx: &egui::Context, path: &Path, series: &[Serie]) -> bool {
        let decks_dir = path.parent().unwrap_or(Path::new("."));
        match self.reload(decks_dir) {
            Ok(()) => {
                self.pick(ctx, path, series);
                true
            }
            Err(e) => {
                self.notice = Some(Notice::from_error("Could not read the simulator decks", &e));
                false
            }
        }
    }

    fn open_editor(&mut self, series: &[Serie], action: DeckAction) {
        let picked = self.picked.as_ref();
        let (name, target, list, sleeve) = match (action, picked) {
            (DeckAction::Edit, Some(picked)) => (
                picked.deck.name.clone(),
                Some(picked.file.path.clone()),
                DeckList::new(&picked.deck.cards, picked.deck.unmatched.clone()),
                picked.file.sleeve.clone(),
            ),
            (DeckAction::Duplicate, Some(picked)) => (
                format!("{} (copy)", picked.deck.name),
                None,
                DeckList::new(&picked.deck.cards, picked.deck.unmatched.clone()),
                picked.file.sleeve.clone(),
            ),
            _ => (String::new(), None, DeckList::default(), String::new()),
        };
        // Names of the other decks, so the editor can warn about duplicates.
        let taken = self
            .decks
            .iter()
            .filter(|d| Some(&d.path) != target.as_ref())
            .map(|d| d.name.clone())
            .collect();
        if let Some((card, _)) = list.entries().first() {
            self.editor_browser.open_serie_of(series, &card.key);
        }
        self.editor = Some(DeckEditor::new(
            name,
            target,
            sleeve,
            self.sleeves_dir.clone(),
            list,
            taken,
        ));
    }

    /// Writes the edited deck: a new file, or the file being edited.
    fn save_editor(&mut self, ctx: &egui::Context, series: &[Serie], decks_dir: &Path) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let codes = editor.list.codes();
        let date = deck_files::now_date();
        let sleeve = editor.sleeve.trim().to_string();
        let result = match &editor.target {
            Some(path) => deck_files::update_deck(path, &editor.name, &date, &sleeve, &codes),
            None => {
                deck_files::save_new_deck(decks_dir, editor.name.trim(), &date, &sleeve, &codes)
            }
        };
        match result {
            Ok(saved) => {
                self.editor = None;
                if self.reload_and_pick(ctx, &saved.path, series) {
                    self.notice = Some(Notice::Info(format!(
                        "Saved \"{}\" ({} cards) to the simulator decks.",
                        saved.name,
                        codes.len()
                    )));
                }
            }
            Err(e) => editor.error = Some(format!("Could not save: {e:#}")),
        }
    }

    /// Writes the open deck's file with a new sleeve, keeping its name, date and cards.
    fn apply_sleeve(&mut self, ctx: &egui::Context, series: &[Serie], sleeve: String) {
        let Some(picked) = &self.picked else {
            return;
        };
        let file = &picked.file;
        match deck_files::update_deck(&file.path, &file.name, &file.date, &sleeve, &file.codes) {
            Ok(saved) => {
                let what = if sleeve.is_empty() {
                    "now has no sleeve".to_string()
                } else {
                    format!("uses the sleeve {sleeve}")
                };
                let message = format!("\"{}\" {what}.", saved.name);
                if self.reload_and_pick(ctx, &saved.path, series) {
                    self.notice = Some(Notice::Info(message));
                }
            }
            Err(e) => self.notice = Some(Notice::from_error("Could not set the sleeve", &e)),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        series: &Arc<Vec<Serie>>,
        decks_dir: &Path,
        sleeves_dir: &Path,
        account: Option<&Account>,
        // The linked account's session is still being read: no upload yet, it would be
        // anonymous.
        account_pending: bool,
    ) {
        if self.sleeves_dir != sleeves_dir {
            self.sleeves_dir = sleeves_dir.to_path_buf();
            self.sleeve_adder.dir = sleeves_dir.to_path_buf();
            self.refresh_sleeves();
        }
        self.poll_import(decks_dir);
        self.poll_upload();
        self.poll_export();

        if self.editor.is_some() {
            self.show_editor(ui, series, decks_dir);
            return;
        }

        let mut clicked = None;
        theme::list_pane(ui, "deck_list", |ui| {
            clicked = self.deck_list(ui, decks_dir)
        });
        if let Some(path) = clicked {
            self.pick(ui.ctx(), &path, series);
        }
        if self.picked.is_some() {
            theme::detail_pane(ui, "deck_card_panel", 16, |ui| {
                self.preview.pane().show(ui, None);
            });
        }
        let s = scheme(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                left: 12,
                right: 8,
                top: 16,
                bottom: 0,
            }))
            .show(ui, |ui| self.deck_page(ui, series));

        match self.action.take() {
            Some(DeckAction::Upload) => self.open_upload(),
            Some(DeckAction::Print(printout)) => self.open_export(printout),
            Some(action) => self.open_editor(series, action),
            None => {}
        }

        self.import_dialog(ui.ctx(), series, decks_dir);
        self.upload_dialog(ui.ctx(), account, account_pending);
        self.export_dialog(ui.ctx());
    }

    /// The deck editor with its card browser, instead of the deck list and page.
    fn show_editor(&mut self, ui: &mut Ui, series: &[Serie], decks_dir: &Path) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let title = editor.title();
        self.editor_browser
            .show(ui, series, title, Some(editor as &mut dyn Picker));
        match editor.take_action() {
            Some(EditorAction::Save) => self.save_editor(ui.ctx(), series, decks_dir),
            Some(EditorAction::Cancel) if editor.is_dirty() => self.confirm_discard = true,
            Some(EditorAction::Cancel) => self.editor = None,
            None => {}
        }

        if self.confirm_discard {
            let name = self
                .editor
                .as_ref()
                .map(|e| e.name.trim().to_string())
                .unwrap_or_default();
            let what = if name.is_empty() {
                "This deck".to_string()
            } else {
                format!("\"{name}\"")
            };
            let message = format!("{what} has changes that are not saved.");
            let answer = theme::confirm_dialog(
                ui.ctx(),
                "discard_deck",
                "Discard changes?",
                &message,
                "Discard",
            );
            match answer {
                Some(true) => {
                    self.editor = None;
                    self.confirm_discard = false;
                }
                Some(false) => self.confirm_discard = false,
                None => {}
            }
        }
    }

    /// The list of deck files, with the New deck and Import buttons. Returns the deck file the
    /// user clicked.
    fn deck_list(&mut self, ui: &mut Ui, decks_dir: &Path) -> Option<PathBuf> {
        let s = scheme(ui);
        let mut clicked = None;
        let own = self.decks.iter().filter(|d| !d.ai).count();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(theme::headline("Decks").color(s.on_surface));
            ui.label(theme::label(own.to_string()).color(s.on_surface_variant));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let other = self.sort.other();
                let sort = theme::text(Some(ICON_SORT), self.sort.label())
                    .show(ui)
                    .on_hover_text(format!("Sort by {}", other.label().to_lowercase()));
                if sort.clicked() {
                    self.set_sort(other);
                    self.sort_changed = true;
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let half = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            if theme::filled(Some(ICON_ADD), "New deck")
                .min_width(half)
                .show(ui)
                .clicked()
            {
                self.action = Some(DeckAction::NewDeck);
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
            let width = ui.available_width() - 16.0;
            theme::search_bar(ui, &mut self.query, "Find a deck", width);
            ui.add_space(4.0);
        }
        let query = self.query.trim().to_lowercase();
        let picked_path = self.picked.as_ref().map(|p| &p.file.path);
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            // The user's decks, then the AI opponents' ones.
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
                    let selected = picked_path == Some(&deck.path);
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
                        clicked = Some(deck.path.clone());
                    }
                }
            }
        });
        clicked
    }

    /// The open deck: name, sleeve, buttons, stats and cards.
    fn deck_page(&mut self, ui: &mut Ui, series: &[Serie]) {
        let s = scheme(ui);
        let Some(picked) = &self.picked else {
            theme::empty_state(
                ui,
                ICON_STACKS,
                "Pick a deck",
                "Choose a deck on the left, build a new one, or import one from Encore Decks.",
            );
            return;
        };

        let ai = picked.file.ai;
        let sleeve = picked.file.sleeve.trim().to_string();
        let sleeve_path = self
            .sleeves
            .iter()
            .find(|sl| sl.name.eq_ignore_ascii_case(&sleeve))
            .map(|sl| sl.path.clone());
        let mut action = None;
        let mut picked_sleeve = None;
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let deck = &picked.deck;
            ui.add(egui::Label::new(theme::headline(&deck.name).color(s.on_surface)).wrap());
            if !deck.date.trim().is_empty() {
                ui.label(
                    RichText::new(format!("Saved {}", deck.date.trim()))
                        .color(s.on_surface_variant),
                );
            }
            // The deck's sleeve, changeable right here: the file is rewritten at once.
            ui.horizontal(|ui| {
                ui.label(RichText::new("Sleeve").small().color(s.on_surface_variant));
                if let Some(path) = &sleeve_path {
                    ui.add(
                        assets::image_from_path(path)
                            .fit_to_exact_size(vec2(16.0, 16.0 * CARD_IMAGE_RATIO))
                            .corner_radius(CornerRadius::same(3)),
                    );
                }
                if ai {
                    // AI decks are the simulator's own: shown, not changed.
                    let name = if sleeve.is_empty() {
                        "None"
                    } else {
                        Sleeve::display_name(&sleeve)
                    };
                    ui.label(RichText::new(name).color(s.on_surface_variant));
                } else {
                    sleeve_combo(
                        ui,
                        egui::Id::new("deck_page_sleeve"),
                        200.0,
                        &self.sleeves,
                        &sleeve,
                        &mut picked_sleeve,
                        &mut self.sleeve_adder,
                    )
                    .on_hover_text("The image the simulator puts the deck's cards in");
                }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                if !ai && theme::tonal(Some(ICON_EDIT), "Edit").show(ui).clicked() {
                    action = Some(DeckAction::Edit);
                }
                let duplicate = theme::outlined(Some(ICON_CONTENT_COPY), "Duplicate")
                    .show(ui)
                    .on_hover_text("Build a new deck starting from this one");
                if duplicate.clicked() {
                    action = Some(DeckAction::Duplicate);
                }
                let upload = theme::outlined(Some(ICON_UPLOAD), "Upload")
                    .show(ui)
                    .on_hover_text("Publish this deck on Encore Decks");
                if upload.clicked() {
                    action = Some(DeckAction::Upload);
                }
                ui.add_space(8.0);
                let proxies = theme::outlined(Some(ICON_PRINT), "Proxies")
                    .show(ui)
                    .on_hover_text("PDF of the deck's cards at real size, to print and cut out");
                if proxies.clicked() {
                    action = Some(DeckAction::Print(Printout::Proxies));
                }
                let translation = theme::outlined(Some(ICON_TRANSLATE), "Translation")
                    .show(ui)
                    .on_hover_text("PDF of the cards with their English text");
                if translation.clicked() {
                    action = Some(DeckAction::Print(Printout::Translation));
                }
            });
            ui.add_space(8.0);
            self.preview.show(ui, &picked.summary, &deck.unmatched);
        });
        if action.is_some() {
            self.action = action;
        }
        if let Some(sleeve) = picked_sleeve {
            self.apply_sleeve(ui.ctx(), series, sleeve);
        }
    }
}
