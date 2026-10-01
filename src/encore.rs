//! Deck import from encoredecks.com into the simulator `Decks` folder, and deck upload.

use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::builder::DECK_SIZE;
use crate::codes::{CardIndex, encore_spellings, transform_to_existing_keys};
use crate::decks;
use crate::model::{Deck, Serie};

const API: &str = "https://www.encoredecks.com/api/deck/";
const CARD_API: &str = "https://www.encoredecks.com/api/card";
const SAVE_API: &str = "https://www.encoredecks.com/api/deck";
const DECK_PAGE: &str = "https://www.encoredecks.com/deck/";
const LOGIN_API: &str = "https://www.encoredecks.com/api/login";
const LOGOUT_API: &str = "https://www.encoredecks.com/api/logout";
/// Redirects to `/user/<name>` when logged in, to `/login` otherwise.
const USER_PAGE: &str = "https://www.encoredecks.com/user";
const SESSION_COOKIE: &str = "encoresesssionid";

/// An Encore Decks account linked to the tool: its name and login session (a cookie valid
/// for a year). The password is never kept. The session is wiped from memory when dropped,
/// never printed, and stored only in the system credential store ([`crate::credentials`]).
#[derive(Clone, PartialEq)]
pub struct Account {
    pub name: String,
    pub session: Zeroizing<String>,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("name", &self.name)
            .field("session", &"<redacted>")
            .finish()
    }
}

/// The linked account's session is no longer valid: the user must link it again.
#[derive(Debug)]
pub struct SessionExpired;

impl std::fmt::Display for SessionExpired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the Encore Decks session has expired; link your account again in Settings"
        )
    }
}

impl std::error::Error for SessionExpired {}

#[derive(Debug, PartialEq)]
pub struct EncoreDeck {
    pub name: String,
    /// Already formatted the way the simulator stores it: `HH:MM  MM/DD/YYYY`.
    pub date: String,
    pub codes: Vec<String>,
}

/// `https://www.encoredecks.com/deck/PPyvcLuvt` -> `PPyvcLuvt`.
pub fn deck_id_from_link(link: &str) -> Option<&str> {
    let link = link.trim();
    let link = link
        .split(['?', '#'])
        .next()
        .unwrap_or(link)
        .trim_end_matches('/');
    let parts: Vec<&str> = link.split('/').collect();
    (parts.len() > 4)
        .then(|| parts[parts.len() - 1])
        .filter(|id| !id.is_empty())
}

/// `2025-09-30T22:20:36.123Z` -> `22:20  09/30/2025`.
pub fn simulator_date(iso: &str) -> Option<String> {
    let trimmed = iso.trim().get(..19)?;
    chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|dt| dt.format("%H:%M  %m/%d/%Y").to_string())
}

pub fn parse_deck_json(json: &Value) -> Result<EncoreDeck> {
    let name = json
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .ok_or_else(|| anyhow!("the deck has no name"))?
        .to_string();
    let date = ["datecreated", "datemodified"]
        .iter()
        .find_map(|k| json.get(*k).and_then(Value::as_str))
        .and_then(simulator_date)
        .unwrap_or_default();
    let codes: Vec<String> = json
        .get("cards")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("the deck has no card list"))?
        .iter()
        .filter_map(|c| c.get("cardcode").and_then(Value::as_str))
        .map(|c| c.trim().to_string())
        .collect();
    if codes.is_empty() {
        bail!("the deck is empty");
    }
    Ok(EncoreDeck { name, date, codes })
}

pub fn fetch_deck(id: &str) -> Result<EncoreDeck> {
    let url = format!("{API}{}", id.trim());
    let body = ureq::get(&url)
        .call()
        .with_context(|| format!("requesting {url}"))?
        .body_mut()
        .read_to_string()
        .context("reading the encoredecks response")?;
    let json: Value = serde_json::from_str(&body).context("encoredecks returned invalid JSON")?;
    parse_deck_json(&json)
}

