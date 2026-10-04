//! Simulator decks: one plain-text file per deck in `<root>/Decks`:
//!
//! ```text
//! # WeissSim Deck
//! Name: Konosuba Aqua and Eris
//! Date: 12:00  06/15/2010
//! Sleeve: RZ Emilia 1.jpg
//!
//! Cards:
//! KS/W49-E001
//! KS/W49-E001
//! ```
//!
//! `Sleeve` names an image of `<simulator>/<Game>_Data/StreamingAssets/Sleeves`; it is
//! empty when the deck has none.
//!
//! `AI_*.txt` files are the AI opponents' decks, copied there by the simulator.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const HEADER: &str = "# WeissSim Deck";
const AI_PREFIX: &str = "AI_";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckFile {
    pub name: String,
    pub date: String,
    /// File name of the deck's sleeve image in the simulator's `Sleeves` folder.
    pub sleeve: String,
    pub codes: Vec<String>,
    pub path: PathBuf,
    /// Deck of an AI opponent.
    pub ai: bool,
}

/// A sleeve image of the simulator's `Sleeves` folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sleeve {
    pub name: String,
    pub path: PathBuf,
}

/// Parses a deck file; the name falls back to the file name.
pub fn parse_deck(content: &str, path: &Path) -> DeckFile {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut deck = DeckFile {
        name: String::new(),
        date: String::new(),
        sleeve: String::new(),
        codes: Vec::new(),
        ai: stem.starts_with(AI_PREFIX),
        path: path.to_path_buf(),
    };
    let mut in_cards = false;
    for line in content.lines() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if in_cards {
            deck.codes.push(line.to_string());
        } else if let Some(rest) = line.strip_prefix("Name:") {
            deck.name = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("Date:") {
            deck.date = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("Sleeve:") {
            deck.sleeve = rest.trim().to_string();
        } else if line == "Cards:" {
            in_cards = true;
        }
    }
    if deck.name.is_empty() {
        deck.name = stem;
    }
    deck
}

/// Every deck in `dir`, sorted by name (a missing folder has no decks).
pub fn list_decks(dir: &Path) -> Result<Vec<DeckFile>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
    };
    let mut decks: Vec<DeckFile> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "txt"))
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .filter_map(|p| {
            let bytes = fs::read(&p).ok()?;
            Some(parse_deck(&String::from_utf8_lossy(&bytes), &p))
        })
        .collect();
    decks.sort_by_key(|d| (d.ai, d.name.to_lowercase()));
    Ok(decks)
}

pub fn format_deck(name: &str, date: &str, sleeve: &str, codes: &[String]) -> String {
    // The simulator writes `Sleeve:` with nothing after the colon when the deck has none.
    let sleeve = sleeve.trim();
    let sleeve = if sleeve.is_empty() {
        String::new()
    } else {
        format!(" {sleeve}")
    };
    let mut out = format!("{HEADER}\nName: {name}\nDate: {date}\nSleeve:{sleeve}\n\nCards:\n");
    for code in codes {
        out.push_str(code);
        out.push('\n');
    }
    out
}

/// Every sleeve image in `dir`, sorted by name (a missing folder has no sleeves).
pub fn list_sleeves(dir: &Path) -> Vec<Sleeve> {
    const IMAGE_EXTENSIONS: [&str; 3] = ["jpg", "jpeg", "png"];
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut sleeves: Vec<Sleeve> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|x| IMAGE_EXTENSIONS.iter().any(|e| x.eq_ignore_ascii_case(e)))
        })
        .filter_map(|p| {
            Some(Sleeve {
                name: p.file_name()?.to_string_lossy().into_owned(),
                path: p,
            })
        })
        .collect();
    sleeves.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    sleeves
}

/// `name` without the characters that are not allowed in file names on any system.
pub fn file_stem_for(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "Imported deck".to_string()
    } else {
        cleaned.to_string()
    }
}

