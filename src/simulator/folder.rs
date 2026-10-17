//! Where things live in a simulator install, and finding installs on this computer.

use std::path::{Path, PathBuf};

/// Names of the common effects file, newest first (it was renamed by a simulator update).
const COMMON_EFFECTS_FILES: [&str; 2] = [
    "CommonEffects_USER_REFERENCE.txt",
    "CommonEffects(copy).txt",
];

/// The root folder of a simulator install: the one that contains `CardContent` and `Decks`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatorFolder {
    pub root: PathBuf,
}

impl SimulatorFolder {
    /// `root` if it is a simulator folder, `None` otherwise.
    pub fn at(root: &Path) -> Option<Self> {
        if root.join("CardContent").join("Cards").is_dir() {
            Some(Self {
                root: root.to_path_buf(),
            })
        } else {
            None
        }
    }

    /// The simulator folder containing `path`: users often pick a subfolder such as `Decks`,
    /// `CardContent` or `Weiss Schwarz_Data`, so the parents are tried too.
    pub fn find(path: &Path) -> Option<Self> {
        path.ancestors().take(4).find_map(Self::at)
    }

    pub fn cards(&self) -> PathBuf {
        self.root.join("CardContent").join("Cards")
    }

    pub fn decks(&self) -> PathBuf {
        self.root.join("Decks")
    }

    /// Sleeve images the simulator offers; may not exist yet.
    pub fn sleeves(&self) -> PathBuf {
        self.root.join("Sleeves")
    }

    /// `<root>/<Game>_Data/StreamingAssets`, if present. Unity games keep their data files in
    /// a folder named after the game, so the name is not fixed.
    pub fn streaming_assets(&self) -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = std::fs::read_dir(&self.root)
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.to_string_lossy().ends_with("_Data"))
            .map(|path| path.join("StreamingAssets"))
            .filter(|path| path.is_dir())
            .collect();
        candidates.sort();
        candidates.into_iter().next()
    }

    pub fn common_effects(&self) -> Option<PathBuf> {
        let assets = self.streaming_assets()?;
        COMMON_EFFECTS_FILES
            .iter()
            .map(|file| assets.join(file))
            .find(|path| path.is_file())
    }

    /// Foil artwork folder of older simulators; may not exist.
    pub fn alternate_artwork(&self) -> PathBuf {
        self.streaming_assets()
            .unwrap_or_else(|| self.root.clone())
            .join("AlternateArtwork")
    }
}

/// Looks for simulator installs in the usual places: folders under the home folder whose names
/// suggest games, and Steam libraries. Returns at most 6, sorted. Reads many folders: run it off
/// the UI thread.
pub fn detect_installs() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Some(home) = dirs::home_dir() {
        search(&home, 0, &mut found);
        for steam in [".local/share/Steam", ".steam/steam"] {
            search(&home.join(steam).join("steamapps/common"), 2, &mut found);
        }
    }
    if cfg!(windows) {
        for steam in [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"] {
            let common = Path::new(steam).join("steamapps").join("common");
            search(&common, 2, &mut found);
        }
    }
    for path in &mut found {
        *path = path.canonicalize().unwrap_or_else(|_| path.clone());
    }
    found.sort();
    found.dedup();
    found.truncate(6);
    found
}

/// Adds the simulator folders under `dir` to `found`. Below the first level, only folders whose
/// name suggests games are opened, and the search stops 3 levels down.
fn search(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    const HINTS: [&str; 8] = [
        "weiss", "schwarz", "simu", "game", "jeu", "steam", "ws", "share",
    ];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.starts_with('.') && name != ".local" {
            continue;
        }
        if SimulatorFolder::at(&path).is_some() {
            found.push(path);
        } else if depth < 3 && (depth == 0 || HINTS.iter().any(|hint| name.contains(hint))) {
            search(&path, depth + 1, found);
        }
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

        let folder = SimulatorFolder::at(&root).unwrap();
        assert_eq!(
            SimulatorFolder::find(&root.join("CardContent/Cards")),
            Some(folder.clone())
        );
        assert_eq!(
            SimulatorFolder::find(&root.join("Weiss Schwarz_Data")),
            Some(folder.clone())
        );
        assert_eq!(
            SimulatorFolder::find(&root.join("Decks")),
            Some(folder.clone())
        );
        assert_eq!(SimulatorFolder::find(dir.path()), None);
        assert!(
            folder
                .common_effects()
                .unwrap()
                .ends_with("CommonEffects_USER_REFERENCE.txt")
        );
        assert_eq!(folder.decks(), root.join("Decks"));
    }

    #[test]
    fn searches_game_folders() {
        let dir = tempfile::tempdir().unwrap();
        let wanted = dir.path().join("Games/SimuWeiss");
        fs::create_dir_all(wanted.join("CardContent/Cards")).unwrap();
        // Too deep inside a folder whose name says nothing about games.
        fs::create_dir_all(dir.path().join("Documents/a/b/Simu/CardContent/Cards")).unwrap();
        let mut found = Vec::new();
        search(dir.path(), 0, &mut found);
        assert_eq!(found, [wanted]);
    }
}