/// Resolves card codes against the loaded simulator cards and builds a displayable deck.
/// Returns the deck and the codes that matched no card.
pub fn build_deck(
    index: &CardIndex,
    name: &str,
    date: &str,
    codes: &[String],
) -> (Deck, Vec<String>) {
    let resolved = transform_to_existing_keys(&index.codes(), codes);
    let mut missing = Vec::new();
    let cards = resolved
        .iter()
        .filter(|c| !c.trim().is_empty())
        .filter_map(|code| {
            let card = index.get(code).cloned();
            if card.is_none() {
                missing.push(code.clone());
            }
            card
        })
        .collect();
    let deck = Deck {
        name: name.to_string(),
        date: date.to_string(),
        cards,
    };
    (deck, missing)
}

/// Downloads a deck and saves it as a new file in `decks_dir`. Blocking: run it off the UI
/// thread. Returns the deck (its name gets a suffix when taken) and the unmatched codes.
pub fn import(link: &str, series: &[Serie], decks_dir: &Path) -> Result<(Deck, Vec<String>)> {
    let id =
        deck_id_from_link(link).ok_or_else(|| anyhow!("not an encoredecks deck link: {link:?}"))?;
    let fetched = fetch_deck(id)?;

    let index = CardIndex::new(series);
    let codes = transform_to_existing_keys(&index.codes(), &fetched.codes);
    let saved = decks::save_new_deck(decks_dir, &fetched.name, &fetched.date, &codes)?;

    Ok(build_deck(&index, &saved.name, &saved.date, &codes))
}

/// Encore Decks' id of a card, or `None` when it has no card with this exact code.
fn fetch_card_id(code: &str) -> Result<Option<String>> {
    let body = ureq::get(CARD_API)
        .query("cardcode", code)
        .call()
        .with_context(|| format!("looking up {code} on encoredecks"))?
        .body_mut()
        .read_to_string()
        .context("reading the encoredecks response")?;
    let json: Value = serde_json::from_str(&body).context("encoredecks returned invalid JSON")?;
    Ok(json.get("_id").and_then(Value::as_str).map(str::to_string))
}

/// Maps deck codes to Encore Decks card ids (one per copy, in deck order), trying the other
/// spellings of each code (JP/EN, foil or not). `lookup` returns the id of an exact code.
pub fn resolve_card_ids(
    codes: &[String],
    mut lookup: impl FnMut(&str) -> Result<Option<String>>,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut known: std::collections::HashMap<&str, Option<String>> = Default::default();
    let mut ids = Vec::new();
    let mut missing = Vec::new();
    for code in codes.iter().map(|c| c.trim()).filter(|c| !c.is_empty()) {
        if !known.contains_key(code) {
            let mut found = None;
            for spelling in encore_spellings(code) {
                if let Some(id) = lookup(&spelling)? {
                    found = Some(id);
                    break;
                }
            }
            if found.is_none() {
                missing.push(code.to_string());
            }
            known.insert(code, found);
        }
        if let Some(id) = &known[code] {
            ids.push(id.clone());
        }
    }
    Ok((ids, missing))
}

/// Checked before anything is looked up: only full decks are published.
pub fn check_deck_size(codes: &[String]) -> Result<()> {
    let count = codes.iter().filter(|c| !c.trim().is_empty()).count();
    if count != DECK_SIZE {
        bail!("the deck has {count} cards; only {DECK_SIZE}-card decks can be uploaded");
    }
    Ok(())
}

/// Checked before sending: an uploaded deck is public and searchable, so it must be the
/// exact deck, never one with cards left out.
pub fn check_all_found(ids: &[String], missing: &[String]) -> Result<()> {
    if !missing.is_empty() {
        bail!(
            "{} code(s) are not on Encore Decks: {}. Nothing was uploaded.",
            missing.len(),
            missing.join(", ")
        );
    }
    if ids.len() != DECK_SIZE {
        bail!(
            "only {} of {DECK_SIZE} cards were found on Encore Decks. Nothing was uploaded.",
            ids.len()
        );
    }
    Ok(())
}