/// Writes a new deck file, never overwriting one: a taken name gets a ` (2)`, ` (3)`… suffix,
/// which is also used as the deck name. Returns the saved deck.
pub fn save_new_deck(
    dir: &Path,
    name: &str,
    date: &str,
    sleeve: &str,
    codes: &[String],
) -> Result<DeckFile> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let stem = file_stem_for(name);
    let taken = |n: &str| {
        list_decks(dir)
            .map(|decks| decks.iter().any(|d| d.name.eq_ignore_ascii_case(n)))
            .unwrap_or(false)
    };
    let (final_name, path) = (1..)
        .map(|i| {
            if i == 1 {
                (name.trim().to_string(), stem.clone())
            } else {
                (format!("{} ({i})", name.trim()), format!("{stem} ({i})"))
            }
        })
        .map(|(n, s)| (n, dir.join(format!("{s}.txt"))))
        .find(|(n, p)| !p.exists() && !taken(n))
        .expect("an unused file name exists");
    let final_name = if final_name.is_empty() {
        stem
    } else {
        final_name
    };
    fs::write(&path, format_deck(&final_name, date, sleeve, codes))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(DeckFile {
        name: final_name,
        date: date.to_string(),
        sleeve: sleeve.to_string(),
        codes: codes.to_vec(),
        path,
        ai: false,
    })
}

/// Rewrites an existing deck file. When the name changed, the file is renamed to match it if
/// that file name is free. Fails when another deck already uses the new name.
pub fn update_deck(
    path: &Path,
    name: &str,
    date: &str,
    sleeve: &str,
    codes: &[String],
) -> Result<DeckFile> {
    let name = name.trim();
    let dir = path.parent().unwrap_or(Path::new("."));
    let clash = list_decks(dir)?
        .into_iter()
        .any(|d| d.path != path && d.name.eq_ignore_ascii_case(name));
    if clash {
        anyhow::bail!("another deck is already named \"{name}\"");
    }
    let renamed = dir.join(format!("{}.txt", file_stem_for(name)));
    let target = if renamed != path && !renamed.exists() {
        fs::rename(path, &renamed).with_context(|| format!("renaming {}", path.display()))?;
        renamed
    } else {
        path.to_path_buf()
    };
    fs::write(&target, format_deck(name, date, sleeve, codes))
        .with_context(|| format!("writing {}", target.display()))?;
    Ok(DeckFile {
        name: name.to_string(),
        date: date.to_string(),
        sleeve: sleeve.to_string(),
        codes: codes.to_vec(),
        ai: false,
        path: target,
    })
}

