use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const FILE: &str = "prefs.json";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    #[serde(default)]
    sidebar_by_repo: bool,
}

#[derive(Debug)]
pub struct Prefs {
    path: PathBuf,
    by_repo: bool,
}

impl Prefs {
    pub fn open(home: &Path) -> Result<Self, String> {
        let path = home.join(FILE);
        let stored = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<Stored>(&text)
                .map_err(|e| format!("{} is not a prefs file: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Stored::default(),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        Ok(Self {
            path,
            by_repo: stored.sidebar_by_repo,
        })
    }

    pub fn by_repo(&self) -> bool {
        self.by_repo
    }

    pub fn set_by_repo(&mut self, on: bool) -> Result<(), String> {
        self.by_repo = on;
        let json = serde_json::to_string(&Stored {
            sidebar_by_repo: on,
        })
        .map_err(|e| format!("cannot encode prefs: {e}"))?;
        let tmp = self.path.with_extension("json.tmp");
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        fs::write(&tmp, json).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .map_err(|e| format!("cannot replace {}: {e}", self.path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_missing_file_means_the_default_flat_list() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!Prefs::open(dir.path()).unwrap().by_repo());
    }

    #[test]
    fn the_by_repo_choice_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut prefs = Prefs::open(dir.path()).unwrap();
        prefs.set_by_repo(true).unwrap();
        assert!(Prefs::open(dir.path()).unwrap().by_repo());
        let mut again = Prefs::open(dir.path()).unwrap();
        again.set_by_repo(false).unwrap();
        assert!(!Prefs::open(dir.path()).unwrap().by_repo());
    }

    #[test]
    fn the_home_folder_is_created_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("not").join("yet");
        Prefs::open(&home).unwrap().set_by_repo(true).unwrap();
        assert!(Prefs::open(&home).unwrap().by_repo());
    }

    #[test]
    fn a_corrupt_file_is_an_error_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("prefs.json"), "nope").unwrap();
        let err = Prefs::open(dir.path()).unwrap_err();
        assert!(err.contains("prefs.json"), "{err}");
    }
}
