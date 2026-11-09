//! The one-time import of phase 1's `~/.townhall/config.json` (`{ "repo": "<path>" }`):
//! the repo goes into `repos` (when it is still a git repo) and becomes the desktop app's
//! repo, and the file is renamed to `config.json.imported`. Never deleted; existing rows are
//! never overridden. Only `repo` is read: settings change only
//! through `PUT /api/settings/{key}`, never through a file.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::Value;

use super::{repos, settings};
use crate::model::Repo;
use crate::repos::{inspect, names};
use crate::{Error, Home, Result};

pub const IMPORTED_SUFFIX: &str = ".imported";

fn importable(home: &Home, path: &Path) -> Result<Repo> {
    let canonical = path
        .canonicalize()
        .map_err(|e| Error::io(format!("repo path {}", path.display()), e))?;
    inspect(home, &canonical, names::from_folder(&canonical), None)
}

/// `config.json.imported`, or `config.json.imported-2` ... when that name is taken.
fn imported_path(path: &Path) -> PathBuf {
    let base = format!("{}{IMPORTED_SUFFIX}", path.display());
    (1..)
        .map(|n| match n {
            1 => PathBuf::from(&base),
            _ => PathBuf::from(format!("{base}-{n}")),
        })
        .find(|p| !p.exists())
        .unwrap_or_else(|| PathBuf::from(base))
}

/// True when a file was imported.
pub(super) fn config_json(conn: &mut Connection, home: &Home) -> Result<bool> {
    let path = home.config_file();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(Error::io(format!("read {}", path.display()), e)),
    };
    let raw: Value = serde_json::from_str(&text).map_err(|e| Error::Corrupt {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    let saved_repo = raw
        .get("repo")
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty());
    let repo = saved_repo.and_then(|p| match importable(home, Path::new(p)) {
        Ok(repo) => Some(repo),
        Err(e) => {
            tracing::warn!(
                target: "townhall::store", event = "store.import_repo_skipped", path = %p,
                error_kind = e.kind(), error = %crate::redact::redact(&e.to_string()),
                "saved repo not registered"
            );
            None
        }
    });
    let tx = conn.transaction()?;
    if let Some(repo) = repo {
        repos::ensure(&tx, repo)?;
    }
    if let Some(p) = saved_repo {
        settings::put_missing(&tx, settings::DESKTOP_REPO, &Value::String(p.into()))?;
    }
    tx.commit()?;
    let dest = imported_path(&path);
    fs::rename(&path, &dest).map_err(|e| Error::io(format!("rename {}", path.display()), e))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_names_never_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        assert_eq!(
            imported_path(&path),
            dir.path().join("config.json.imported")
        );
        fs::write(dir.path().join("config.json.imported"), "{}").unwrap();
        assert_eq!(
            imported_path(&path),
            dir.path().join("config.json.imported-2")
        );
    }
}
