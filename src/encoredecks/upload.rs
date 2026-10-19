//! Publishing a simulator deck on encoredecks.com.
//!
//! Encore Decks stores decks as lists of its own card ids, so each card code is first looked up
//! on the site (trying its other spellings). Nothing is sent unless every card was found: an
//! uploaded deck is public, so it must be exactly the user's deck.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

use super::account::{SessionExpired, account_name, cookie};
use super::{CARD_API, DECK_PAGE, SAVE_API, read_json};
use crate::card_codes::encore_spellings;
use crate::deck_builder::DECK_SIZE;

/// Publishes a deck and returns its link: in the linked account when there is a session, as
/// an anonymous deck otherwise.
///
/// Nothing is sent unless the deck has exactly [`DECK_SIZE`] cards, every one of them exists
/// on Encore Decks, and the session (if any) is still logged in: an expired session would
/// silently publish an anonymous deck instead, so [`SessionExpired`] is returned.
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
    let response = request
        .send(save_request(name, &ids).to_string())
        .context("sending the deck to encoredecks")?;
    deck_url_from_response(&read_json(response)?)
}

/// Checked before anything is looked up: only full decks are published.
pub fn check_deck_size(codes: &[String]) -> Result<()> {
    let count = codes.iter().filter(|c| !c.trim().is_empty()).count();
    if count != DECK_SIZE {
        bail!("the deck has {count} cards; only {DECK_SIZE}-card decks can be uploaded");
    }
    Ok(())
}

/// Encore Decks' id of a card, or `None` when it has no card with this exact code.
fn fetch_card_id(code: &str) -> Result<Option<String>> {
    let response = ureq::get(CARD_API)
        .query("cardcode", code)
        .call()
        .with_context(|| format!("looking up {code} on encoredecks"))?;
    let json = read_json(response)?;
    Ok(json.get("_id").and_then(Value::as_str).map(str::to_string))
}

/// Maps deck codes to Encore Decks card ids (one per copy, in deck order), trying the other
/// spellings of each code (JP/EN, foil or not). Returns the ids and the codes not found.
///
/// `lookup` returns the id of an exact code; it is a parameter so tests can replace the site
/// with a fake. (`impl FnMut(…)` means "any function or closure with this signature".)
pub fn resolve_card_ids(
    codes: &[String],
    mut lookup: impl FnMut(&str) -> Result<Option<String>>,
) -> Result<(Vec<String>, Vec<String>)> {
    // Each code is looked up once, however many copies the deck has.
    let mut known: HashMap<&str, Option<String>> = HashMap::new();
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
