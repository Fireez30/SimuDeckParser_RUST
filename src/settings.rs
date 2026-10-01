//! Tiny `key:value` settings file at `~/.config/weiss_simulator_parser/settings.conf`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const SIMULATOR_PATH: &str = "simulator_data_path";
/// Name of the linked Encore Decks account. Its session is in the system credential store.
pub const ENCORE_USER: &str = "encoredecks_user";

pub struct Settings {
    path: PathBuf,
}

impl Settings {
    pub fn open_default() -> Self {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("weiss_simulator_parser");
        Self::at(dir.join("settings.conf"))
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn all(&self) -> BTreeMap<String, String> {
        let Ok(content) = fs::read_to_string(&self.path) else {
            return BTreeMap::new();
        };
        content
            .lines()
            .filter_map(|line| line.trim().split_once(':'))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.all().remove(key).filter(|v| !v.is_empty())
    }

    pub fn set(&self, key: &str, value: &str) -> std::io::Result<()> {
        let mut all = self.all();
        all.insert(key.to_string(), value.to_string());
        self.write(&all)
    }

    pub fn delete(&self, key: &str) -> std::io::Result<()> {
        let mut all = self.all();
        if all.remove(key).is_some() {
            self.write(&all)?;
        }
        Ok(())
    }

    fn write(&self, all: &BTreeMap<String, String>) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let content: String = all.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
        fs::write(&self.path, content)?;
        // Personal settings: readable by their owner only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings::at(dir.path().join("sub/settings.conf"));
        assert_eq!(s.get(SIMULATOR_PATH), None);
        s.delete("missing").unwrap();

        s.set(SIMULATOR_PATH, "/games/ws:1/Linux_Data/").unwrap();
        s.set("other", "x").unwrap();
        assert_eq!(
            s.get(SIMULATOR_PATH).as_deref(),
            Some("/games/ws:1/Linux_Data/")
        );
        assert_eq!(
            fs::read_to_string(s.path()).unwrap(),
            "other:x\nsimulator_data_path:/games/ws:1/Linux_Data/\n"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(s.path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        s.delete(SIMULATOR_PATH).unwrap();
        assert_eq!(s.get(SIMULATOR_PATH), None);
        assert_eq!(s.get("other").as_deref(), Some("x"));
    }
}
