//! Encore: the linked account's decks on encoredecks.com, browsed one page at a time,
//! previewed with the loaded simulator cards, and imported into the simulator.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Margin, RichText, ScrollArea, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ACCOUNT_CIRCLE, ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_CLOUD_DOWNLOAD,
    ICON_DOWNLOAD, ICON_OPEN_IN_NEW, ICON_REFRESH,
};
use simu_deck_parser::codes::CardIndex;
use simu_deck_parser::decks;
use simu_deck_parser::encore::{self, Account, SessionExpired, build_deck};
use simu_deck_parser::model::{Deck, Serie};

use super::deck_preview::{DeckPreview, Summary};
use super::theme::{self, scheme};
use super::{Notice, Task};

/// One page of the account's decks, or the message and whether the session had expired.
type PageResult = Result<encore::DeckPage, (String, bool)>;
/// The picked deck with its resolved cards, or the message.
type PreviewResult = Result<(Deck, Vec<String>), String>;
/// The imported deck, or the message.
type ImportResult = Result<(Deck, Vec<String>), String>;

/// The Encore screen: the linked account's decks on encoredecks.com (24 per page, private
/// and unfinished ones included), previewed before being imported into the simulator.
#[derive(Default)]
pub struct EncorePage {
    /// 1-based page shown.
    page: u32,
    /// The page's decks, once loaded.
    loaded: Option<encore::DeckPage>,
    task: Option<Task<PageResult>>,
    /// The page could not be read: no automatic retry, only the refresh button and the
    /// pagination start one.
    failed: bool,
    /// Deck picked on the left, previewed in the middle.
    picked: Option<encore::AccountDeck>,
    /// The picked deck's resolved cards and unmatched codes.
    resolved: Option<(Deck, Vec<String>)>,
    summary: Option<Summary>,
    preview_task: Option<Task<PreviewResult>>,
    import_task: Option<Task<ImportResult>>,
    /// Card groups and detail pane of the previewed deck.
    pane: DeckPreview,
    /// Names of the simulator's decks, to mark the imported ones; loaded in the
    /// background.
    local: Vec<String>,
    local_task: Option<Task<Result<Vec<String>, String>>>,
    local_loaded: bool,
    /// A page found the linked account's session expired; taken by the app.
    session_expired: bool,
    /// Message for the user, taken by the app.
    notice: Option<Notice>,
}

impl EncorePage {
    pub fn take_notice(&mut self) -> Option<Notice> {
        self.notice.take()
    }

    /// Whether the linked account must be forgotten (its session expired).
    pub fn take_session_expired(&mut self) -> bool {
        std::mem::take(&mut self.session_expired)
    }

    /// Forgets everything: the account is gone.
    fn reset(&mut self) {
        self.page = 1;
        self.loaded = None;
        self.task = None;
        self.failed = false;
        self.picked = None;
        self.resolved = None;
        self.summary = None;
        self.preview_task = None;
        self.import_task = None;
        self.pane.clear();
        self.local.clear();
        self.local_task = None;
        self.local_loaded = false;
    }

    /// Loads the account's `page`th deck page off the UI thread.
    fn fetch_page(&mut self, account: &Account, ctx: &egui::Context, page: u32) {
        let session = account.session.clone();
        self.task = Some(Task::spawn(ctx, move || {
            encore::fetch_account_decks(&session, page)
                .map_err(|e| (format!("{e:#}"), e.is::<SessionExpired>()))
        }));
    }

    /// Finishes the background work: loading a page, the local deck names, the preview
    /// and the import.
    fn poll(&mut self) {
        if let Some(result) = self.task.as_ref().and_then(Task::poll) {
            self.task = None;
            match result {
                Some(Ok(page)) => {
                    self.page = page.page.max(1);
                    self.loaded = Some(page);
                }
                Some(Err((e, expired))) => {
                    self.session_expired |= expired;
                    self.notice = Some(Notice::Error("Could not read your decks".into(), e));
                    self.failed = true;
                    self.loaded = None;
                    self.task = None;
                }
                None => {
                    self.notice = Some(Notice::Error(
                        "Could not read your decks".into(),
                        "Reading the decks stopped unexpectedly.".into(),
                    ));
                    self.failed = true;
                    self.loaded = None;
                    self.task = None;
                }
            }
        }
        if let Some(result) = self.local_task.as_ref().and_then(Task::poll) {
            self.local_task = None;
            self.local_loaded = true;
            if let Some(Ok(names)) = result {
                self.local = names;
            }
        }
        if let Some(result) = self.preview_task.as_ref().and_then(Task::poll) {
            self.preview_task = None;
            match result {
                Some(Ok((deck, missing))) => {
                    self.summary = Some(Summary::new(&deck.cards));
                    self.resolved = Some((deck, missing));
                }
                Some(Err(e)) => {
                    self.notice = Some(Notice::Error("Could not read the deck".into(), e));
                    self.picked = None;
                    self.resolved = None;
                    self.summary = None;
                    self.pane.clear();
                }
                None => {
                    self.notice = Some(Notice::Error(
                        "Could not read the deck".into(),
                        "Reading the deck stopped unexpectedly.".into(),
                    ));
                    self.picked = None;
                    self.resolved = None;
                    self.summary = None;
                    self.pane.clear();
                }
            }
        }
        if let Some(result) = self.import_task.as_ref().and_then(Task::poll) {
            self.import_task = None;
            self.notice = Some(match result {
                Some(Ok((deck, missing))) => {
                    // Refresh the "in the simulator" marks.
                    self.local_loaded = false;
                    let mut msg = format!(
                        "Imported \"{}\" ({} cards) into the simulator decks.",
                        deck.name,
                        deck.cards.len()
                    );
                    if !missing.is_empty() {
                        msg += &format!(" {} code(s) matched no card.", missing.len());
                    }
                    Notice::Info(msg)
                }
                Some(Err(e)) => Notice::Error("Import failed".into(), e),
                None => Notice::Error(
                    "Import failed".into(),
                    "The import stopped unexpectedly.".into(),
                ),
            });
        }
    }

