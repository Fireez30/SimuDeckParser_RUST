//! Finding loaded cards: by code ([`CardIndex`]), and by the names quoted in effect text
//! ([`NameIndex`]).

use std::collections::{HashMap, HashSet};

use crate::card_codes::transform_to_existing_keys;
use crate::model::{Card, Deck, Serie};
use crate::text::{remove_trailing_alphas, split_names};

/// Lookup of every loaded card by code, across all series.
///
/// It borrows the cards rather than copying them: the `'a` ("lifetime") says the index may not
/// outlive the series it was built from.
pub struct CardIndex<'a> {
    exact: HashMap<&'a str, &'a Card>,
    lowercase: HashMap<String, &'a Card>,
}

impl<'a> CardIndex<'a> {
    pub fn new(series: &'a [Serie]) -> Self {
        let mut exact = HashMap::new();
        let mut lowercase = HashMap::new();
        for card in series
            .iter()
            .flat_map(|s| &s.sets)
            .flat_map(|set| set.cards.values())
        {
            // First occurrence wins, like the original series-then-set scan.
            exact.entry(card.key.as_str()).or_insert(card);
            lowercase.entry(card.key.to_lowercase()).or_insert(card);
        }
        Self { exact, lowercase }
    }

    pub fn codes(&self) -> HashSet<String> {
        self.exact.keys().map(|k| k.to_string()).collect()
    }

    /// Exact, then without rarity suffix, then case-insensitively for both.
    pub fn get(&self, code: &str) -> Option<&'a Card> {
        let code = code.trim();
        let stripped = remove_trailing_alphas(code);
        self.exact
            .get(code)
            .or_else(|| self.exact.get(stripped))
            .copied()
            .or_else(|| self.lowercase.get(&code.to_lowercase()).copied())
            .or_else(|| self.lowercase.get(&stripped.to_lowercase()).copied())
    }

    /// Matches the codes of a deck to loaded cards, trying the other spellings of each code
    /// (JP/EN, foil or not). Codes that match nothing end up in [`Deck::unmatched`].
    pub fn resolve_deck(&self, name: &str, date: &str, codes: &[String]) -> Deck {
        let mut deck = Deck {
            name: name.to_string(),
            date: date.to_string(),
            ..Deck::default()
        };
        for code in transform_to_existing_keys(&self.codes(), codes) {
            if code.trim().is_empty() {
                continue;
            }
            match self.get(&code) {
                Some(card) => deck.cards.push(card.clone()),
                None => deck.unmatched.push(code),
            }
        }
        deck
    }
}

/// Cards by name, to resolve the names quoted in effect text (`"Aqua" gets +1000 power`).
/// Names are matched exactly: effect text spells them as the cards do.
#[derive(Default)]
pub struct NameIndex {
    by_name: HashMap<String, Vec<Card>>,
}

impl NameIndex {
    pub fn new(series: &[Serie]) -> Self {
        let mut by_name: HashMap<String, Vec<Card>> = HashMap::new();
        for card in series
            .iter()
            .flat_map(|s| &s.sets)
            .flat_map(|set| set.cards.values())
            .filter(|c| !c.name.trim().is_empty())
        {
            by_name
                .entry(card.name.trim().to_owned())
                .or_default()
                .push(card.clone());
        }
        Self { by_name }
    }

    /// The card named `name`, preferring one of the same series as `from`
    /// (same code prefix before the `/`), as names repeat across series.
    pub fn get(&self, name: &str, from: &Card) -> Option<&Card> {
        let cards = self.by_name.get(name.trim())?;
        let own = series_prefix(from);
        cards
            .iter()
            .find(|c| c.key != from.key && series_prefix(c) == own)
            .or_else(|| cards.iter().find(|c| c.key != from.key))
    }

