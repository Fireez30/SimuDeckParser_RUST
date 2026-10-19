//! The Upload dialog: publishing the open deck on Encore Decks. It asks for confirmation, shows
//! progress, then the new deck's link.

use egui::RichText;
use egui_material_icons::icons::{ICON_CONTENT_COPY, ICON_OPEN_IN_NEW, ICON_UPLOAD, ICON_WARNING};
use simu_deck_parser::encoredecks::account::Account;
use simu_deck_parser::encoredecks::upload::{check_deck_size, upload};

use super::DecksScreen;
use crate::app::Notice;
use crate::app::background::{Task, finished};
use crate::app::theme::{self, scheme};

pub(super) struct UploadDialog {
    name: String,
    /// Every code of the deck file, unmatched ones included.
    codes: Vec<String>,
    /// Account the deck was sent to (set when sending); anonymous when `None`.
    account: Option<Account>,
    /// The upload, once started; it ends with the deck's link.
    task: Option<Task<String>>,
    /// Link of the published deck.
    done: Option<String>,
}

impl DecksScreen {
    /// Asks to publish the open deck.
    pub(super) fn open_upload(&mut self) {
        if let Some(picked) = &self.picked {
            self.upload = Some(UploadDialog {
                name: picked.deck.name.clone(),
                codes: picked.file.codes.clone(),
                account: None,
                task: None,
                done: None,
            });
        }
    }

    /// Finishes a background upload: shows the link, or closes the dialog on error.
    pub(super) fn poll_upload(&mut self) {
        let Some(dialog) = &mut self.upload else {
            return;
        };
        match finished(&mut dialog.task) {
            Some(Ok(url)) => dialog.done = Some(url),
            Some(Err(e)) => {
                self.upload = None;
                // An expired session is noted in the notice: the app then unlinks the account.
                self.notice = Some(Notice::from_error("Upload failed", &e));
            }
            None => {}
        }
    }

    /// `account` is the one linked right now: the deck goes to it when Upload is clicked.
    pub(super) fn upload_dialog(
        &mut self,
        ctx: &egui::Context,
        account: Option<&Account>,
        account_pending: bool,
    ) {
        let Some(dialog) = &mut self.upload else {
            return;
        };
        let running = dialog.task.is_some();
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("upload"))
            .frame(theme::dialog_frame(ctx))
            .show(ctx, |ui| {
                let s = scheme(ui);
                ui.set_width(520.0);
                ui.vertical_centered(|ui| {
                    ui.label(ICON_UPLOAD.rich_text().size(28.0).color(s.primary));
                    let title = if dialog.done.is_some() {
                        "Uploaded to Encore Decks"
                    } else {
                        "Upload to Encore Decks"
                    };
                    ui.label(theme::headline(title).color(s.on_surface));
                });
                ui.add_space(8.0);

                // Done: the link.
                if let Some(done) = &dialog.done {
                    let where_to = match &dialog.account {
                        Some(account) => format!(" in the account {}", account.name),
                        None => String::new(),
                    };
                    ui.label(
                        RichText::new(format!("\"{}\" is online{where_to}:", dialog.name))
                            .color(s.on_surface_variant),
                    );
                    ui.hyperlink(done);
                    ui.add_space(16.0);
                    theme::actions_row(ui, |ui| {
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

                // Not sent yet: what will happen, and the Upload button.
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
                        dialog.name,
                        dialog.codes.len()
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
                let size_error = check_deck_size(&dialog.codes).err();
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
                theme::actions_row(ui, |ui| {
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
                        let name = dialog.name.clone();
                        let codes = dialog.codes.clone();
                        dialog.account = account.cloned();
                        let session = account.map(|a| a.session.clone());
                        dialog.task = Some(Task::spawn(ui.ctx(), move || {
                            upload(&name, &codes, session.as_deref().map(String::as_str))
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
