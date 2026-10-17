//! The window: the [`App`] holding every screen, and the code that draws a frame.
//!
//! egui is an "immediate mode" UI library: there are no widget objects to create and update.
//! Instead, [`App::ui`] runs on every frame (many times per second while something moves) and
//! draws the whole window from the app's state; a button is drawn by a function call that
//! returns whether it was clicked. So the screens are plain structs holding their state, each
//! with a `show(ui, …)` method that draws it and reacts to clicks.
//!
//! The screens:
//! - setup (`setup_screen.rs`): picking the simulator folder, then loading the cards;
//! - Cards: the [`card_browser::CardBrowser`];
//! - Decks (`decks_screen/`): the simulator's decks, the deck editor, imports, uploads, PDFs;
//! - Encore Deck (`encore_screen.rs`): the linked Encore Decks account's decks;
//! - Settings (`settings_screen.rs`, `account.rs`).

mod account;
mod assets;
mod background;
mod card_browser;
mod card_view;
mod deck_editor;
mod deck_preview;
mod decks_screen;
mod encore_screen;
mod settings_screen;
mod setup_screen;
mod sleeve_picker;
mod theme;

#[cfg(test)]
mod screenshots;

use std::sync::Arc;

use egui::{Margin, RichText, ThemePreference, Ui};
use egui_material_icons::icons::{
    ICON_CLOUD_DOWNLOAD, ICON_DARK_MODE, ICON_ERROR, ICON_LIGHT_MODE, ICON_SETTINGS, ICON_STACKS,
    ICON_STYLE,
};
use simu_deck_parser::encoredecks::account::{Account, SessionExpired};
use simu_deck_parser::model::Serie;
use simu_deck_parser::settings::{self, Settings};
use simu_deck_parser::simulator::folder::SimulatorFolder;

use account::AccountForm;
use background::Task;
use card_browser::CardBrowser;
use decks_screen::DecksScreen;
use encore_screen::EncoreScreen;
use setup_screen::Setup;
use theme::{Snackbar, scheme};

/// Something a screen wants to tell the user: a snackbar for information, a dialog for errors.
pub enum Notice {
    Info(String),
    Error {
        title: String,
        message: String,
        /// The Encore Decks session expired: the app forgets the linked account.
        session_expired: bool,
    },
}

impl Notice {
    pub fn error(title: &str, message: impl Into<String>) -> Self {
        Notice::Error {
            title: title.to_string(),
            message: message.into(),
            session_expired: false,
        }
    }

