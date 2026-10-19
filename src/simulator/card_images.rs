//! Finding the artwork of a card in the simulator's folders.
//!
//! The card data rarely says where a card's picture is: it is found by trying the file names the
//! simulator uses (`KS_W49_E001.jpg` for `KS/W49-E001`), with the same code rewrites as
//! [`crate::card_codes`] (`-E`, `-TE`, `-PE`), case-insensitively.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::text::image_stem;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
        // -E rewrite + lowercase (the original C++ tool never got this far)
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
}
