use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use townhall_core::Home;

const LEGACY_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub repo: Option<PathBuf>,
}

fn read(path: &Path) -> Result<Option<Settings>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("corrupt settings file {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Loads `<home>/config.json`. A missing file falls back once to the old
/// `<legacy_dir>/settings.json`, which is then migrated. A corrupt file is an error.
pub fn load(home: &Home, legacy_dir: Option<&Path>) -> Result<Settings, String> {
    if let Some(found) = read(&home.config_file())? {
        return Ok(found);
    }
    let legacy = match legacy_dir {
        Some(dir) => read(&dir.join(LEGACY_FILE_NAME))?,
        None => None,
    };
    match legacy {
        Some(old) => {
            save(home, &old)?;
            Ok(old)
        }
        None => Ok(Settings::default()),
    }
}

/// Writes settings atomically (temp file then rename).
pub fn save(home: &Home, settings: &Settings) -> Result<(), String> {
    home.ensure().map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("serialize: {e}"))?;
    let dest = home.config_file();
    let tmp = dest.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &dest).map_err(|e| format!("cannot replace {}: {e}", dest.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(dir: &tempfile::TempDir) -> Home {
        Home::at(dir.path().join(".townhall"))
    }

    #[test]
    fn missing_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&home(&dir), None).unwrap(), Settings::default());
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings {
            repo: Some(PathBuf::from("/x/y")),
        };
        save(&home(&dir), &s).unwrap();
        assert_eq!(load(&home(&dir), None).unwrap(), s);
        assert!(home(&dir).config_file().exists());
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let h = home(&dir);
        h.ensure().unwrap();
        fs::write(h.config_file(), "{nope").unwrap();
        assert!(load(&h, None).unwrap_err().contains("corrupt"));
    }

    #[test]
    fn legacy_file_is_migrated_once() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = tempfile::tempdir().unwrap();
        fs::write(
            legacy.path().join(LEGACY_FILE_NAME),
            r#"{"repo":"/old/repo"}"#,
        )
        .unwrap();
        let h = home(&dir);
        let got = load(&h, Some(legacy.path())).unwrap();
        assert_eq!(got.repo, Some(PathBuf::from("/old/repo")));
        assert!(h.config_file().exists());
        fs::remove_file(legacy.path().join(LEGACY_FILE_NAME)).unwrap();
        assert_eq!(load(&h, Some(legacy.path())).unwrap(), got);
    }

    #[test]
    fn corrupt_legacy_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = tempfile::tempdir().unwrap();
        fs::write(legacy.path().join(LEGACY_FILE_NAME), "{nope").unwrap();
        let err = load(&home(&dir), Some(legacy.path())).unwrap_err();
        assert!(err.contains("corrupt"), "{err}");
    }
}
