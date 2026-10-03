mod card_list;
mod card_view;
mod deck_editor;
mod deck_preview;
mod deck_viewer;
mod encore_page;
mod theme;

#[cfg(test)]
mod screenshots;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use egui::{Margin, RichText, ScrollArea, ThemePreference, Ui};
use egui_material_icons::icons::{
    ICON_ACCOUNT_CIRCLE, ICON_BRIGHTNESS_AUTO, ICON_CHECK_CIRCLE, ICON_CLOUD_DOWNLOAD,
    ICON_DARK_MODE, ICON_ERROR, ICON_FOLDER, ICON_FOLDER_OPEN, ICON_LIGHT_MODE, ICON_LOGIN,
    ICON_LOGOUT, ICON_REFRESH, ICON_SETTINGS, ICON_STACKS, ICON_STYLE,
};
use simu_deck_parser::codes::NameIndex;
use simu_deck_parser::credentials;
use simu_deck_parser::encore::{self, Account};
use simu_deck_parser::layout::Layout;
use simu_deck_parser::model::Serie;
use simu_deck_parser::parser::load_simulator;
use simu_deck_parser::settings::{ENCORE_USER, SIMULATOR_PATH, Settings};
use zeroize::{Zeroize, Zeroizing};

use card_list::CardList;
use deck_viewer::DeckViewer;
use encore_page::EncorePage;
use theme::{Snackbar, scheme};

const THEME_KEY: &str = "ui_theme";
const SCREEN_KEY: &str = "ui_screen";
const SERIE_KEY: &str = "ui_serie";

/// Work running on a background thread; polled every frame.
pub struct Task<T> {
    rx: Receiver<T>,
}

