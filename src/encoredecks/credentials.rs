//! The Encore Decks session, kept in the operating system's credential store: the Secret
//! Service (GNOME Keyring, KWallet) on Linux, the Credential Manager on Windows, the Keychain
//! on macOS. They encrypt it at rest and tie it to the user's login. It is never written to
//! the settings file, and there is no plaintext fallback: without a credential store, the
//! account cannot be linked.
//!
//! Every call may block (the store can ask to be unlocked): run them off the UI thread.

use anyhow::{Context, Result};
use keyring::{Entry, Error};
use zeroize::Zeroizing;

const SERVICE: &str = "simu_deck_parser.encoredecks";

fn entry(account: &str) -> Result<Entry> {
    Entry::new(SERVICE, account).context(
        "no secure credential store is available (Secret Service / KWallet on Linux, \
         Credential Manager on Windows)",
    )
}

pub fn save_session(account: &str, session: &str) -> Result<()> {
    entry(account)?
        .set_password(session)
        .context("could not save the session in the system credential store")
}

/// `None` when the store has no session for this account.
pub fn load_session(account: &str) -> Result<Option<Zeroizing<String>>> {
    match entry(account)?.get_password() {
        Ok(session) => Ok(Some(Zeroizing::new(session))),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(e).context("could not read the session from the system credential store"),
    }
}

/// Removing a session that is not there is not an error.
pub fn delete_session(account: &str) -> Result<()> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("could not remove the session from the system credential store"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uses the real credential store: `cargo test -- --ignored live_credential_store`.
    #[test]
    #[ignore]
    fn live_credential_store() {
        let account = "simu_deck_parser test account";
        save_session(account, "s%3Atest").unwrap();
        assert_eq!(
            load_session(account)
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("s%3Atest")
        );
        delete_session(account).unwrap();
        assert!(load_session(account).unwrap().is_none());
        delete_session(account).unwrap();
    }
}