/// The body Encore Decks' deck builder posts to save a new deck.
pub fn save_request(name: &str, card_ids: &[String]) -> Value {
    serde_json::json!({
        "name": name.trim(),
        "description": "",
        "private": false,
        "attribute-group": [],
        "cards": card_ids,
    })
}

/// `{"deck": {"deckid": "abc", ...}}` -> `https://www.encoredecks.com/deck/abc`.
pub fn deck_url_from_response(json: &Value) -> Result<String> {
    json.pointer("/deck/deckid")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(|id| format!("{DECK_PAGE}{id}"))
        .ok_or_else(|| {
            let msg = json.get("message").and_then(Value::as_str).unwrap_or("");
            anyhow!("encoredecks did not return the new deck {msg}")
        })
}

/// `encoresesssionid=s%3Aabc.def; Path=/; HttpOnly` -> `s%3Aabc.def`.
pub fn session_from_set_cookie(header: &str) -> Option<String> {
    let (key, value) = header.split(';').next()?.split_once('=')?;
    (key.trim() == SESSION_COOKIE && !value.trim().is_empty()).then(|| value.trim().to_string())
}

/// `/user/Some%20Name` -> `Some Name`; `/login` (not logged in) -> `None`.
pub fn name_from_user_redirect(location: &str) -> Option<String> {
    let encoded = location
        .trim()
        .trim_start_matches("https://www.encoredecks.com")
        .strip_prefix("/user/")?
        .split(['/', '?', '#'])
        .next()
        .filter(|n| !n.is_empty())?;
    // Percent-decoding, byte-wise so multi-byte UTF-8 names survive.
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%')
            .then(|| encoded.get(i + 1..i + 3))
            .flatten()
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(b) => {
                out.push(b);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn cookie(session: &str) -> Zeroizing<String> {
    Zeroizing::new(format!("{SESSION_COOKIE}={session}"))
}

/// The account name of a session, or `None` when the session is not logged in.
pub fn account_name(session: &str) -> Result<Option<String>> {
    let response = ureq::get(USER_PAGE)
        .config()
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .header("Cookie", cookie(session).as_str())
        .call()
        .context("checking the Encore Decks session")?;
    Ok(response
        .headers()
        .get("location")
        .and_then(|l| l.to_str().ok())
        .and_then(name_from_user_redirect))
}

/// `{"username":…,"password":…}`, written straight into a buffer that is wiped when dropped
/// and sized up front (JSON escaping at most sextuples a character), so it never reallocates
/// and leaves no copy of the password behind.
fn login_body(email: &str, password: &str) -> Result<Zeroizing<Vec<u8>>> {
    let mut body = Zeroizing::new(Vec::with_capacity(32 + 6 * (email.len() + password.len())));
    body.extend_from_slice(b"{\"username\":");
    serde_json::to_writer(&mut *body, email.trim())?;
    body.extend_from_slice(b",\"password\":");
    serde_json::to_writer(&mut *body, password)?;
    body.push(b'}');
    Ok(body)
}

/// Logs in to Encore Decks with the account's email and password, over HTTPS only, and
/// returns the session. The password is sent once and not kept. Blocking.
pub fn login(email: &str, password: &str) -> Result<Account> {
    let body = login_body(email, password)?;
    let response = ureq::post(LOGIN_API)
        .config()
        .http_status_as_error(false)
        // Never let a redirect carry the credentials anywhere else.
        .max_redirects(0)
        .build()
        .header("Content-Type", "application/json")
        .send(&body[..])
        .context("signing in to encoredecks")?;
    match response.status().as_u16() {
        200 => {}
        401 => bail!("wrong email or password"),
        code => bail!("encoredecks answered {code} to the sign-in"),
    }
    let session = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|h| h.to_str().ok())
        .find_map(session_from_set_cookie)
        .map(Zeroizing::new)
        .ok_or_else(|| anyhow!("encoredecks did not return a session"))?;
    let name = account_name(&session)?
        .ok_or_else(|| anyhow!("encoredecks did not keep the session open"))?;
    Ok(Account { name, session })
}

/// Ends the session on encoredecks.com. Best effort: the tool forgets it anyway.
pub fn logout(session: &str) {
    let _ = ureq::post(LOGOUT_API)
        .config()
        .max_redirects(0)
        .build()
        .header("Cookie", cookie(session).as_str())
        .send_empty();
}

/// Publishes a deck on encoredecks.com and returns its link: in the linked account when
/// there is a session, as an anonymous deck otherwise. Blocking: run it off the UI thread.
/// Nothing is sent unless the deck has exactly [`DECK_SIZE`] cards, every one of them exists
/// on Encore Decks, and the session (if any) is still logged in: an expired session would
/// silently publish an anonymous deck instead ([`SessionExpired`] is returned).
pub fn upload(name: &str, codes: &[String], session: Option<&str>) -> Result<String> {
    if name.trim().is_empty() {
        bail!("the deck has no name");
    }
    check_deck_size(codes)?;
    let (ids, missing) = resolve_card_ids(codes, fetch_card_id)?;
    check_all_found(&ids, &missing)?;
    let mut request = ureq::post(SAVE_API)
        .config()
        // The session cookie must not follow a redirect to another host.
        .max_redirects(0)
        .build()
        .header("Content-Type", "application/json");
    if let Some(session) = session {
        if account_name(session)?.is_none() {
            return Err(SessionExpired.into());
        }
        request = request.header("Cookie", cookie(session).as_str());
    }
    let body = request
        .send(save_request(name, &ids).to_string())
        .context("sending the deck to encoredecks")?
        .body_mut()
        .read_to_string()
        .context("reading the encoredecks response")?;
    let json: Value = serde_json::from_str(&body).context("encoredecks returned invalid JSON")?;
    deck_url_from_response(&json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn link_parsing() {
        assert_eq!(
            deck_id_from_link(" https://www.encoredecks.com/deck/PPyvcLuvt "),
            Some("PPyvcLuvt")
        );
        assert_eq!(
            deck_id_from_link("https://www.encoredecks.com/deck/PPyvcLuvt/?x=1"),
            Some("PPyvcLuvt")
        );
        assert_eq!(deck_id_from_link("PPyvcLuvt"), None);
        assert_eq!(deck_id_from_link("https://www.encoredecks.com/"), None);
    }

    #[test]
    fn date_format() {
        assert_eq!(
            simulator_date("2025-09-30T22:20:36.123Z").as_deref(),
            Some("22:20  09/30/2025")
        );
        assert_eq!(simulator_date("garbage"), None);
    }

    #[test]
    fn json_parsing() {
        let deck = parse_deck_json(&json!({
            "name": " My deck ",
            "datemodified": "2025-01-02T03:04:05Z",
            "cards": [{"cardcode": "KS/W49-E001 "}, {"cardcode": "KS/W49-E002"}, {"x": 1}]
        }))
        .unwrap();
        assert_eq!(
            deck,
            EncoreDeck {
                name: "My deck".into(),
                date: "03:04  01/02/2025".into(),
                codes: vec!["KS/W49-E001".into(), "KS/W49-E002".into()],
            }
        );
        assert!(parse_deck_json(&json!({"cards": []})).is_err());
        let null_created = parse_deck_json(&json!({
            "name": "x", "datecreated": null, "datemodified": "2025-10-01T03:35:02.799Z",
            "cards": [{"cardcode": "A/B-001"}]
        }))
        .unwrap();
        assert_eq!(null_created.date, "03:35  10/01/2025");
    }

    #[test]
    fn card_id_resolution() {
        let codes: Vec<String> = ["KS/W49-001", "KS/W49-001", " ", "X/Y-1", "KS/W49-002SP"]
            .map(String::from)
            .to_vec();
        let mut calls = Vec::new();
        let (ids, missing) = resolve_card_ids(&codes, |c| {
            calls.push(c.to_string());
            Ok(match c {
                "KS/W49-E001" => Some("id1".into()),
                "KS/W49-E002SP" => Some("id2".into()),
                _ => None,
            })
        })
        .unwrap();
        assert_eq!(ids, ["id1", "id1", "id2"]);
        assert_eq!(missing, ["X/Y-1"]);
        // A code is looked up once, however many copies the deck has.
        assert_eq!(calls.iter().filter(|c| *c == "KS/W49-001").count(), 1);
    }

    #[test]
    fn upload_fail_safe() {
        let full: Vec<String> = vec!["A/B-001".into(); DECK_SIZE];
        assert!(check_deck_size(&full).is_ok());
        assert!(check_deck_size(&full[1..]).is_err());
        let mut blank = full.clone();
        blank[0] = " ".into();
        assert!(check_deck_size(&blank).is_err());

        let ids: Vec<String> = vec!["id".into(); DECK_SIZE];
        assert!(check_all_found(&ids, &[]).is_ok());
        let err = check_all_found(&ids[1..], &["X/Y-1".into()]).unwrap_err();
        assert!(err.to_string().contains("X/Y-1"));
        assert!(check_all_found(&ids[1..], &[]).is_err());
    }

    #[test]
    fn login_body_is_json() {
        let body = login_body(" me@x.y ", "p\"a\\ss\u{1}é").unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"username": "me@x.y", "password": "p\"a\\ss\u{1}é"})
        );
        assert!(body.capacity() >= body.len());
    }

    #[test]
    fn session_parsing() {
        assert_eq!(
            session_from_set_cookie(
                "encoresesssionid=s%3Aab.c%2Fd; Path=/; Expires=Wed, 29 Sep 2027; HttpOnly"
            )
            .as_deref(),
            Some("s%3Aab.c%2Fd")
        );
        assert_eq!(session_from_set_cookie("other=x; Path=/"), None);
        assert_eq!(session_from_set_cookie("encoresesssionid=; Path=/"), None);

        assert_eq!(
            name_from_user_redirect("/user/Benji").as_deref(),
            Some("Benji")
        );
        assert_eq!(
            name_from_user_redirect("https://www.encoredecks.com/user/A%20B%C3%A9").as_deref(),
            Some("A Bé")
        );
        assert_eq!(name_from_user_redirect("/login"), None);
        assert_eq!(name_from_user_redirect("/user/"), None);

        let account = Account {
            name: "Benji".into(),
            session: Zeroizing::new("s%3Asecret".into()),
        };
        let printed = format!("{account:?}");
        assert!(printed.contains("Benji") && !printed.contains("secret"));
    }

    /// Hits the network (read-only): `cargo test -- --ignored live_session_check`.
    #[test]
    #[ignore]
    fn live_session_check() {
        assert_eq!(account_name("s%3Anot-a-session").unwrap(), None);
        let err = login("nobody@example.invalid", "wrong").unwrap_err();
        assert!(err.to_string().contains("wrong email or password"));
    }

    #[test]
    fn save_body_and_response() {
        let body = save_request(" Deck ", &["a".into(), "a".into()]);
        assert_eq!(body["name"], "Deck");
        assert_eq!(body["cards"], json!(["a", "a"]));
        assert_eq!(body["attribute-group"], json!([]));
        assert_eq!(
            deck_url_from_response(&json!({"deck": {"deckid": "abc"}})).unwrap(),
            "https://www.encoredecks.com/deck/abc"
        );
        assert!(deck_url_from_response(&json!({"message": "something went wrong"})).is_err());
    }

    /// Hits the network (read-only): `cargo test -- --ignored live_card_lookup`.
    #[test]
    #[ignore]
    fn live_card_lookup() {
        let codes = vec![
            "ks/w49-e001".to_string(),
            "SDS/SX03-022".to_string(),
            "NOPE/X00-000".to_string(),
        ];
        let (ids, missing) = resolve_card_ids(&codes, fetch_card_id).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], "5c7b056969abf551db112951");
        assert_eq!(missing, ["NOPE/X00-000"]);
    }

    /// Hits the network: `cargo test -- --ignored live_encoredecks`.
    #[test]
    #[ignore]
    fn live_encoredecks() {
        let deck = fetch_deck("PPyvcLuvt").unwrap();
        assert!(!deck.name.is_empty());
        assert_eq!(deck.codes.len(), 50);
    }
}