    /// An error dialog for a failed operation. `{:#}` prints the error with its causes
    /// ("reading the deck: no such file").
    pub fn from_error(title: &str, error: &anyhow::Error) -> Self {
        Notice::Error {
            title: title.to_string(),
            message: format!("{error:#}"),
            session_expired: error.is::<SessionExpired>(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Screen {
    Setup,
    Cards,
    Decks,
    Encore,
    Settings,
}

impl Screen {
    /// The value saved in the settings, to reopen the screen at the next start.
    fn key(self) -> &'static str {
        match self {
            Screen::Decks => "decks",
            Screen::Encore => "encore",
            _ => "cards",
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum ThemeChoice {
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    /// Each choice with its key in the settings file and its label.
    const ALL: [(ThemeChoice, &'static str, &'static str); 3] = [
        (ThemeChoice::System, "system", "System"),
        (ThemeChoice::Light, "light", "Light"),
        (ThemeChoice::Dark, "dark", "Dark"),
    ];

    fn from_key(key: Option<&str>) -> Self {
        Self::ALL
            .iter()
            .find(|(_, k, _)| Some(*k) == key)
            .map_or(ThemeChoice::System, |(choice, _, _)| *choice)
    }

    fn key(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(choice, _, _)| *choice == self)
            .map_or("system", |(_, key, _)| key)
    }

    fn preference(self) -> ThemePreference {
        match self {
            ThemeChoice::System => ThemePreference::System,
            ThemeChoice::Light => ThemePreference::Light,
            ThemeChoice::Dark => ThemePreference::Dark,
        }
    }
}

pub struct App {
    settings: Settings,
    /// The Encore Decks account decks are uploaded to; uploads are anonymous without one.
    account: Option<Account>,
    account_form: AccountForm,
    screen: Screen,
    /// Screen to open once the simulator is loaded.
    start_screen: Screen,
    theme: ThemeChoice,
    setup: Setup,
    /// Every card of the simulator. `Arc` ("shared pointer") lets background tasks read the
    /// cards too, without copying them.
    series: Arc<Vec<Serie>>,
    /// The simulator whose cards are loaded.
    folder: Option<SimulatorFolder>,
    /// Cards being read, from this folder.
    loading: Option<(SimulatorFolder, Task<Vec<Serie>>)>,
    /// Error dialog on screen: its title and message.
    error_dialog: Option<(String, String)>,
    /// Last serie written to the settings (avoids writing the file on every frame).
    saved_serie: Option<String>,
    snackbar: Option<Snackbar>,
    /// The window was asked to close while a deck had unsaved changes.
    confirm_quit: bool,
    /// The user confirmed quitting anyway.
    quitting: bool,
    card_browser: CardBrowser,
    decks_screen: DecksScreen,
    encore_screen: EncoreScreen,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_settings(cc, Settings::open_default())
    }

    /// The app reading and writing `settings` (tests use a temporary file).
    pub fn with_settings(cc: &eframe::CreationContext<'_>, settings: Settings) -> Self {
        let ctx = &cc.egui_ctx;
        egui_extras::install_image_loaders(ctx);
        theme::install(ctx);
        let theme = ThemeChoice::from_key(settings.get(settings::THEME).as_deref());
        ctx.set_theme(theme.preference());

        let mut setup = Setup::default();
        setup.set_path(settings.get(settings::SIMULATOR_PATH).unwrap_or_default());
        let start_screen = match settings.get(settings::START_SCREEN).as_deref() {
            Some("decks") => Screen::Decks,
            _ => Screen::Cards,
        };
        let account_form = AccountForm::restoring(ctx, settings.get(settings::ENCORE_USER));
        let mut app = Self {
            settings,
            account: None,
            account_form,
            screen: Screen::Setup,
            start_screen,
            theme,
            setup,
            series: Arc::default(),
            folder: None,
            loading: None,
            error_dialog: None,
            saved_serie: None,
            snackbar: None,
            confirm_quit: false,
            quitting: false,
            card_browser: CardBrowser::default(),
            decks_screen: DecksScreen::default(),
            encore_screen: EncoreScreen::default(),
        };
        if app.setup.resolved.is_some() {
            app.load_simulator(ctx, false);
        } else {
            app.setup.start_detecting(ctx);
        }
        app
    }

    fn notify(&mut self, ctx: &egui::Context, notice: Notice) {
        match notice {
            Notice::Info(text) => self.snackbar = Some(Snackbar::new(text)),
            Notice::Error {
                title,
                message,
                session_expired,
            } => {
                if session_expired {
                    self.unlink_account(ctx);
                }
                self.error_dialog = Some((title, message));
            }
        }
    }

    /// Writes a setting, unless it already has this value.
    fn save_setting(&mut self, ctx: &egui::Context, key: &str, value: &str) {
        if self.settings.get(key).as_deref() == Some(value) {
            return;
        }
        if let Err(e) = self.settings.set(key, value) {
            let message = format!("{}: {e}", self.settings.path().display());
            self.notify(ctx, Notice::error("Could not save settings", message));
        }
    }

    fn delete_setting(&mut self, ctx: &egui::Context, key: &str) {
        if let Err(e) = self.settings.delete(key) {
            self.notify(ctx, Notice::error("Could not save settings", e.to_string()));
        }
    }

    fn switch_to(&mut self, ctx: &egui::Context, screen: Screen) {
        if screen == self.screen {
            return;
        }
        // Card pictures take memory: free the ones of the screen being left.
        ctx.forget_all_images();
        if screen == Screen::Decks
            && let Some(folder) = &self.folder
            && let Err(e) = self.decks_screen.reload(&folder.decks())
        {
            self.notify(
                ctx,
                Notice::from_error("Could not read the simulator decks", &e),
            );
        }
        if matches!(screen, Screen::Cards | Screen::Decks) {
            self.save_setting(ctx, settings::START_SCREEN, screen.key());
        }
        self.screen = screen;
    }

    fn set_theme(&mut self, ctx: &egui::Context, choice: ThemeChoice) {
        self.theme = choice;
        ctx.set_theme(choice.preference());
        self.save_setting(ctx, settings::THEME, choice.key());
    }

    /// The column of screen buttons on the left.
    fn navigation_rail(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        let mut target = None;
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.label(ICON_STYLE.rich_text().size(28.0).color(s.primary))
                .on_hover_text("SimuDeckParser");
            ui.add_space(20.0);
            for (screen, icon, label) in [
                (Screen::Cards, ICON_STYLE, "Cards"),
                (Screen::Decks, ICON_STACKS, "Decks"),
                (Screen::Encore, ICON_CLOUD_DOWNLOAD, "Encore Deck"),
            ] {
                if theme::rail_item(ui, icon, label, self.screen == screen).clicked() {
                    target = Some(screen);
                }
                ui.add_space(4.0);
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                let settings = self.screen == Screen::Settings;
                if theme::rail_item(ui, ICON_SETTINGS, "Settings", settings).clicked() {
                    target = Some(Screen::Settings);
                }
                let dark = ui.visuals().dark_mode;
                let (icon, tip, other) = if dark {
                    (ICON_LIGHT_MODE, "Switch to light theme", ThemeChoice::Light)
                } else {
                    (ICON_DARK_MODE, "Switch to dark theme", ThemeChoice::Dark)
                };
                if theme::icon_button(icon, tip).show(ui).clicked() {
                    self.set_theme(ui.ctx(), other);
                }
            });
        });
        if let Some(screen) = target {
            self.switch_to(ui.ctx(), screen);
        }
    }

    /// Asks before closing the window over a deck with unsaved changes.
    fn quit_guard(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quitting
            && self.decks_screen.has_unsaved_deck()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_quit = true;
        }
        if !self.confirm_quit {
            return;
        }
        let answer = theme::confirm_dialog(
            ctx,
            "confirm_quit",
            "Quit without saving?",
            "The deck you are building has changes that are not saved.",
            "Quit",
        );
        match answer {
            Some(true) => {
                self.quitting = true;
                self.confirm_quit = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(false) => {
                self.confirm_quit = false;
                self.switch_to(ctx, Screen::Decks);
            }
            None => {}
        }
    }

    fn show_error_dialog(&mut self, ctx: &egui::Context) {
        let Some((title, message)) = &self.error_dialog else {
            return;
        };
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("message"))
            .frame(theme::dialog_frame(ctx))
            .show(ctx, |ui| {
                let s = scheme(ui);
                ui.set_width(420.0);
                ui.vertical_centered(|ui| {
                    ui.label(ICON_ERROR.rich_text().size(28.0).color(s.error));
                    ui.label(theme::headline(title).color(s.on_surface));
                });
                ui.add_space(12.0);
                let message = RichText::new(message).color(s.on_surface_variant);
                ui.add(egui::Label::new(message).wrap());
                ui.add_space(20.0);
                theme::actions_row(ui, |ui| {
                    close = theme::text(None, "OK").show(ui).clicked();
                });
            });
        if close || modal.should_close() {
            self.error_dialog = None;
        }
    }

