//! Card code heuristics.
//!
//! Encore Decks, the simulator's card data and the artwork file names don't always agree on
//! the exact card code: English cards gain an `E` after the dash (`-E073`), trial decks use
//! `-TE`, promos `-PE`, foils add a rarity suffix (`073SP`) and some files are lowercase.
//! These helpers generate the candidate spellings in the same order as the original tool.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::model::{Card, Serie};
use crate::text::{image_stem, remove_trailing_alphas, split_names};

/// A candidate spelling of a card code. `lowercase` means the lookup is done on the
/// lowercased candidate, while the candidate itself (original case) is what gets returned.
struct Variant {
    candidate: String,
    lowercase: bool,
}

impl Variant {
    fn lookup_key(&self) -> String {
        if self.lowercase {
            self.candidate.to_lowercase()
        } else {
            self.candidate.clone()
        }
    }
}

/// The dash rewrites tried, in order, for key matching (both directions).
const KEY_REWRITES: [(&str, &str); 6] = [
    ("-", "-E"),
    ("-E", "-"),
    ("-T", "-TE"),
    ("-TE", "-T"),
    ("-P", "-PE"),
    ("-PE", "-P"),
];

/// Candidate spellings used to match an imported code against the simulator's card keys.
fn key_variants(key: &str) -> Vec<Variant> {
    let plain = |s: String| Variant {
        candidate: s,
        lowercase: false,
    };
    let lower = |s: String| Variant {
        candidate: s,
        lowercase: true,
    };
    let mut out = vec![plain(key.to_string()), lower(key.to_string())];

    // (-E, -TE, -PE rewrites) exact then lowercase, per rewrite pair
    for pair in KEY_REWRITES.chunks(2) {
        let a = key.replace(pair[0].0, pair[0].1);
        let b = key.replace(pair[1].0, pair[1].1);
        out.extend([plain(a.clone()), plain(b.clone()), lower(a), lower(b)]);
    }

    // Same again on the code without its rarity suffix; order follows the original tool:
    // bare, -PE, -E, -TE.
    let stripped = remove_trailing_alphas(key).to_string();
    out.extend([plain(stripped.clone()), lower(stripped.clone())]);
    for pair in [
        &KEY_REWRITES[4..6],
        &KEY_REWRITES[0..2],
        &KEY_REWRITES[2..4],
    ] {
        let a = stripped.replace(pair[0].0, pair[0].1);
        let b = stripped.replace(pair[1].0, pair[1].1);
        out.extend([plain(a.clone()), plain(b.clone()), lower(a), lower(b)]);
    }
    out
}

/// Maps each imported code to the spelling that exists in the simulator, or keeps it unchanged.
pub fn transform_to_existing_keys(existing: &HashSet<String>, keys: &[String]) -> Vec<String> {
    keys.iter()
        .map(|key| {
            key_variants(key)
                .into_iter()
                .find(|v| existing.contains(&v.lookup_key()))
                .map(|v| v.candidate)
                .unwrap_or_else(|| key.clone())
        })
        .collect()
}

/// Spellings of a simulator code to try against Encore Decks, which matches codes exactly:
/// the code as-is, then the -E / -TE / -PE rewrites, then without the rarity suffix, then the
/// same in uppercase. No duplicates.
///
/// Encore Decks only knows cards with several arts by their first art (`SDS/SX03-022a`, never
/// `SDS/SX03-022`), so a spelling ending in a digit is tried with an `a` first.
pub fn encore_spellings(key: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for code in [key.trim().to_string(), key.trim().to_uppercase()] {
        for v in key_variants(&code) {
            if v.lowercase {
                continue;
            }
            let first_art = v
                .candidate
                .ends_with(|c: char| c.is_ascii_digit())
                .then(|| format!("{}a", v.candidate));
            for spelling in first_art.into_iter().chain([v.candidate]) {
                if !out.contains(&spelling) {
                    out.push(spelling);
                }
            }
        }
    }
    out
}

/// Artwork files (`.jpg`, `.jpeg`, `.png`) of one folder, by lowercase file stem, so a set's
/// folder is listed once instead of probing every spelling of every card.
#[derive(Debug, Default)]
pub struct ImageDir(HashMap<String, PathBuf>);

