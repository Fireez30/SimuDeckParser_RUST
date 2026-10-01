//! `CardContent/Cards/<serie>/SingleSetData.txt` -> serie name and sets, each pointing at a
//! card folder:
//!
//! ```text
//! Series: Angel Beats!
//! Folder: Angel Beats/W31
//! Prefix: AB/W31
//! SetName: Re:Edit (W31)
//! ```

use std::path::{Path, PathBuf};

use super::cards::load_cards;
use super::effects::CommonEffects;
use crate::model::{Serie, Set};
use crate::text::decode_text;

#[derive(Default)]
struct SetBlock {
    folder: String,
    prefix: String,
    name: String,
    file: Option<String>,
}

/// Reads a serie folder: its display name (`Series:`, else the folder name) and its sets.
/// `None` when the folder has no `SingleSetData.txt` (plain card folders such as `Key/`).
pub fn load_serie(
    serie_path: &Path,
    cards_path: &Path,
    alternate_artwork: &Path,
    effects: &CommonEffects,
) -> Option<Serie> {
    let bytes = std::fs::read(serie_path.join("SingleSetData.txt")).ok()?;
    let mut name = serie_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Each `Folder:` starts a set; `Prefix:` (possibly repeated), `SetName:` and `FileName:`
    // (card file other than CardData.txt) describe it.
    let mut blocks: Vec<SetBlock> = Vec::new();
    for line in decode_text(&bytes).lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Series:") {
            if !rest.trim().is_empty() {
                name = rest.trim().to_string();
            }
        } else if let Some(rest) = line.strip_prefix("Folder:") {
            blocks.push(SetBlock {
                folder: rest.trim().to_string(),
                ..SetBlock::default()
            });
        } else if let Some(block) = blocks.last_mut() {
            if let Some(rest) = line.strip_prefix("Prefix:") {
                if block.prefix.is_empty() {
                    block.prefix = rest.trim().to_string();
                }
            } else if let Some(rest) = line.strip_prefix("SetName:") {
                block.name = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("FileName:") {
                block.file = Some(rest.trim().to_string()).filter(|f| !f.is_empty());
            }
        }
    }
    let sets: Vec<Set> = blocks
        .into_iter()
        .filter(|b| !b.folder.is_empty())
        .map(|b| {
            // Folders are relative to the Cards root, not to the serie.
            let folder = cards_path.join(&b.folder);
            let name = [b.name.as_str(), b.prefix.as_str(), b.folder.as_str()]
                .into_iter()
                .find(|n| !n.is_empty())
                .unwrap_or_default()
                .to_string();
            Set {
                key: b.prefix,
                name,
                cards: load_cards(&folder, b.file.as_deref(), alternate_artwork, effects),
                path: folder,
            }
        })
        .filter(|set| !set.cards.is_empty())
        .collect();
    (!sets.is_empty()).then(|| Serie {
        name,
        path: serie_path.to_path_buf(),
        sets,
    })
}

/// Every non-hidden folder under `cards_path` with a `SingleSetData.txt` is a serie, sorted by
/// name. Series are parsed in parallel.
pub fn load_series(
    cards_path: &Path,
    alternate_artwork: &Path,
    effects: &CommonEffects,
) -> std::io::Result<Vec<Serie>> {
    let mut folders: Vec<PathBuf> = std::fs::read_dir(cards_path)?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .collect();
    folders.sort();

    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = folders.len().div_ceil(workers).max(1);
    let series = std::thread::scope(|scope| {
        let handles: Vec<_> = folders
            .chunks(chunk)
            .map(|group| {
                scope.spawn(move || {
                    group
                        .iter()
                        .filter_map(|path| load_serie(path, cards_path, alternate_artwork, effects))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("serie parser thread panicked"))
            .collect::<Vec<_>>()
    });
    let mut series = series;
    series.sort_by_key(|s| s.name.to_lowercase());
    Ok(series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_series_and_sets() {
        let dir = tempfile::tempdir().unwrap();
        let cards = dir.path().join("CardContent/Cards");
        let serie = cards.join("Konosuba");
        let alternate = dir.path().join("AlternateArtwork");
        fs::create_dir_all(serie.join("W49")).unwrap();
        fs::create_dir_all(cards.join(".hidden")).unwrap();
        fs::create_dir_all(cards.join("Key/W50")).unwrap();
        fs::create_dir_all(alternate.join("ENW49")).unwrap();
        fs::write(
            serie.join("SingleSetData.txt"),
            "\u{feff}Series: KonoSuba!\r\nFolder: Konosuba/W49\nPrefix: KS/W49\nSetName: Booster\n\n\
             Folder: Key/W50\nPrefix: K/W50\nPrefix: K2/W50\nFileName: Premium\n\n\
             Folder: Konosuba/Missing\nSetName: Not shipped\n",
        )
        .unwrap();
        fs::write(
            serie.join("W49/CardData.txt"),
            "Character: KS/W49-E001\nName Aqua\nEndCard\n",
        )
        .unwrap();
        fs::write(
            cards.join("Key/W50/Premium.txt"),
            [0xFF, 0xFE]
                .into_iter()
                .chain(
                    "Character: K/W50-E001\r\nName Kud\r\nEndCard\r\n"
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes),
                )
                .collect::<Vec<u8>>(),
        )
        .unwrap();
        fs::write(alternate.join("ENW49/KS_W49_E001.jpg"), b"").unwrap();

        let series = load_series(&cards, &alternate, &CommonEffects::new()).unwrap();
        // `Key` holds card folders only, so it is not a serie.
        assert_eq!(series.len(), 1);
        let s = &series[0];
        assert_eq!(s.name, "KonoSuba!");
        assert_eq!(s.sets.len(), 2);
        let set = &s.sets[0];
        assert_eq!((set.key.as_str(), set.name.as_str()), ("KS/W49", "Booster"));
        let card = &set.cards["KS/W49-E001"];
        assert_eq!(card.name, "Aqua");
        assert!(
            card.image
                .as_ref()
                .unwrap()
                .ends_with("ENW49/KS_W49_E001.jpg")
        );
        // No SetName: named after its first prefix, cards read from the FileName file.
        assert_eq!(s.sets[1].name, "K/W50");
        assert_eq!(s.sets[1].cards["K/W50-E001"].name, "Kud");
    }
}
