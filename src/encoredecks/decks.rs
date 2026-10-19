//! Reading decks from encoredecks.com, and importing them into the simulator.

use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

use super::account::{SessionExpired, account_name, cookie};
use super::{DECK_API, DECK_PAGE, SEARCH_API, read_json};
use crate::card_codes::transform_to_existing_keys;
use crate::card_index::CardIndex;
use crate::model::{Deck, Serie};
use crate::simulator::deck_files;

/// A deck as encoredecks.com serves it: card codes only.
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
    // `https:`, ``, `www.encoredecks.com`, `deck`, then the id.
    let parts: Vec<&str> = link.split('/').collect();
    if parts.len() <= 4 {
        return None;
    }
    let id = parts[parts.len() - 1];
    if id.is_empty() { None } else { Some(id) }
}

/// `PPyvcLuvt` -> `https://www.encoredecks.com/deck/PPyvcLuvt`: a link [`import`] accepts.
pub fn deck_link(id: &str) -> String {
    format!("{DECK_PAGE}{}", id.trim())
}

/// `2025-09-30T22:20:36.123Z` -> `22:20  09/30/2025`.
pub fn simulator_date(iso: &str) -> Option<String> {
    let trimmed = iso.trim().get(..19)?;
    chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|dt| dt.format("%H:%M  %m/%d/%Y").to_string())
}

/// The text field `key` of a JSON object, trimmed; `None` when it is absent or not text.
fn text_field<'a>(json: &'a Value, key: &str) -> Option<&'a str> {
    json.get(key).and_then(Value::as_str).map(str::trim)
}

/// The first of the date fields `keys` that is set, in the simulator's date format.
fn date_field(json: &Value, keys: [&str; 2]) -> String {
    keys.iter()
        .find_map(|key| json.get(*key).and_then(Value::as_str))
        .and_then(simulator_date)
        .unwrap_or_default()
}

pub fn parse_deck_json(json: &Value) -> Result<EncoreDeck> {
    let name = text_field(json, "name")
        .filter(|n| !n.is_empty())
        .ok_or_else(|| anyhow!("the deck has no name"))?
        .to_string();
    let date = date_field(json, ["datecreated", "datemodified"]);
    let codes: Vec<String> = json
        .get("cards")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("the deck has no card list"))?
        .iter()
        .filter_map(|card| text_field(card, "cardcode"))
        .map(str::to_string)
        .collect();
    if codes.is_empty() {
        bail!("the deck is empty");
    }
    Ok(EncoreDeck { name, date, codes })
}

/// Downloads the deck `id` (the last part of its link).
pub fn fetch_deck(id: &str) -> Result<EncoreDeck> {
    let url = format!("{DECK_API}{}", id.trim());
    let response = ureq::get(&url)
        .call()
        .with_context(|| format!("requesting {url}"))?;
    parse_deck_json(&read_json(response)?)
}

/// Downloads a deck and saves it as a new file in `decks_dir`, matched to the loaded cards.
/// Returns the deck as saved: its name gets a ` (2)` suffix when taken.
pub fn import(link: &str, series: &[Serie], decks_dir: &Path) -> Result<Deck> {
    let id =
        deck_id_from_link(link).ok_or_else(|| anyhow!("not an encoredecks deck link: {link:?}"))?;
    let fetched = fetch_deck(id)?;

    let index = CardIndex::new(series);
    let codes = transform_to_existing_keys(&index.codes(), &fetched.codes);
    let saved = deck_files::save_new_deck(decks_dir, &fetched.name, &fetched.date, "", &codes)?;

    Ok(index.resolve_deck(&saved.name, &saved.date, &codes))
}

/// One of the linked account's decks, as the deck search lists it (no cards).
#[derive(Clone, Debug, PartialEq)]
pub struct AccountDeck {
    pub name: String,
    /// Encore Decks id: the last part of the deck's link.
    pub id: String,
    /// Already formatted the way the simulator stores it: `HH:MM  MM/DD/YYYY`.
    pub date: String,
    pub description: String,
}

