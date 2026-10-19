//! The Encore Deck screen: the linked account's decks on encoredecks.com (24 per page, private
//! and unfinished ones included), previewed with the loaded cards and imported into the
//! simulator.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Margin, RichText, ScrollArea, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ACCOUNT_CIRCLE, ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_CLOUD_DOWNLOAD, ICON_DOWNLOAD,
    ICON_OPEN_IN_NEW, ICON_REFRESH,
};
use simu_deck_parser::card_index::CardIndex;
use simu_deck_parser::encoredecks::account::Account;
use simu_deck_parser::encoredecks::decks::{
    AccountDeck, DeckPage, deck_link, fetch_account_decks, fetch_deck, import,
};
use simu_deck_parser::model::{Deck, Serie};
use simu_deck_parser::simulator::deck_files;

use super::Notice;
use super::background::{Task, finished};
use super::deck_preview::{DeckPreview, Summary};
use super::decks_screen::import_message;
use super::theme::{self, scheme};

#[derive(Default)]
pub struct EncoreScreen {
    /// 1-based page shown.
    page: u32,
    /// The page's decks, once loaded.
    loaded: Option<DeckPage>,
    loading: Option<Task<DeckPage>>,
    /// The page could not be read: no automatic retry, only the refresh button and the
    /// page buttons start one.
    failed: bool,
    /// Deck picked on the left, previewed in the middle.
    picked: Option<AccountDeck>,
    /// The picked deck's cards, matched to the loaded simulator, and their stats.
    resolved: Option<(Deck, Summary)>,
    /// Reading the picked deck's cards.
    previewing: Option<Task<Deck>>,
    importing: Option<Task<Deck>>,
    /// Card groups and detail pane of the previewed deck.
    preview: DeckPreview,
    /// Names of the simulator's decks, to mark the decks already imported; read in the
    /// background.
    local_names: Vec<String>,
    reading_local_names: Option<Task<Vec<String>>>,
    local_names_read: bool,
    /// Message for the user, taken by the app.
    notice: Option<Notice>,
}

impl EncoreScreen {
    pub fn take_notice(&mut self) -> Option<Notice> {
        self.notice.take()
    }

    /// Forgets everything: the account is gone.
    fn reset(&mut self) {
        *self = Self {
            page: 1,
            notice: self.notice.take(),
            ..Self::default()
        };
    }

    /// Loads the account's `page`th deck page off the UI thread.
    fn fetch_page(&mut self, account: &Account, ctx: &egui::Context, page: u32) {
        let session = account.session.clone();
        self.loading = Some(Task::spawn(ctx, move || {
            fetch_account_decks(&session, page)
        }));
    }

    /// Asks for another page (or the same one again, to refresh).
    fn go_to_page(&mut self, account: &Account, ctx: &egui::Context, page: u32) {
        self.failed = false;
        self.loaded = None;
        self.page = page;
        self.fetch_page(account, ctx, page);
    }

    /// Finishes the background work: loading a page, the local deck names, the preview
    /// and the import.
    fn poll(&mut self) {
        match finished(&mut self.loading) {
            Some(Ok(page)) => {
                self.page = page.page.max(1);
                self.loaded = Some(page);
            }
            Some(Err(e)) => {
                // An expired session is noted in the notice: the app then unlinks the account.
                self.notice = Some(Notice::from_error("Could not read your decks", &e));
                self.failed = true;
                self.loaded = None;
            }
            None => {}
        }

        if let Some(result) = finished(&mut self.reading_local_names) {
            self.local_names_read = true;
            if let Ok(names) = result {
                self.local_names = names;
            }
        }

        match finished(&mut self.previewing) {
            Some(Ok(deck)) => {
                let summary = Summary::new(&deck.cards);
                self.resolved = Some((deck, summary));
            }
            Some(Err(e)) => {
                self.notice = Some(Notice::from_error("Could not read the deck", &e));
                self.picked = None;
                self.resolved = None;
                self.preview.clear();
            }
            None => {}
        }

        match finished(&mut self.importing) {
            Some(Ok(deck)) => {
                // Read the local names again, to mark the deck "in the simulator".
                self.local_names_read = false;
                self.notice = Some(Notice::Info(import_message(&deck)));
            }
            Some(Err(e)) => self.notice = Some(Notice::from_error("Import failed", &e)),
            None => {}
        }
    }

