//! The Encore Decks account: the sign-in form of the Settings screen, and the background work
//! that links, restores and unlinks the account.
//!
//! The password only lives in the form until it is sent; it is then wiped from memory, along
//! with the text field's own copy. The session that sign-in returns is kept in the system
//! credential store (see `simu_deck_parser::encoredecks::credentials`).

use egui::{RichText, Ui};
use egui_material_icons::icons::{ICON_ACCOUNT_CIRCLE, ICON_LOGIN, ICON_LOGOUT};
use simu_deck_parser::encoredecks::account::{self, Account};
use simu_deck_parser::encoredecks::credentials;
use simu_deck_parser::settings;
use zeroize::{Zeroize, Zeroizing};

use super::background::{Task, finished};
use super::theme::{self, scheme};
use super::{App, Notice};

/// egui id of the password field, to wipe the text it keeps.
const PASSWORD_FIELD: &str = "encoredecks_password";

/// The sign-in form, and the background work on the account.
#[derive(Default)]
pub(super) struct AccountForm {
    email: String,
    password: Zeroizing<String>,
    /// Signing in, then saving the session in the credential store.
    signing_in: Option<Task<Account>>,
    /// Reading the saved session at startup.
    restoring: Option<Task<Option<Account>>>,
    /// Signing out and removing the saved session.
    unlinking: Option<Task<()>>,
    error: Option<String>,
}

impl AccountForm {
    /// The form at startup: reads the session of `account_name` (the account linked last time,
    /// from the settings) in the background.
    pub(super) fn restoring(ctx: &egui::Context, account_name: Option<String>) -> Self {
        let restoring = account_name.map(|name| {
            Task::spawn(ctx, move || {
                let session = credentials::load_session(&name)?;
                Ok(session.map(|session| Account { name, session }))
            })
        });
        Self {
            restoring,
            ..Self::default()
        }
    }

    /// The saved session is still being read: an upload now would wrongly be anonymous.
    pub(super) fn is_restoring(&self) -> bool {
        self.restoring.is_some()
    }
}

impl App {
    /// Signs in off the UI thread and saves the session in the credential store. The password
    /// is moved out of the form and wiped, along with the text field's copy of it.
    fn link_account(&mut self, ctx: &egui::Context) {
        let form = &mut self.account_form;
        let email = form.email.trim().to_string();
        let password = std::mem::take(&mut form.password);
        egui::text_edit::TextEditState::default().store(ctx, egui::Id::new(PASSWORD_FIELD));
        form.error = None;
        form.signing_in = Some(Task::spawn(ctx, move || {
            let account = account::login(&email, &password)?;
            drop(password);
            if let Err(e) = credentials::save_session(&account.name, &account.session) {
                // Not kept anywhere: end the session rather than leave it open.
                account::logout(&account.session);
                return Err(e);
            }
            Ok(account)
        }));
    }

    /// Forgets the linked account: signs out of encoredecks.com and removes the session from
    /// the credential store, in the background.
    pub(super) fn unlink_account(&mut self, ctx: &egui::Context) {
        if let Some(account) = self.account.take() {
            self.account_form.unlinking = Some(Task::spawn(ctx, move || {
                account::logout(&account.session);
                credentials::delete_session(&account.name)
            }));
        }
        self.delete_setting(ctx, settings::ENCORE_USER);
    }

    /// Finishes the background work on the account: startup restore, sign-in, unlink.
    pub(super) fn poll_account(&mut self, ctx: &egui::Context) {
        let form = &mut self.account_form;
        match finished(&mut form.restoring) {
            Some(Ok(Some(account))) => self.account = Some(account),
            // Nothing saved for this account any more: forget its name too.
            Some(Ok(None)) => {
                if let Err(e) = self.settings.delete(settings::ENCORE_USER) {
                    form.error = Some(format!("Could not save settings: {e}"));
                }
            }
            Some(Err(e)) => {
                form.error = Some(format!(
                    "Could not read the Encore Decks session: {e:#}. Unlock the system \
                     keyring and restart, or link the account again."
                ))
            }
            None => {}
        }

        if let Some(Err(e)) = finished(&mut form.unlinking) {
            form.error = Some(format!("Could not unlink completely: {e:#}"));
        }

        match finished(&mut form.signing_in) {
            Some(Ok(account)) => {
                form.email.zeroize();
                self.save_setting(ctx, settings::ENCORE_USER, &account.name);
                let message = format!("Linked the Encore Decks account \"{}\".", account.name);
                self.notify(ctx, Notice::Info(message));
                self.account = Some(account);
            }
            Some(Err(e)) => form.error = Some(format!("Could not link the account: {e:#}")),
            None => {}
        }
    }

    /// The "Encore Decks account" card of the Settings screen.
    pub(super) fn account_card(&mut self, ui: &mut Ui) {
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
        if self.account_form.is_restoring() {
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
        let running = form.signing_in.is_some();
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
            let pressed_enter =
                password.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            submit = ready && pressed_enter;
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
}