    /// Previews `deck`: its cards are read from encoredecks.com and resolved.
    fn pick(&mut self, ctx: &egui::Context, deck: encore::AccountDeck, series: &Arc<Vec<Serie>>) {
        self.picked = Some(deck.clone());
        self.resolved = None;
        self.summary = None;
        self.pane.clear();
        let id = deck.id.clone();
        let series = Arc::clone(series);
        self.preview_task = Some(Task::spawn(ctx, move || {
            let fetched = encore::fetch_deck(&id).map_err(|e| format!("{e:#}"))?;
            let index = CardIndex::new(&series);
            Ok(build_deck(&index, &fetched.name, &fetched.date, &fetched.codes))
        }));
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        series: &Arc<Vec<Serie>>,
        decks_dir: &Path,
        account: Option<&Account>,
        // The linked account's session is still being read.
        account_pending: bool,
    ) {
        self.poll();
        let s = scheme(ui);
        let Some(account) = account else {
            if !account_pending {
                self.reset();
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin::same(24)))
                .show(ui, |ui| {
                    if account_pending {
                        ui.vertical_centered(|ui| {
                            ui.add_space(64.0);
                            ui.add(egui::Spinner::new().size(28.0).color(s.primary));
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Reading the session from the system keyring…")
                                    .color(s.on_surface_variant),
                            );
                        });
                    } else {
                        theme::empty_state(
                            ui,
                            ICON_ACCOUNT_CIRCLE,
                            "No account linked",
                            "Link your Encore Decks account in Settings to browse its decks \
                             and import them into the simulator.",
                        );
                    }
                });
            return;
        };

        // First visit (and after a refresh): read the page and the simulator's decks.
        if self.loaded.is_none() && self.task.is_none() && !self.failed {
            self.fetch_page(account, ui.ctx(), self.page.max(1));
        }
        if !self.local_loaded && self.local_task.is_none() {
            let decks_dir: PathBuf = decks_dir.to_path_buf();
            self.local_task = Some(Task::spawn(ui.ctx(), move || {
                decks::list_decks(&decks_dir)
                    .map(|files| {
                        files
                            .iter()
                            .filter(|d| !d.ai)
                            .map(|d| d.name.clone())
                            .collect()
                    })
                    .map_err(|e| format!("{e:#}"))
            }));
        }

        let mut pick = None;
        egui::Panel::left("encore_deck_list")
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
                        pick = self.deck_list(ui, account);
                    });
            });
        if let Some(deck) = pick {
            self.pick(ui.ctx(), deck, series);
        }

        if self.summary.is_some() {
            egui::Panel::right("encore_card_panel")
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
                        self.pane.pane().show(ui);
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
            .show(ui, |ui| self.deck_content(ui, series, decks_dir));
    }

    /// Returns the deck the user clicked.
    fn deck_list(&mut self, ui: &mut Ui, account: &Account) -> Option<encore::AccountDeck> {
        let s = scheme(ui);
        let mut pick = None;
        let loading = self.task.is_some();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(theme::headline("Encore Decks").color(s.on_surface));
            if let Some(page) = &self.loaded {
                ui.label(theme::label(page.total_decks.to_string()).color(s.on_surface_variant));
            }
            let refresh = theme::icon_button(ICON_REFRESH, "Read the list again")
                .enabled(!loading)
                .show(ui);
            if refresh.clicked() {
                self.failed = false;
                self.loaded = None;
            }
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(4.0);
            ui.label(ICON_ACCOUNT_CIRCLE.rich_text().size(16.0).color(s.primary));
            ui.label(RichText::new(&account.name).color(s.on_surface_variant).small());
        });
        ui.add_space(6.0);

        let (page, total_pages) = self
            .loaded
            .as_ref()
            .map(|p| (p.page, p.total_pages))
            .unwrap_or((self.page.max(1), self.page.max(1)));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let prev = theme::icon_button(ICON_ARROW_BACK, "Previous page")
                .enabled(!loading && page > 1)
                .show(ui);
            ui.label(
                theme::label(format!("Page {page} of {total_pages}")).color(s.on_surface_variant),
            );
            let next = theme::icon_button(ICON_ARROW_FORWARD, "Next page")
                .enabled(!loading && page < total_pages)
                .show(ui);
            if prev.clicked() {
                self.failed = false;
                self.loaded = None;
                self.page = page - 1;
                self.fetch_page(account, ui.ctx(), page - 1);
            }
            if next.clicked() {
                self.failed = false;
                self.loaded = None;
                self.page = page + 1;
                self.fetch_page(account, ui.ctx(), page + 1);
            }
        });
        ui.add_space(4.0);

        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if loading {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    ui.add(egui::Spinner::new().size(24.0).color(s.primary));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Reading your decks…").color(s.on_surface_variant));
                });
                return;
            }
            let Some(page) = &self.loaded else {
                if self.failed {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new(
                            "Could not read the list. Press the reload button above to try \
                             again.",
                        )
                        .color(s.on_surface_variant),
                    );
                }
                return;
            };
            if page.decks.is_empty() {
                ui.add_space(24.0);
                ui.label(
                    RichText::new("This account has no decks yet.")
                        .color(s.on_surface_variant),
                );
                return;
            }
            for deck in &page.decks {
                let selected = self.picked.as_ref().is_some_and(|p| p.id == deck.id);
                let mut supporting = deck.date.trim().to_string();
                if !deck.description.is_empty() {
                    if !supporting.is_empty() {
                        supporting += " · ";
                    }
                    supporting += &deck.description;
                }
                if self.local.iter().any(|n| n == &deck.name) {
                    if !supporting.is_empty() {
                        supporting += " · ";
                    }
                    supporting += "in the simulator";
                }
                if theme::list_item(ui, &deck.name, &supporting, selected).clicked() && !selected
                {
                    pick = Some(deck.clone());
                }
            }
        });
        pick
    }

    /// The previewed deck: its cards, and the import button.
    fn deck_content(
        &mut self,
        ui: &mut Ui,
        series: &Arc<Vec<Serie>>,
        decks_dir: &Path,
    ) {
        let s = scheme(ui);
        let Some(picked) = self.picked.clone() else {
            theme::empty_state(
                ui,
                ICON_CLOUD_DOWNLOAD,
                "Pick a deck",
                "Choose one of your decks on the left to see its cards, then import it into \
                 the simulator.",
            );
            return;
        };
        if self.preview_task.is_some() {
            ui.vertical_centered(|ui| {
                ui.add_space(64.0);
                ui.add(egui::Spinner::new().size(28.0).color(s.primary));
                ui.add_space(8.0);
                ui.label(RichText::new("Reading the deck…").color(s.on_surface_variant));
            });
            return;
        }
        let (Some((deck, missing)), Some(summary)) = (&self.resolved, &self.summary) else {
            return;
        };
        let mut import = false;
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add(egui::Label::new(theme::headline(&deck.name).color(s.on_surface)).wrap());
            let saved = if deck.date.trim().is_empty() {
                "On encoredecks.com".to_string()
            } else {
                format!("On encoredecks.com, saved {}", deck.date.trim())
            };
            ui.label(RichText::new(saved).color(s.on_surface_variant));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                let running = self.import_task.is_some();
                let button = theme::filled(Some(ICON_DOWNLOAD), "Import")
                    .enabled(!running)
                    .show(ui)
                    .on_hover_text(
                        "Save this deck as a new file in the simulator's Decks folder; \
                         existing decks are never overwritten",
                    );
                if button.clicked() {
                    import = true;
                }
                let open = theme::outlined(Some(ICON_OPEN_IN_NEW), "Open on the site")
                    .show(ui)
                    .on_hover_text("Open this deck in your browser");
                if open.clicked() {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(encore::deck_link(&picked.id)));
                }
                if running {
                    ui.label(RichText::new("Importing…").color(s.on_surface_variant));
                    ui.spinner();
                }
            });
            ui.add_space(8.0);
            self.pane.show(ui, summary, missing);
        });
        if !import {
            return;
        }
        let link = encore::deck_link(&picked.id);
        let series = Arc::clone(series);
        let decks_dir: PathBuf = decks_dir.to_path_buf();
        self.import_task = Some(Task::spawn(ui.ctx(), move || {
            encore::import(&link, &series, &decks_dir).map_err(|e| format!("{e:#}"))
        }));
    }

    /// Screenshots only: shows a page already loaded, without contacting encoredecks.com.
    #[cfg(test)]
    pub(super) fn open_loaded(&mut self, page: encore::DeckPage) {
        self.page = page.page.max(1);
        self.loaded = Some(page);
        self.local_loaded = true;
    }

    /// Screenshots only: previews a deck as if it had been read and resolved.
    #[cfg(test)]
    pub(super) fn preview_loaded(
        &mut self,
        picked: encore::AccountDeck,
        resolved: (Deck, Vec<String>),
    ) {
        self.summary = Some(Summary::new(&resolved.0.cards));
        self.resolved = Some(resolved);
        self.picked = Some(picked);
    }
}