impl ImageDir {
    pub fn read(dir: &Path) -> Self {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Self::default();
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|x| {
                    ["jpg", "jpeg", "png"].contains(&x.to_string_lossy().to_lowercase().as_str())
                })
            })
            .collect();
        // Deterministic pick when two files differ only by case or extension (jpg first).
        files.sort();
        let mut map = HashMap::new();
        for path in files {
            if let Some(stem) = path.file_stem() {
                map.entry(stem.to_string_lossy().to_lowercase())
                    .or_insert(path);
            }
        }
        Self(map)
    }

    fn get(&self, stem: &str) -> Option<&PathBuf> {
        self.0.get(&stem.to_lowercase())
    }
}

/// Image file stems tried for a card, in order: the card's `Image` line, then the code as-is,
/// -E, -TE and -PE, then the same without the series prefix (`W107_E001` for `BTR/W107-001`),
/// then the card number alone (`001`).
fn image_stems(key: &str, hint: Option<&str>) -> Vec<String> {
    let rewrites = |code: &str| {
        [
            code.to_string(),
            code.replace('-', "-E"),
            code.replace("-T", "-TE"),
            code.replace("-P", "-PE"),
        ]
        .map(|s| image_stem(&s))
    };
    let mut out: Vec<String> = hint
        .into_iter()
        .flat_map(|h| [h.trim().to_string(), image_stem(h)])
        .collect();
    out.extend(rewrites(key));
    if let Some((_, without_prefix)) = key.split_once('/') {
        out.extend(rewrites(without_prefix));
    }
    if let Some((_, number)) = key.rsplit_once('-') {
        out.push(number.to_string());
    }
    out
}

/// Finds the artwork for a card: the set folder first, then (older simulators) the alternate
/// artwork folders `<set>` (JP foils) and `EN<set>` (EN foils), where `<set>` comes from
/// `ABC/<set>-001`.
pub fn resolve_card_image(
    set_images: &ImageDir,
    key: &str,
    hint: Option<&str>,
    alternate_artwork: &Path,
) -> Option<PathBuf> {
    let stems = image_stems(key, hint);
    if let Some(found) = stems.iter().find_map(|s| set_images.get(s)) {
        return Some(found.clone());
    }
    if !alternate_artwork.is_dir() {
        return None;
    }
    let foil_set = key
        .split('/')
        .nth(1)
        .and_then(|rest| rest.split_once('-'))
        .map(|(set, _)| set)
        .unwrap_or("");
    [
        alternate_artwork.join(foil_set),
        alternate_artwork.join(format!("EN{foil_set}")),
    ]
    .iter()
    .flat_map(|folder| {
        stems.iter().flat_map(move |stem| {
            [
                folder.join(format!("{stem}.jpg")),
                folder.join(format!("{}.jpg", stem.to_lowercase())),
            ]
        })
    })
    .find(|p| p.is_file())
}