/// One page of the account's decks; the search API serves 24 decks per page.
#[derive(Debug, PartialEq)]
pub struct DeckPage {
    pub decks: Vec<AccountDeck>,
    /// 1-based page number.
    pub page: u32,
    pub total_pages: u32,
    pub total_decks: u32,
}

/// Parses one page of the deck search response.
pub fn parse_search_json(json: &Value) -> Result<DeckPage> {
    let decks = json
        .get("decks")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("encoredecks returned no deck list"))?;
    let decks: Vec<AccountDeck> = decks
        .iter()
        .map(|deck| AccountDeck {
            name: text_field(deck, "name")
                .filter(|n| !n.is_empty())
                .unwrap_or("(unnamed deck)")
                .to_string(),
            id: text_field(deck, "deckid").unwrap_or_default().to_string(),
            date: date_field(deck, ["datemodified", "datecreated"]),
            description: text_field(deck, "description")
                .unwrap_or_default()
                .to_string(),
        })
        .filter(|deck| !deck.id.is_empty())
        .collect();
    let number = |key: &str, default: u64| json.get(key).and_then(Value::as_u64).unwrap_or(default);
    Ok(DeckPage {
        decks,
        page: number("page", 1) as u32,
        total_pages: number("totalPages", 1).max(1) as u32,
        total_decks: number("totalDecks", 0) as u32,
    })
}

/// One page (1-based, 24 decks) of the linked account's decks, private and unfinished ones
/// included.
pub fn fetch_account_decks(session: &str, page: u32) -> Result<DeckPage> {
    // Logged out, the search would silently list public decks instead.
    if account_name(session)?.is_none() {
        return Err(SessionExpired.into());
    }
    let url = format!("{SEARCH_API}?page={}&username=true&invalid=1", page.max(1));
    let response = ureq::get(&url)
        .config()
        .max_redirects(0)
        .build()
        .header("Cookie", cookie(session).as_str())
        .call()
        .with_context(|| format!("requesting {url}"))?;
    parse_search_json(&read_json(response)?)
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
    fn search_page_parsing() {
        assert_eq!(
            deck_link(" PPyvcLuvt "),
            "https://www.encoredecks.com/deck/PPyvcLuvt"
        );
        // Shape of `/api/search/deck` (fields the select leaves out are absent).
        let page = parse_search_json(&json!({
            "decks": [
                {"name": " My deck ", "deckid": "PPyvcLuvt",
                 "datemodified": "2025-01-02T03:04:05Z", "description": " fun "},
                {"name": "", "deckid": "  ", "description": null},
                {"name": "Old deck", "deckid": "abc",
                 "datecreated": "2024-12-31T23:59:59.999Z", "description": ""}
            ],
            "totalDecks": 25, "limit": 24, "totalPages": 2, "page": 1,
            "hasNextPage": true
        }))
        .unwrap();
        assert_eq!(
            page,
            DeckPage {
                decks: vec![
                    AccountDeck {
                        name: "My deck".into(),
                        id: "PPyvcLuvt".into(),
                        date: "03:04  01/02/2025".into(),
                        description: "fun".into(),
                    },
                    AccountDeck {
                        name: "Old deck".into(),
                        id: "abc".into(),
                        date: "23:59  12/31/2024".into(),
                        description: String::new(),
                    },
                ],
                page: 1,
                total_pages: 2,
                total_decks: 25,
            }
        );
        assert!(parse_search_json(&json!({"message": "Something went wrong"})).is_err());
        let empty = parse_search_json(&json!({"decks": []})).unwrap();
        assert_eq!((empty.decks.len(), empty.total_pages), (0, 1));
    }

    /// Hits the network (read-only): `cargo test -- --ignored live_encoredecks`.
    #[test]
    #[ignore]
    fn live_encoredecks() {
        let deck = fetch_deck("PPyvcLuvt").unwrap();
        assert!(!deck.name.is_empty());
        assert_eq!(deck.codes.len(), 50);
    }

    /// Hits the network (read-only): `cargo test -- --ignored live_account_decks`.
    #[test]
    #[ignore]
    fn live_account_decks() {
        let err = fetch_account_decks("s%3Anot-a-session", 1).unwrap_err();
        assert!(err.is::<SessionExpired>());
    }
}
