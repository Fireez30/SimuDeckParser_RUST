//! Where things live in a simulator install:
//!
//! ```text
//! <root>/CardContent/Cards/<serie>/SingleSetData.txt   series, sets and card data
//! <root>/Decks/<deck>.txt                             decks, plain text
//! <root>/<Game>_Data/StreamingAssets/                 common effects, AI decks
//! ```

use std::path::{Path, PathBuf};

const EFFECTS_FILES: [&str; 2] = [
    "CommonEffects_USER_REFERENCE.txt",
    "CommonEffects(copy).txt",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub root: PathBuf,
}

impl Layout {
    /// `root` if it is a simulator folder.
    pub fn at(root: &Path) -> Option<Self> {
        root.join("CardContent")
            .join("Cards")
            .is_dir()
            .then(|| Self {
                root: root.to_path_buf(),
            })
    }

    /// The simulator folder containing `path` (users often pick a subfolder such as `Decks`,
    /// `CardContent` or `Weiss Schwarz_Data`).
    pub fn find(path: &Path) -> Option<Self> {
        path.ancestors().take(4).find_map(Self::at)
    }

    pub fn cards(&self) -> PathBuf {
        self.root.join("CardContent").join("Cards")
    }

    pub fn decks(&self) -> PathBuf {
        self.root.join("Decks")
    }

    /// `<root>/<Game>_Data/StreamingAssets`, if present.
    pub fn streaming_assets(&self) -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = std::fs::read_dir(&self.root)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().ends_with("_Data"))
            .map(|p| p.join("StreamingAssets"))
            .filter(|p| p.is_dir())
            .collect();
        candidates.sort();
        candidates.into_iter().next()
    }

    pub fn common_effects(&self) -> Option<PathBuf> {
        let assets = self.streaming_assets()?;
        EFFECTS_FILES
            .iter()
            .map(|f| assets.join(f))
            .find(|p| p.is_file())
    }

    /// Foil artwork folder (older simulators); may not exist.
    pub fn alternate_artwork(&self) -> PathBuf {
        self.streaming_assets()
            .unwrap_or_else(|| self.root.clone())
            .join("AlternateArtwork")
    }

    /// Sleeve images the simulator offers; may not exist.
    pub fn sleeves(&self) -> PathBuf {
        self.streaming_assets()
            .unwrap_or_else(|| self.root.clone())
            .join("Sleeves")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_root_from_subfolders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("SimuWeiss");
        fs::create_dir_all(root.join("CardContent/Cards")).unwrap();
        fs::create_dir_all(root.join("Weiss Schwarz_Data/StreamingAssets")).unwrap();
        fs::write(
            root.join("Weiss Schwarz_Data/StreamingAssets/CommonEffects_USER_REFERENCE.txt"),
            "",
        )
        .unwrap();

        let layout = Layout::at(&root).unwrap();
        assert_eq!(
            Layout::find(&root.join("CardContent/Cards")),
            Some(layout.clone())
        );
        assert_eq!(
            Layout::find(&root.join("Weiss Schwarz_Data")),
            Some(layout.clone())
        );
        assert_eq!(Layout::find(&root.join("Decks")), Some(layout.clone()));
        assert_eq!(Layout::find(dir.path()), None);
        assert!(
            layout
                .common_effects()
                .unwrap()
                .ends_with("CommonEffects_USER_REFERENCE.txt")
        );
        assert_eq!(layout.decks(), root.join("Decks"));
    }
}
