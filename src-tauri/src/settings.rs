//! The desktop app's chosen repo (its in-process daemon's startup repo), kept in the store.
//! Phase 1's `config.json` is imported by the store itself; the even older
//! `<app config dir>/settings.json` is imported here, once, when the store has no repo yet.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use townhall_core::store::Store;

const LEGACY_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    pub repo: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct Legacy {
    repo: Option<PathBuf>,
}

fn read_legacy(path: &Path) -> Result<Option<Legacy>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("corrupt settings file {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// The saved repo. Without one, an old `<legacy_dir>/settings.json` is imported once.
/// A corrupt legacy file is an error.
pub async fn load(store: &Store, legacy_dir: Option<&Path>) -> Result<Settings, String> {
    if let Some(repo) = store.desktop_repo().await.map_err(|e| e.to_string())? {
        return Ok(Settings { repo: Some(repo) });
    }
    let legacy = match legacy_dir {
        Some(dir) => read_legacy(&dir.join(LEGACY_FILE_NAME))?,
        None => None,
    };
    let migrated = Settings {
        repo: legacy.and_then(|l| l.repo),
    };
    save(store, &migrated).await?;
    Ok(migrated)
}

/// Saves the chosen repo. Nothing to save when there is none.
pub async fn save(store: &Store, settings: &Settings) -> Result<(), String> {
    match &settings.repo {
        Some(repo) => store
            .set_desktop_repo(repo)
            .await
            .map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use townhall_core::events::EventBus;
    use townhall_core::Home;

    fn store(dir: &tempfile::TempDir) -> Store {
        Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap()
    }

    #[tokio::test]
    async fn missing_is_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&store(&dir), None).await.unwrap(), Settings::default());
    }

    #[tokio::test]
    async fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings {
            repo: Some(PathBuf::from("/x/y")),
        };
        let st = store(&dir);
        save(&st, &s).await.unwrap();
        assert_eq!(load(&st, None).await.unwrap(), s);
        assert!(!dir.path().join(".townhall/config.json").exists());
    }

    #[tokio::test]
    async fn legacy_file_is_imported_once() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = tempfile::tempdir().unwrap();
        let file = legacy.path().join(LEGACY_FILE_NAME);
        fs::write(&file, r#"{"repo":"/old/repo"}"#).unwrap();
        let st = store(&dir);
        let got = load(&st, Some(legacy.path())).await.unwrap();
        assert_eq!(got.repo, Some(PathBuf::from("/old/repo")));
        fs::write(&file, r#"{"repo":"/newer/repo"}"#).unwrap();
        assert_eq!(load(&st, Some(legacy.path())).await.unwrap(), got);
        assert!(file.exists(), "the old file is never deleted");
    }

    #[tokio::test]
    async fn corrupt_legacy_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = tempfile::tempdir().unwrap();
        fs::write(legacy.path().join(LEGACY_FILE_NAME), "{nope").unwrap();
        let err = load(&store(&dir), Some(legacy.path())).await.unwrap_err();
        assert!(err.contains("corrupt"), "{err}");
    }
}
