use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const FILE: &str = "drafts.json";

#[derive(Debug)]
pub struct Drafts {
    path: PathBuf,
    entries: BTreeMap<String, String>,
}

impl Drafts {
    pub fn open(home: &Path) -> Result<Self, String> {
        let path = home.join(FILE);
        let entries = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| format!("{} is not a drafts file: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        Ok(Self { path, entries })
    }

    pub fn get(&self, conversation_id: &str) -> &str {
        self.entries
            .get(conversation_id)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn set(&mut self, conversation_id: &str, text: &str) -> Result<(), String> {
        if text.is_empty() {
            if self.entries.remove(conversation_id).is_none() {
                return Ok(());
            }
        } else if self.entries.get(conversation_id).map(String::as_str) == Some(text) {
            return Ok(());
        } else {
            self.entries
                .insert(conversation_id.to_string(), text.to_string());
        }
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let json = serde_json::to_string(&self.entries)
            .map_err(|e| format!("cannot encode drafts: {e}"))?;
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

    #[test]
    fn a_missing_file_means_no_drafts() {
        let dir = tempfile::tempdir().unwrap();
        let drafts = Drafts::open(dir.path()).unwrap();
        assert_eq!(drafts.get("c1"), "");
    }

    #[test]
    fn drafts_survive_a_restart_per_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let mut drafts = Drafts::open(dir.path()).unwrap();
        drafts.set("c1", "first\nsecond").unwrap();
        drafts.set("c2", "日本語").unwrap();
        let reopened = Drafts::open(dir.path()).unwrap();
        assert_eq!(reopened.get("c1"), "first\nsecond");
        assert_eq!(reopened.get("c2"), "日本語");
        assert_eq!(reopened.get("c3"), "");
    }

    #[test]
    fn an_empty_draft_removes_the_entry_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut drafts = Drafts::open(dir.path()).unwrap();
        drafts.set("c1", "hello").unwrap();
        drafts.set("c1", "").unwrap();
        assert_eq!(Drafts::open(dir.path()).unwrap().get("c1"), "");
        let text = fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert_eq!(text, "{}");
    }

    #[test]
    fn setting_an_empty_draft_that_never_existed_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut drafts = Drafts::open(dir.path()).unwrap();
        drafts.set("c1", "").unwrap();
        assert!(!dir.path().join(FILE).exists());
    }

    #[test]
    fn the_home_folder_is_created_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("not").join("yet");
        let mut drafts = Drafts::open(&home).unwrap();
        drafts.set("c1", "x").unwrap();
        assert_eq!(Drafts::open(&home).unwrap().get("c1"), "x");
    }

    #[test]
    fn a_corrupt_file_is_an_error_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE), "not json").unwrap();
        let err = Drafts::open(dir.path()).unwrap_err();
        assert!(err.contains("drafts.json"), "{err}");
    }
}
