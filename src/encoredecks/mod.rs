//! encoredecks.com, the deck-sharing site: importing its decks into the simulator, publishing
//! simulator decks on it, and signing in to an account.
//!
//! - [`decks`]: reading decks (one by its link, or a page of the linked account's decks), and
//!   importing them into the simulator's `Decks` folder.
//! - [`upload`]: publishing a simulator deck.
//! - [`account`]: signing in, and checking that a saved session is still valid.
//! - [`credentials`]: keeping the session in the operating system's credential store.
//!
//! Every function that talks to the site blocks until it answers: the app runs them on a
//! background thread (see `app/background.rs`).
//!
//! Security: the password is only ever sent to the login endpoint, over HTTPS, and is never
//! stored. Requests carrying the session cookie never follow redirects, so the cookie can
//! only reach encoredecks.com.

pub mod account;
pub mod credentials;
pub mod decks;
pub mod upload;

use anyhow::{Context, Result};
use serde_json::Value;

const SITE: &str = "https://www.encoredecks.com";
/// `{DECK_API}{id}`: one deck, as JSON.
const DECK_API: &str = "https://www.encoredecks.com/api/deck/";
/// `?cardcode=…`: one card, as JSON.
const CARD_API: &str = "https://www.encoredecks.com/api/card";
/// POST a deck here to publish it.
const SAVE_API: &str = "https://www.encoredecks.com/api/deck";
/// `{DECK_PAGE}{id}`: a deck's page, the link users share.
const DECK_PAGE: &str = "https://www.encoredecks.com/deck/";
/// Deck search: serves 24 decks per page, with `page`, `username` and `invalid` parameters.
const SEARCH_API: &str = "https://www.encoredecks.com/api/search/deck";
const LOGIN_API: &str = "https://www.encoredecks.com/api/login";
const LOGOUT_API: &str = "https://www.encoredecks.com/api/logout";
/// Redirects to `/user/<name>` when logged in, to `/login` otherwise.
const USER_PAGE: &str = "https://www.encoredecks.com/user";

/// Reads the body of a response as JSON.
fn read_json(mut response: ureq::http::Response<ureq::Body>) -> Result<Value> {
    let body = response
        .body_mut()
        .read_to_string()
        .context("reading the encoredecks response")?;
    serde_json::from_str(&body).context("encoredecks returned invalid JSON")
}