impl<T: Send + 'static> Task<T> {
    pub fn spawn(ctx: &egui::Context, job: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
        Self { rx }
    }

    /// `Some` once the job is done (or its thread died, which is reported as `None` inside).
    pub fn poll(&self) -> Option<Option<T>> {
        match self.rx.try_recv() {
            Ok(v) => Some(Some(v)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}

/// Something to tell the user: a snackbar for information, a dialog for errors.
pub enum Notice {
    Info(String),
    Error(String, String),
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
    const ALL: [(ThemeChoice, &'static str, &'static str); 3] = [
        (ThemeChoice::System, "system", "System"),
        (ThemeChoice::Light, "light", "Light"),
        (ThemeChoice::Dark, "dark", "Dark"),
    ];

    fn from_key(key: Option<&str>) -> Self {
        Self::ALL
            .iter()
            .find(|(_, k, _)| Some(*k) == key)
            .map_or(ThemeChoice::System, |(t, _, _)| *t)
    }

    fn key(self) -> &'static str {
        Self::ALL.iter().find(|(t, _, _)| *t == self).unwrap().1
    }

    fn preference(self) -> ThemePreference {
        match self {
            ThemeChoice::System => ThemePreference::System,
            ThemeChoice::Light => ThemePreference::Light,
            ThemeChoice::Dark => ThemePreference::Dark,
        }
    }
}

/// First-run form: where is the simulator?
#[derive(Default)]
struct Setup {
    path_input: String,
    /// The simulator install `path_input` points to, if any.
    resolved: Option<Layout>,
    error: Option<String>,
    detected: Vec<PathBuf>,
    detecting: Option<Task<Vec<PathBuf>>>,
}

impl Setup {
    fn set_path(&mut self, path: String) {
        self.path_input = path;
        self.revalidate();
    }

    fn revalidate(&mut self) {
        self.error = None;
        let trimmed = self.path_input.trim();
        self.resolved = (!trimmed.is_empty())
            .then(|| Layout::find(Path::new(trimmed)))
            .flatten();
    }
}

const PASSWORD_FIELD: &str = "encoredecks_password";

/// Sign-in form of the Encore Decks account card (Settings), and the background work on the
/// account. The password lives only in this form until it is sent, then it is wiped.
#[derive(Default)]
struct AccountForm {
    email: String,
    password: Zeroizing<String>,
    /// Sign-in, then saving the session in the credential store.
    task: Option<Task<Result<Account, String>>>,
    /// Reading the saved session at startup.
    restoring: Option<Task<Result<Option<Account>, String>>>,
    /// Signing out and removing the saved session.
    unlinking: Option<Task<Result<(), String>>>,
    error: Option<String>,
}

pub struct App {
    settings: Settings,
    /// Encore Decks account decks are uploaded to; anonymous uploads without one.
    account: Option<Account>,
    account_form: AccountForm,
    screen: Screen,
    /// Screen to open once the simulator is loaded.
    home: Screen,
    theme: ThemeChoice,
    setup: Setup,
    series: Arc<Vec<Serie>>,
    /// The loaded simulator.
    layout: Option<Layout>,
    loading: Option<(Layout, Task<std::io::Result<Vec<Serie>>>)>,
    dialog: Option<(String, String)>,
    /// Last serie written to the settings (avoids a write per frame).
    saved_serie: Option<String>,
    snackbar: Option<Snackbar>,
    /// The window was asked to close while a deck had unsaved changes.
    confirm_quit: bool,
    /// The user confirmed quitting anyway.
    quitting: bool,
    card_list: CardList,
    deck_viewer: DeckViewer,
    /// The Encore screen: the linked account's decks on encoredecks.com.
    encore_page: EncorePage,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_settings(cc, Settings::open_default())
    }

    pub fn with_settings(cc: &eframe::CreationContext<'_>, settings: Settings) -> Self {
        let ctx = &cc.egui_ctx;
        egui_extras::install_image_loaders(ctx);
        theme::install(ctx);
        let theme = ThemeChoice::from_key(settings.get(THEME_KEY).as_deref());
        ctx.set_theme(theme.preference());

        let mut setup = Setup::default();
        setup.set_path(settings.get(SIMULATOR_PATH).unwrap_or_default());
        let home = match settings.get(SCREEN_KEY).as_deref() {
            Some("decks") => Screen::Decks,
            _ => Screen::Cards,
        };
        let restoring = settings.get(ENCORE_USER).map(|name| {
            Task::spawn(ctx, move || {
                credentials::load_session(&name)
                    .map(|session| session.map(|session| Account { name, session }))
                    .map_err(|e| format!("{e:#}"))
            })
        });
        let mut app = Self {
            account: None,
            account_form: AccountForm {
                restoring,
                ..AccountForm::default()
            },
            settings,
            screen: Screen::Setup,
            home,
            theme,
            setup,
            series: Arc::default(),
            layout: None,
            loading: None,
            dialog: None,
            saved_serie: None,
            snackbar: None,
            confirm_quit: false,
            quitting: false,
            card_list: CardList::default(),
            deck_viewer: DeckViewer::default(),
            encore_page: EncorePage::default(),
        };
        if app.setup.resolved.is_some() {
            app.load_simulator(ctx, false);
        } else {
            app.setup.detecting = Some(Task::spawn(ctx, detect_simulators));
        }
        app
    }

    fn save_setting(&mut self, key: &str, value: &str) {
        if self.settings.get(key).as_deref() != Some(value)
            && let Err(e) = self.settings.set(key, value)
        {
            self.notify(Notice::Error(
                "Could not save settings".into(),
                format!("{}: {e}", self.settings.path().display()),
            ));
        }
    }

    /// Signs in off the UI thread and saves the session in the credential store. The password
    /// is moved out of the form and wiped, along with the text field's copy of it.
    fn link_account(&mut self, ctx: &egui::Context) {
        let form = &mut self.account_form;
        let email = form.email.trim().to_string();
        let password = std::mem::take(&mut form.password);
        egui::text_edit::TextEditState::default().store(ctx, egui::Id::new(PASSWORD_FIELD));
        form.error = None;
        form.task = Some(Task::spawn(ctx, move || {
            let account = encore::login(&email, &password).map_err(|e| format!("{e:#}"))?;
            drop(password);
            if let Err(e) = credentials::save_session(&account.name, &account.session) {
                // Not kept anywhere: end the session rather than leave it open.
                encore::logout(&account.session);
                return Err(format!("{e:#}"));
            }
            Ok(account)
        }));
    }

    /// Finishes the background account work: sign-in, startup restore, unlink.
    fn poll_account(&mut self) {
        let form = &mut self.account_form;
        if let Some(result) = form.restoring.as_ref().and_then(Task::poll) {
            form.restoring = None;
            match result {
                Some(Ok(Some(account))) => self.account = Some(account),
                // Nothing saved for this account any more: forget its name too.
                Some(Ok(None)) => {
                    if let Err(e) = self.settings.delete(ENCORE_USER) {
                        form.error = Some(format!("Could not save settings: {e}"));
                    }
                }
                Some(Err(e)) => {
                    form.error = Some(format!(
                        "Could not read the Encore Decks session: {e}. Unlock the system \
                         keyring and restart, or link the account again."
                    ))
                }
                None => form.error = Some("Reading the Encore Decks session failed.".into()),
            }
        }
        let form = &mut self.account_form;
        if let Some(result) = form.unlinking.as_ref().and_then(Task::poll) {
            form.unlinking = None;
            match result {
                Some(Ok(())) => {}
                Some(Err(e)) => form.error = Some(format!("Could not unlink completely: {e}")),
                None => form.error = Some("Unlinking stopped unexpectedly.".into()),
            }
        }
        let form = &mut self.account_form;
        let Some(result) = form.task.as_ref().and_then(Task::poll) else {
            return;
        };
        form.task = None;
        match result {
            Some(Ok(account)) => {
                form.email.zeroize();
                self.save_setting(ENCORE_USER, &account.name);
                self.notify(Notice::Info(format!(
                    "Linked the Encore Decks account \"{}\".",
                    account.name
                )));
                self.account = Some(account);
            }
            Some(Err(e)) => form.error = Some(format!("Could not link the account: {e}")),
            None => form.error = Some("The sign-in stopped unexpectedly.".into()),
        }
    }

    /// Forgets the linked account: signs out of encoredecks.com and removes the session from
    /// the credential store, in the background.
    fn unlink_account(&mut self, ctx: &egui::Context) {
        if let Some(account) = self.account.take() {
            self.account_form.unlinking = Some(Task::spawn(ctx, move || {
                encore::logout(&account.session);
                credentials::delete_session(&account.name).map_err(|e| format!("{e:#}"))
            }));
        }
        if let Err(e) = self.settings.delete(ENCORE_USER) {
            self.notify(Notice::Error(
                "Could not save settings".into(),
                e.to_string(),
            ));
        }
    }

    fn account_card(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ui.label(theme::title_medium("Encore Decks account").color(s.on_surface));
        if let Some(account) = &self.account {
            ui.horizontal(|ui| {
                ui.label(ICON_ACCOUNT_CIRCLE.rich_text().size(20.0).color(s.primary));
                ui.label(RichText::new(format!("Linked as {}", account.name)).color(s.on_surface));
            });
            ui.label(
                theme::body_small("Uploaded decks are saved in this account.")
                    .color(s.on_surface_variant),
            );
            ui.add_space(8.0);
            if theme::outlined(Some(ICON_LOGOUT), "Unlink")
                .show(ui)
                .on_hover_text("Sign out and upload anonymously again")
                .clicked()
            {
                self.unlink_account(ui.ctx());
            }
            return;
        }
        if self.account_form.restoring.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new("Reading the session from the system keyring…")
                        .color(s.on_surface_variant),
                );
            });
            return;
        }
        ui.label(
            theme::body_small(
                "Sign in to upload decks to your account, where you can edit or delete them. \
                 Without an account, decks are uploaded anonymously. Your password is never \
                 stored: it is sent once over HTTPS, then wiped. The session is kept \
                 encrypted in the system keyring (Secret Service / KWallet, Windows \
                 Credential Manager), never in the settings file.",
            )
            .color(s.on_surface_variant),
        );
        ui.add_space(8.0);
        let form = &mut self.account_form;
        let running = form.task.is_some();
        let mut submit = false;
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled(
                !running,
                egui::TextEdit::singleline(&mut form.email)
                    .hint_text("Email")
                    .desired_width(240.0),
            );
            let password = ui.add_enabled(
                !running,
                egui::TextEdit::singleline(&mut *form.password)
                    .id(egui::Id::new(PASSWORD_FIELD))
                    .hint_text("Password")
                    .password(true)
                    .desired_width(200.0),
            );
            let ready = !running && !form.email.trim().is_empty() && !form.password.is_empty();
            submit =
                ready && password.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if theme::tonal(Some(ICON_LOGIN), "Link account")
                .enabled(ready)
                .show(ui)
                .clicked()
            {
                submit = true;
            }
            if running {
                ui.spinner();
            }
        });
        if let Some(error) = &form.error {
            ui.label(RichText::new(error).color(s.error).small());
        }
        if submit {
            self.link_account(ui.ctx());
        }
    }

    fn notify(&mut self, notice: Notice) {
        match notice {
            Notice::Info(text) => self.snackbar = Some(Snackbar::new(text)),
            Notice::Error(title, body) => self.dialog = Some((title, body)),
        }
    }

    fn load_simulator(&mut self, ctx: &egui::Context, save: bool) {
        let Some(layout) = self.setup.resolved.clone() else {
            self.setup.error = Some(
                "This is not the simulator folder. Pick the folder that contains \
                 CardContent and Decks."
                    .into(),
            );
            return;
        };
        if save {
            self.save_setting(SIMULATOR_PATH, &layout.root.to_string_lossy());
        }
        let job = layout.clone();
        self.loading = Some((layout, Task::spawn(ctx, move || load_simulator(&job))));
    }

    fn unload_simulator(&mut self, ctx: &egui::Context) {
        if let Err(e) = self.settings.delete(SIMULATOR_PATH) {
            self.notify(Notice::Error(
                "Could not save settings".into(),
                e.to_string(),
            ));
        }
        self.series = Arc::default();
        card_view::set_names(ctx, None);
        self.layout = None;
        self.card_list = CardList::default();
        self.deck_viewer = DeckViewer::default();
        self.encore_page = EncorePage::default();
        self.screen = Screen::Setup;
        self.setup.revalidate();
        if self.setup.detected.is_empty() && self.setup.detecting.is_none() {
            self.setup.detecting = Some(Task::spawn(ctx, detect_simulators));
        }
        ctx.forget_all_images();
    }

    fn switch_to(&mut self, ctx: &egui::Context, screen: Screen) {
        if screen == self.screen {
            return;
        }
        ctx.forget_all_images();
        if screen == Screen::Decks
            && let Some(layout) = &self.layout
            && let Err(e) = self.deck_viewer.reload(&layout.decks())
        {
            self.notify(Notice::Error(
                "Could not read the simulator decks".into(),
                format!("{e:#}"),
            ));
        }
        if matches!(screen, Screen::Cards | Screen::Decks) {
            self.save_setting(SCREEN_KEY, screen.key());
        }
        self.screen = screen;
    }

    fn poll_tasks(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.setup.detecting.as_ref().and_then(Task::poll) {
            self.setup.detecting = None;
            self.setup.detected = result.unwrap_or_default();
        }

        let Some((layout, task)) = &self.loading else {
            return;
        };
        let Some(result) = task.poll() else { return };
        let layout = layout.clone();
        self.loading = None;
        match result {
            Some(Ok(series)) => {
                let reloaded = self.layout.as_ref() == Some(&layout);
                self.series = Arc::new(series);
                card_view::set_names(ctx, Some(Arc::new(NameIndex::new(&self.series))));
                self.layout = Some(layout);
                let serie = self.settings.get(SERIE_KEY);
                self.saved_serie = serie.clone();
                self.card_list = CardList::default();
                self.card_list.open_serie(&self.series, serie.as_deref());
                if reloaded {
                    let sets: usize = self.series.iter().map(|s| s.sets.len()).sum();
                    self.notify(Notice::Info(format!(
                        "Reloaded {} series ({sets} sets).",
                        self.series.len()
                    )));
                } else {
                    let home = self.home;
                    self.screen = Screen::Setup;
                    self.switch_to(ctx, home);
                }
            }
            Some(Err(e)) => {
                self.setup.error = Some(format!("Could not load {}: {e}", layout.root.display()));
                self.screen = Screen::Setup;
            }
            None => {
                self.setup.error = Some("Loading the simulator failed unexpectedly.".into());
                self.screen = Screen::Setup;
            }
        }
    }

    fn setup_screen(&mut self, ui: &mut Ui) {
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
                        if let Some((layout, _)) = &self.loading {
                            ui.vertical_centered(|ui| {
                                ui.add(egui::Spinner::new().size(36.0).color(s.primary));
                                ui.add_space(8.0);
                                ui.label(theme::title_medium("Loading cards…").color(s.on_surface));
                                ui.label(
                                    RichText::new(layout.root.display().to_string())
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

        let status_color = match (&self.setup.resolved, &self.setup.error) {
            (_, Some(_)) => s.error,
            (Some(_), None) => s.success,
            (None, None) if self.setup.path_input.trim().is_empty() => s.outline,
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
            (Some(layout), None) => (
                Some(ICON_CHECK_CIRCLE),
                if Path::new(self.setup.path_input.trim()) == layout.root {
                    "Simulator found.".to_string()
                } else {
                    format!("Simulator found in {}", layout.root.display())
                },
            ),
            (None, None) if self.setup.path_input.trim().is_empty() => (
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
            if let Some(i) = icon {
                ui.label(i.rich_text().size(16.0).color(color));
            }
            ui.add(egui::Label::new(theme::body_small(message).color(color)).wrap());
        });

        let detected: Vec<PathBuf> = self
            .setup
            .detected
            .iter()
            .filter(|p| self.setup.resolved.as_ref().is_none_or(|l| &l.root != *p))
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
        actions_row(ui, |ui| {
            let load = theme::filled(Some(ICON_FOLDER_OPEN), "Load simulator")
                .enabled(self.setup.resolved.is_some())
                .show(ui);
            if load.clicked() || enter {
                self.load_simulator(ui.ctx(), true);
            }
        });
    }

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
                if theme::rail_item(
                    ui,
                    ICON_SETTINGS,
                    "Settings",
                    self.screen == Screen::Settings,
                )
                .clicked()
                {
                    target = Some(Screen::Settings);
                }
                let dark = ui.visuals().dark_mode;
                let (icon, tip) = if dark {
                    (ICON_LIGHT_MODE, "Switch to light theme")
                } else {
                    (ICON_DARK_MODE, "Switch to dark theme")
                };
                if theme::icon_button(icon, tip).show(ui).clicked() {
                    self.set_theme(
                        ui.ctx(),
                        if dark {
                            ThemeChoice::Light
                        } else {
                            ThemeChoice::Dark
                        },
                    );
                }
            });
        });
        if let Some(screen) = target {
            self.switch_to(ui.ctx(), screen);
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, choice: ThemeChoice) {
        self.theme = choice;
        ctx.set_theme(choice.preference());
        self.save_setting(THEME_KEY, choice.key());
    }

    fn settings_screen(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let width = ui.available_width().min(760.0);
            ui.set_max_width(width);
            ui.label(theme::headline("Settings").color(s.on_surface));
            ui.add_space(16.0);

            theme::surface_card(s.surface_container_low).show(ui, |ui| {
                ui.set_width(ui.available_width());
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
                        let selected = self.theme == choice;
                        let b = if selected {
                            theme::tonal(Some(icon), label)
                        } else {
                            theme::outlined(Some(icon), label)
                        };
                        if b.min_width(120.0).show(ui).clicked() {
                            self.set_theme(ui.ctx(), choice);
                        }
                    }
                });
            });
            ui.add_space(12.0);

            theme::surface_card(s.surface_container_low).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(theme::title_medium("Simulator").color(s.on_surface));
                let path = self
                    .layout
                    .as_ref()
                    .map(|l| l.root.display().to_string())
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
            });
            ui.add_space(12.0);

            theme::surface_card(s.surface_container_low).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(theme::title_medium("Simulator decks").color(s.on_surface));
                let decks = self.layout.as_ref().map(Layout::decks).unwrap_or_default();
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
            });
            ui.add_space(12.0);

            theme::surface_card(s.surface_container_low).show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.account_card(ui);
            });
            ui.add_space(12.0);

            theme::surface_card(s.surface_container_low).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(theme::title_medium("About").color(s.on_surface));
                ui.label(
                    RichText::new(format!(
                        "SimuDeckParser {} — settings in {}",
                        env!("CARGO_PKG_VERSION"),
                        self.settings.path().display()
                    ))
                    .color(s.on_surface_variant),
                );
            });
        });
    }

    /// Asks before closing the window over a deck with unsaved changes.
    fn quit_guard(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quitting
            && self.deck_viewer.has_unsaved_deck()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_quit = true;
        }
        if !self.confirm_quit {
            return;
        }
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("confirm_quit"))
            .frame(theme::dialog_frame(ctx))
            .show(ctx, |ui| {
                let s = scheme(ui);
                ui.set_width(400.0);
                ui.label(theme::headline("Quit without saving?").color(s.on_surface));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("The deck you are building has changes that are not saved.")
                        .color(s.on_surface_variant),
                );
                ui.add_space(20.0);
                actions_row(ui, |ui| {
                    if theme::filled(None, "Quit").show(ui).clicked() {
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

    fn error_dialog(&mut self, ctx: &egui::Context) {
        let Some((title, body)) = &self.dialog else {
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
                ui.add(egui::Label::new(RichText::new(body).color(s.on_surface_variant)).wrap());
                ui.add_space(20.0);
                actions_row(ui, |ui| {
                    close = theme::text(None, "OK").show(ui).clicked();
                });
            });
        if close || modal.should_close() {
            self.dialog = None;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_tasks(&ctx);
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

            match self.screen {
                Screen::Cards => {
                    self.card_list.show(ui, &self.series, "Cards", None);
                    let serie = self.card_list.serie_name(&self.series).map(str::to_owned);
                    if serie.is_some() && serie != self.saved_serie {
                        self.save_setting(SERIE_KEY, serie.as_deref().unwrap_or_default());
                        self.saved_serie = serie;
                    }
                }
                Screen::Decks => {
                    let decks = self.layout.as_ref().map(Layout::decks).unwrap_or_default();
                    self.deck_viewer.show(
                        ui,
                        &self.series,
                        &decks,
                        self.account.as_ref(),
                        self.account_form.restoring.is_some(),
                    );
                }
                Screen::Encore => {
                    let decks = self.layout.as_ref().map(Layout::decks).unwrap_or_default();
                    self.encore_page.show(
                        ui,
                        &self.series,
                        &decks,
                        self.account.as_ref(),
                        self.account_form.restoring.is_some(),
                    );
                }
                Screen::Settings => {
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::new()
                                .fill(s.surface)
                                .inner_margin(Margin::same(24)),
                        )
                        .show(ui, |ui| self.settings_screen(ui));
                }
                Screen::Setup => unreachable!(),
            }
        }
        if let Some(notice) = self.deck_viewer.take_notice() {
            self.notify(notice);
        }
        if self.deck_viewer.take_session_expired() {
            self.unlink_account(&ctx);
        }
        if let Some(notice) = self.encore_page.take_notice() {
            self.notify(notice);
        }
        if self.encore_page.take_session_expired() {
            self.unlink_account(&ctx);
        }
        self.poll_account();
        if let Some(bar) = &mut self.snackbar
            && !bar.show(&ctx)
        {
            self.snackbar = None;
        }
        if self.screen != Screen::Setup {
            card_view::related_window(&ctx);
        }
        self.error_dialog(&ctx);
        self.quit_guard(&ctx);
    }
}

/// Right-aligned row of dialog/form buttons (first added is rightmost).
pub fn actions_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 40.0),
        egui::Layout::right_to_left(egui::Align::Center),
        add,
    );
}

/// Looks for simulator installs in the usual places: folders under the home folder whose
/// names suggest games, and Steam libraries.
fn detect_simulators() -> Vec<PathBuf> {
    const HINTS: [&str; 8] = [
        "weiss", "schwarz", "simu", "game", "jeu", "steam", "ws", "share",
    ];
    fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with('.') && name != ".local" {
                continue;
            }
            if Layout::at(&path).is_some() {
                found.push(path);
            } else if depth < 3 && (depth == 0 || HINTS.iter().any(|k| name.contains(k))) {
                walk(&path, depth + 1, found);
            }
        }
    }
    let mut found = Vec::new();
    if let Some(home) = dirs::home_dir() {
        walk(&home, 0, &mut found);
        for steam in [".local/share/Steam", ".steam/steam"] {
            walk(&home.join(steam).join("steamapps/common"), 2, &mut found);
        }
    }
    if cfg!(windows) {
        for steam in [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"] {
            walk(
                &Path::new(steam).join("steamapps").join("common"),
                2,
                &mut found,
            );
        }
    }
    for path in &mut found {
        *path = path.canonicalize().unwrap_or_else(|_| path.clone());
    }
    found.sort();
    found.dedup();
    found.truncate(6);
    found
}
