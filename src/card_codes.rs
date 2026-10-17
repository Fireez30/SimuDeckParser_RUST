//! The many spellings of a card code.
//!
//! Encore Decks, the simulator's card data and the artwork file names don't always agree on
//! the exact card code: English cards gain an `E` after the dash (`-E073`), trial decks use
//! `-TE`, promos `-PE`, foils add a rarity suffix (`073SP`) and some files are lowercase.
//! These helpers generate the candidate spellings, in the same order as the original tool.

use std::collections::HashSet;

use crate::text::remove_trailing_alphas;

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

/// A rewrite of the dash of a card code, and its reverse: `KS/W49-073` <-> `KS/W49-E073`.
type Rewrite = [(&'static str, &'static str); 2];
/// English cards: `-001` <-> `-E001`.
const ENGLISH: Rewrite = [("-", "-E"), ("-E", "-")];
/// English trial decks: `-T01` <-> `-TE01`.
const TRIAL: Rewrite = [("-T", "-TE"), ("-TE", "-T")];
/// English promos: `-P01` <-> `-PE01`.
const PROMO: Rewrite = [("-P", "-PE"), ("-PE", "-P")];

/// Candidate spellings used to match an imported code against the simulator's card keys:
/// the code as is, its rewrites, then the same without the rarity suffix (`-001SP` -> `-001`).
/// Each spelling is tried exactly, then case-insensitively.
fn key_variants(key: &str) -> Vec<Variant> {
    let exact = |s: String| Variant {
        candidate: s,
        lowercase: false,
    };
    let any_case = |s: String| Variant {
        candidate: s,
        lowercase: true,
    };
    // For each rewrite: both directions exactly, then both case-insensitively.
    let rewritten = |code: &str, rewrite: Rewrite| {
        let [(a_from, a_to), (b_from, b_to)] = rewrite;
        let a = code.replace(a_from, a_to);
        let b = code.replace(b_from, b_to);
        [exact(a.clone()), exact(b.clone()), any_case(a), any_case(b)]
    };

    let mut out = vec![exact(key.to_string()), any_case(key.to_string())];
    for rewrite in [ENGLISH, TRIAL, PROMO] {
        out.extend(rewritten(key, rewrite));
    }

    let stripped = remove_trailing_alphas(key).to_string();
    out.extend([exact(stripped.clone()), any_case(stripped.clone())]);
    // The original tool tried the promo rewrite first here.
    for rewrite in [PROMO, ENGLISH, TRIAL] {
        out.extend(rewritten(&stripped, rewrite));
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