/// Lookup of every loaded card by code, across all series.
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
    use std::fs;

    #[test]
    fn encore_spellings_order() {
        let spellings = encore_spellings("ks/w49-001sp");
        assert_eq!(spellings[0], "ks/w49-001sp");
        assert_eq!(spellings[1], "ks/w49-E001sp");
        assert!(spellings.contains(&"KS/W49-E001".to_string()));
        let unique: HashSet<&String> = spellings.iter().collect();
        assert_eq!(unique.len(), spellings.len());
        // No `a` after a letter: `001SPa` is never a card.
        assert!(
            !spellings
                .iter()
                .any(|s| s.ends_with("spa") || s.ends_with("SPa"))
        );

        // Several arts: the first art comes before the plain code, for every rewrite.
        let spellings = encore_spellings("SY/WE09-19");
        assert_eq!(
            &spellings[..4],
            ["SY/WE09-19a", "SY/WE09-19", "SY/WE09-E19a", "SY/WE09-E19"]
        );
    }

    fn existing(codes: &[&str]) -> HashSet<String> {
        codes.iter().map(|s| s.to_string()).collect()
    }

    fn transform(existing_codes: &[&str], key: &str) -> String {
        transform_to_existing_keys(&existing(existing_codes), &[key.to_string()]).remove(0)
    }

    #[test]
    fn key_variant_order_matches_original() {
        let v: Vec<(String, bool)> = key_variants("AB/W1-T01S")
            .into_iter()
            .map(|v| (v.candidate, v.lowercase))
            .collect();
        assert_eq!(v.len(), 28);
        assert_eq!(v[0], ("AB/W1-T01S".into(), false));
        assert_eq!(v[1], ("AB/W1-T01S".into(), true));
        assert_eq!(v[2], ("AB/W1-ET01S".into(), false));
        assert_eq!(v[6], ("AB/W1-TE01S".into(), false));
        assert_eq!(v[14], ("AB/W1-T01".into(), false));
        assert_eq!(v[16], ("AB/W1-T01".into(), false)); // -P rewrite is a no-op here
        assert_eq!(v[20], ("AB/W1-ET01".into(), false));
        assert_eq!(v[24], ("AB/W1-TE01".into(), false));
    }

    #[test]
    fn transform_cases() {
        assert_eq!(transform(&["KS/W49-E073"], "KS/W49-E073"), "KS/W49-E073");
        assert_eq!(transform(&["KS/W49-E073"], "KS/W49-073"), "KS/W49-E073");
        assert_eq!(transform(&["KS/W49-073"], "KS/W49-E073"), "KS/W49-073");
        assert_eq!(transform(&["KS/W49-TE01"], "KS/W49-T01"), "KS/W49-TE01");
        assert_eq!(transform(&["KS/W49-PE01"], "KS/W49-P01"), "KS/W49-PE01");
        assert_eq!(transform(&["KS/W49-073"], "KS/W49-073SP"), "KS/W49-073");
        assert_eq!(transform(&["KS/W49-E073"], "KS/W49-073SP"), "KS/W49-E073");
        // lowercase match keeps the original case
        assert_eq!(transform(&["ks/w49-e073"], "KS/W49-E073"), "KS/W49-E073");
        // unknown codes pass through
        assert_eq!(transform(&[], "XX/Y1-001"), "XX/Y1-001");
    }

    #[test]
    fn resolve_images() {
        let dir = tempfile::tempdir().unwrap();
        let set = dir.path().join("Cards/W49");
        let alt = dir.path().join("AlternateArtwork");
        fs::create_dir_all(&set).unwrap();
        fs::create_dir_all(alt.join("ENW49")).unwrap();
        for file in [
            "KS_W49_E001.jpg",
            "ks_w49_e002.jpg",
            "W49_E004.jpg",
            "ks_w49_005.png",
            "035.jpg",
            "007.jpg",
            "notes.txt",
        ] {
            fs::write(set.join(file), b"").unwrap();
        }
        fs::write(alt.join("ENW49/KS_W49_E003SP.jpg"), b"").unwrap();
        let images = ImageDir::read(&set);
        let resolve = |key, hint| resolve_card_image(&images, key, hint, &alt);

        assert_eq!(
            resolve("KS/W49-001", None),
            Some(set.join("KS_W49_E001.jpg"))
        );
        // -E rewrite + lowercase (was never reached in the C++ version)
        assert_eq!(
            resolve("KS/W49-002", None),
            Some(set.join("ks_w49_e002.jpg"))
        );
        assert_eq!(
            resolve("KS/W49-003SP", None),
            Some(alt.join("ENW49/KS_W49_E003SP.jpg"))
        );
        // Without the series prefix, png, and the card's `Image` line.
        assert_eq!(resolve("KS/W49-004", None), Some(set.join("W49_E004.jpg")));
        assert_eq!(
            resolve("KS/W49-005", None),
            Some(set.join("ks_w49_005.png"))
        );
        assert_eq!(
            resolve("KS/W49-006", Some("035")),
            Some(set.join("035.jpg"))
        );
        assert_eq!(resolve("KS/W49-007", None), Some(set.join("007.jpg")));
        assert_eq!(resolve("KS/W49-999", None), None);
    }

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