    /// The screen picked in the navigation rail.
    fn current_screen(&mut self, ui: &mut Ui) {
        let decks_dir = self.folder.as_ref().map(|f| f.decks()).unwrap_or_default();
        let account_pending = self.account_form.is_restoring();
        match self.screen {
            Screen::Cards => {
                self.card_browser.show(ui, &self.series, "Cards", None);
                // Remember the serie for the next start.
                let serie = self
                    .card_browser
                    .serie_name(&self.series)
                    .map(str::to_owned);
                if serie.is_some() && serie != self.saved_serie {
                    let ctx = ui.ctx().clone();
                    self.save_setting(&ctx, settings::SERIE, serie.as_deref().unwrap_or_default());
                    self.saved_serie = serie;
                }
            }
            Screen::Decks => {
                let sleeves_dir = self
                    .folder
                    .as_ref()
                    .map(|f| f.sleeves())
                    .unwrap_or_default();
                self.decks_screen.show(
                    ui,
                    &self.series,
                    &decks_dir,
                    &sleeves_dir,
                    self.account.as_ref(),
                    account_pending,
                );
            }
            Screen::Encore => {
                self.encore_screen.show(
                    ui,
                    &self.series,
                    &decks_dir,
                    self.account.as_ref(),
                    account_pending,
                );
            }
            Screen::Settings => {
                let s = scheme(ui);
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(s.surface)
                            .inner_margin(Margin::same(24)),
                    )
                    .show(ui, |ui| self.settings_screen(ui));
            }
            Screen::Setup => {}
        }
    }
}

/// `eframe::App` is the interface eframe (egui's window host) calls: `ui` draws one frame.
impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_loading(&ctx);
        self.poll_account(&ctx);
        let s = scheme(ui);

        if self.screen == Screen::Setup {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(s.surface)
                        .inner_margin(Margin::same(24)),
                )
                .show(ui, |ui| self.setup_screen(ui));
        } else {
            egui::Panel::left("navigation_rail")
                .exact_size(96.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(s.surface_container)
                        .inner_margin(Margin::symmetric(8, 8)),
                )
                .show(ui, |ui| self.navigation_rail(ui));
            self.current_screen(ui);
        }

        // What the screens have to say.
        let notices = [
            self.decks_screen.take_notice(),
            self.encore_screen.take_notice(),
        ];
        for notice in notices.into_iter().flatten() {
            self.notify(&ctx, notice);
        }

        // Windows drawn over the screens.
        if let Some(bar) = &mut self.snackbar
            && !bar.show(&ctx)
        {
            self.snackbar = None;
        }
        if self.screen != Screen::Setup {
            card_view::related_window(&ctx);
        }
        self.show_error_dialog(&ctx);
        self.quit_guard(&ctx);
    }
}