    /// The cards named in the effect text of `card`.
    pub fn named_by(&self, card: &Card) -> Vec<&Card> {
        let mut out: Vec<&Card> = Vec::new();
        for line in card.text.lines() {
            for (_, name) in split_names(line, |n| self.get(n, card).is_some()) {
                if let Some(named) = name.and_then(|n| self.get(n, card))
                    && !out.iter().any(|c| c.key == named.key)
                {
                    out.push(named);
                }
            }
        }
        out
    }

    /// Cards that react with `target`: the ones its text names, and the ones of its series
    /// whose text names it (or a card of the same name), by code.
    pub fn related(&self, target: &Card) -> Related<'_> {
        let own = series_prefix(target);
        let mut naming: Vec<&Card> = self
            .by_name
            .values()
            .flatten()
            .filter(|c| c.key != target.key && c.text.contains(&target.name))
            .filter(|c| {
                self.named_by(c)
                    .iter()
                    .any(|n| n.name == target.name && series_prefix(n) == own)
            })
            .collect();
        naming.sort_by(|a, b| a.key.cmp(&b.key));
        naming.dedup_by(|a, b| a.key == b.key);
        Related {
            named: self.named_by(target),
            naming,
        }
    }
}

/// See [`NameIndex::related`].
pub struct Related<'a> {
    /// Cards named in the target's text.
    pub named: Vec<&'a Card>,
    /// Cards whose text names the target.
    pub naming: Vec<&'a Card>,
}

/// `KS` for `KS/W49-E001`: names repeat across series, not within one.
fn series_prefix(card: &Card) -> &str {
    card.key.split('/').next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        use crate::model::{CardType, Color, Set};
        let card = |key: &str, name: &str| Card {
            key: key.into(),
            card_type: CardType::Character,
            name: name.into(),
            image: None,
            color: Color::Yellow,
            level: 0,
            cost: 0,
            power: 0,
            triggers: vec![],
            soul_count: 0,
            traits: vec![],
            code: String::new(),
            text: String::new(),
        };
        let serie = |cards: Vec<Card>| Serie {
            sets: vec![Set {
                cards: cards.into_iter().map(|c| (c.key.clone(), c)).collect(),
                ..Set::default()
            }],
            ..Serie::default()
        };
        let series = [
            serie(vec![card("AA/W1-001", "Aqua")]),
            serie(vec![
                card("KS/W49-001", "Aqua"),
                card("KS/W49-002", "Megumin"),
            ]),
        ];
        let index = NameIndex::new(&series);
        let from = card("KS/W49-003", "Kazuma");
        assert_eq!(index.get("Aqua ", &from).unwrap().key, "KS/W49-001");
        assert_eq!(
            index.get("Aqua", &card("XX/Y1-001", "")).unwrap().key,
            "AA/W1-001"
        );
        // Never the card itself.
        assert!(
            index
                .get("Megumin", &card("KS/W49-002", "Megumin"))
                .is_none()
        );
        assert!(index.get("Darkness", &from).is_none());

        let mut kazuma = card("KS/W49-003", "Kazuma");
        kazuma.text = r#"If "Aqua" and 'Megumin' are on stage"#.into();
        let mut other = card("AA/W1-002", "Other");
        other.text = r#""Aqua""#.into();
        let mut megumin = card("KS/W49-004", "Megumin");
        megumin.text = r#"Your "Kazuma" gets +1000"#.into();
        let series = [
            serie(vec![card("AA/W1-001", "Aqua"), other]),
            serie(vec![card("KS/W49-001", "Aqua"), kazuma.clone(), megumin]),
        ];
        let index = NameIndex::new(&series);
        let keys = |cards: Vec<&Card>| cards.iter().map(|c| c.key.clone()).collect::<Vec<_>>();
        let related = index.related(&kazuma);
        assert_eq!(keys(related.named), ["KS/W49-001", "KS/W49-004"]);
        assert_eq!(keys(related.naming), ["KS/W49-004"]);
        // The other series' "Aqua" is named by its own series only.
        let aqua = index.get("Aqua", &kazuma).unwrap().clone();
        assert_eq!(keys(index.related(&aqua).naming), ["KS/W49-003"]);
    }
}