    /// Previews `deck`: its cards are read from encoredecks.com and matched to loaded cards.
    fn pick(&mut self, ctx: &egui::Context, deck: AccountDeck, series: &Arc<Vec<Serie>>) {
        let id = deck.id.clone();
        self.picked = Some(deck);
        self.resolved = None;
        self.preview.clear();
        let series = Arc::clone(series);
        self.previewing = Some(Task::spawn(ctx, move || {
            let fetched = fetch_deck(&id)?;
            let index = CardIndex::new(&series);
            Ok(index.resolve_deck(&fetched.name, &fetched.date, &fetched.codes))
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
                .frame(
                    egui::Frame::new()
                        .fill(s.surface)
                        .inner_margin(Margin::same(24)),
                )
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

        // First visit (and after a refresh): read the page and the simulator's deck names.
        if self.loaded.is_none() && self.loading.is_none() && !self.failed {
            self.fetch_page(account, ui.ctx(), self.page.max(1));
        }
        if !self.local_names_read && self.reading_local_names.is_none() {
            let decks_dir: PathBuf = decks_dir.to_path_buf();
            self.reading_local_names = Some(Task::spawn(ui.ctx(), move || {
                let files = deck_files::list_decks(&decks_dir)?;
                Ok(files
                    .into_iter()
                    .filter(|d| !d.ai)
                    .map(|d| d.name)
                    .collect())
            }));
        }

        let mut clicked = None;
        theme::list_pane(ui, "encore_deck_list", |ui| {
            clicked = self.deck_list(ui, account);
        });
        if let Some(deck) = clicked {
            self.pick(ui.ctx(), deck, series);
        }
        if self.resolved.is_some() {
            theme::detail_pane(ui, "encore_card_panel", 16, |ui| {
                self.preview.pane().show(ui, None);
            });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(s.surface).inner_margin(Margin {
                left: 12,
                right: 8,
                top: 16,
                bottom: 0,
            }))
            .show(ui, |ui| self.deck_page(ui, series, decks_dir));
    }

    /// The account's decks, with page buttons. Returns the deck the user clicked.
    fn deck_list(&mut self, ui: &mut Ui, account: &Account) -> Option<AccountDeck> {
        let s = scheme(ui);
        let mut clicked = None;
        let loading = self.loading.is_some();
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
            ui.label(
                RichText::new(&account.name)
                    .color(s.on_surface_variant)
                    .small(),
            );
        });
        ui.add_space(6.0);

        let (page, total_pages) = self
            .loaded
            .as_ref()
            .map(|p| (p.page, p.total_pages))
            .unwrap_or((self.page.max(1), self.page.max(1)));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let previous = theme::icon_button(ICON_ARROW_BACK, "Previous page")
                .enabled(!loading && page > 1)
                .show(ui);
            ui.label(
                theme::label(format!("Page {page} of {total_pages}")).color(s.on_surface_variant),
            );
            let next = theme::icon_button(ICON_ARROW_FORWARD, "Next page")
                .enabled(!loading && page < total_pages)
                .show(ui);
            if previous.clicked() {
                self.go_to_page(account, ui.ctx(), page - 1);
            }
            if next.clicked() {
                self.go_to_page(account, ui.ctx(), page + 1);
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
                    RichText::new("This account has no decks yet.").color(s.on_surface_variant),
                );
                return;
            }
            for deck in &page.decks {
                let selected = self.picked.as_ref().is_some_and(|p| p.id == deck.id);
                // "date · description · in the simulator", leaving out what is empty.
                let mut details = vec![deck.date.trim(), deck.description.as_str()];
                if self.local_names.contains(&deck.name) {
                    details.push("in the simulator");
                }
                details.retain(|d| !d.is_empty());
                let supporting = details.join(" · ");
                if theme::list_item(ui, &deck.name, &supporting, selected).clicked() && !selected {
                    clicked = Some(deck.clone());
                }
            }
        });
        clicked
    }

    /// The previewed deck: its cards, and the Import button.
    fn deck_page(&mut self, ui: &mut Ui, series: &Arc<Vec<Serie>>, decks_dir: &Path) {
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
        if self.previewing.is_some() {
            ui.vertical_centered(|ui| {
                ui.add_space(64.0);
                ui.add(egui::Spinner::new().size(28.0).color(s.primary));
                ui.add_space(8.0);
                ui.label(RichText::new("Reading the deck…").color(s.on_surface_variant));
            });
            return;
        }
        let Some((deck, summary)) = &self.resolved else {
            return;
        };
        let mut import_clicked = false;
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
                let running = self.importing.is_some();
                let button = theme::filled(Some(ICON_DOWNLOAD), "Import")
                    .enabled(!running)
                    .show(ui)
                    .on_hover_text(
                        "Save this deck as a new file in the simulator's Decks folder; \
                         existing decks are never overwritten",
                    );
                if button.clicked() {
                    import_clicked = true;
                }
                let open = theme::outlined(Some(ICON_OPEN_IN_NEW), "Open on the site")
                    .show(ui)
                    .on_hover_text("Open this deck in your browser");
                if open.clicked() {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(deck_link(&picked.id)));
                }
                if running {
                    ui.label(RichText::new("Importing…").color(s.on_surface_variant));
                    ui.spinner();
                }
            });
            ui.add_space(8.0);
            self.preview.show(ui, summary, &deck.unmatched);
        });
        if !import_clicked {
            return;
        }
        let link = deck_link(&picked.id);
        let series = Arc::clone(series);
        let decks_dir: PathBuf = decks_dir.to_path_buf();
        self.importing = Some(Task::spawn(ui.ctx(), move || {
            import(&link, &series, &decks_dir)
        }));
    }

    /// Screenshots only: shows a page already loaded, without contacting encoredecks.com.
    #[cfg(test)]
    pub(super) fn open_loaded(&mut self, page: DeckPage) {
        self.page = page.page.max(1);
        self.loaded = Some(page);
        self.local_names_read = true;
    }

    /// Screenshots only: previews a deck as if it had been read and matched.
    #[cfg(test)]
    pub(super) fn preview_loaded(&mut self, picked: AccountDeck, deck: Deck) {
        let summary = Summary::new(&deck.cards);
        self.resolved = Some((deck, summary));
        self.picked = Some(picked);
    }
}