/// Current time in the simulator's deck date format, `HH:MM  MM/DD/YYYY`.
pub fn now_date() -> String {
    chrono::Local::now().format("%H:%M  %m/%d/%Y").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let deck = parse_deck(
            "\u{feff}# WeissSim Deck\nName: My deck\nDate: 12:00  06/15/2010\nSleeve: RZ Emilia 1.jpg\n\n\
             Cards:\nBD/W95-017\r\nBD/W95-017\n\nBD/W95-001\n",
            Path::new("/d/AI_file.txt"),
        );
        assert_eq!(deck.name, "My deck");
        assert_eq!(deck.date, "12:00  06/15/2010");
        assert_eq!(deck.sleeve, "RZ Emilia 1.jpg");
        assert_eq!(deck.codes, ["BD/W95-017", "BD/W95-017", "BD/W95-001"]);
        assert!(deck.ai);
        assert_eq!(
            parse_deck("Cards:\nA\n", Path::new("x/Fallback.txt")).name,
            "Fallback"
        );
        assert_eq!(
            parse_deck("# WeissSim Deck\nName: X\nCards:\nA\n", Path::new("x/None.txt")).sleeve,
            ""
        );
    }

    #[test]
    fn save_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let decks = dir.path().join("Decks");
        assert!(list_decks(&decks).unwrap().is_empty());

        let codes = vec!["KS/W49-E001".to_string(), "KS/W49-E002".to_string()];
        let a = save_new_deck(&decks, "Aqua / Eris?", "10:00  01/01/2025", "", &codes).unwrap();
        assert!(a.path.ends_with("Aqua - Eris-.txt"));
        assert_eq!(a.name, "Aqua / Eris?");
        assert_eq!(a.sleeve, "");
        let b = save_new_deck(
            &decks,
            "Aqua / Eris?",
            "10:00  01/01/2025",
            "RZ Emilia 1.jpg",
            &codes,
        )
        .unwrap();
        assert_eq!(b.name, "Aqua / Eris? (2)");
        assert!(b.path.ends_with("Aqua - Eris- (2).txt"));
        fs::write(
            decks.join("AI_Bot.txt"),
            format_deck("AI_Bot", "", "KS/W49-E001", &codes),
        )
        .unwrap();
        fs::write(decks.join("notes.md"), "").unwrap();

        let listed = list_decks(&decks).unwrap();
        let names: Vec<_> = listed.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Aqua / Eris?", "Aqua / Eris? (2)", "AI_Bot"]);
        assert_eq!(listed[0], a);
        assert_eq!(listed[1].sleeve, "RZ Emilia 1.jpg");
    }

    #[test]
    fn format_round_trip() {
        let codes = vec!["BD/W95-017".to_string()];
        // The simulator writes `Sleeve:` with nothing after the colon when there is none.
        let none = format_deck("My deck", "12:00  06/15/2010", "", &codes);
        assert!(none.contains("Sleeve:\n"));
        assert!(!none.contains("Sleeve: \n"));
        assert_eq!(
            parse_deck(&none, Path::new("d/My deck.txt")),
            DeckFile {
                name: "My deck".into(),
                date: "12:00  06/15/2010".into(),
                sleeve: String::new(),
                codes: codes.clone(),
                path: Path::new("d/My deck.txt").to_path_buf(),
                ai: false,
            }
        );
        let with_sleeve = format_deck("My deck", "12:00  06/15/2010", " RZ Emilia 1.jpg ", &codes);
        assert!(with_sleeve.contains("Sleeve: RZ Emilia 1.jpg\n"));
        assert_eq!(
            parse_deck(&with_sleeve, Path::new("d/My deck.txt")).sleeve,
            "RZ Emilia 1.jpg"
        );
    }

    #[test]
    fn sleeves_of_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list_sleeves(dir.path()).is_empty());

        let sleeves = dir.path().join("Sleeves");
        fs::create_dir_all(&sleeves).unwrap();
        fs::write(sleeves.join("B sleeve.jpg"), "").unwrap();
        fs::write(sleeves.join("a sleeve.png"), "").unwrap();
        fs::write(sleeves.join("ignored.txt"), "").unwrap();

        let listed = list_sleeves(&sleeves);
        assert_eq!(
            listed.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["a sleeve.png", "B sleeve.jpg"]
        );
        assert!(listed[0].path.ends_with("a sleeve.png"));
    }

    #[test]
    fn update_and_rename() {
        let dir = tempfile::tempdir().unwrap();
        let codes = vec!["A-001".to_string()];
        let a = save_new_deck(dir.path(), "Alpha", "d", "", &codes).unwrap();
        save_new_deck(dir.path(), "Beta", "d", "", &codes).unwrap();

        let more = vec!["A-001".to_string(), "A-002".to_string()];
        let same = update_deck(&a.path, "Alpha", "e", "RZ Rem 1.jpg", &more).unwrap();
        assert_eq!(same.path, a.path);
        assert_eq!(same.sleeve, "RZ Rem 1.jpg");
        let listed = list_decks(dir.path()).unwrap();
        assert_eq!(listed[0].codes, more);
        assert_eq!(listed[0].sleeve, "RZ Rem 1.jpg");

        let renamed = update_deck(&same.path, "Gamma", "e", "", &more).unwrap();
        assert!(renamed.path.ends_with("Gamma.txt"));
        assert!(!a.path.exists());
        let stored: Vec<_> = list_decks(dir.path())
            .unwrap()
            .into_iter()
            .map(|d| (d.name, d.sleeve))
            .collect();
        assert_eq!(
            stored,
            [
                ("Beta".to_string(), String::new()),
                ("Gamma".to_string(), String::new())
            ]
        );

        assert!(update_deck(&renamed.path, "beta", "e", "", &more).is_err());
        assert!(renamed.path.exists());
    }
}
