//! The repo registry: the `repos` table plus the git checks around it.
//! Removing a repo only unregisters it: the folder, its worktrees and branches stay.

mod git;
pub mod names;

use std::path::{Path, PathBuf};

pub use git::{check_top_folder, detect_base_branch, verify_branch};

use crate::home::WORKTREES_DIR;
use crate::model::{AddRepo, Repo};
use crate::store::Store;
use crate::{Error, Home, Result};

/// A typed path: trimmed, `~` expanded, absolute, existing, canonical, a folder.
pub fn resolve_path(input: &str) -> Result<PathBuf> {
    let input = input.trim();
    if input.is_empty() {
        return Err(Error::InvalidInput("path is required".into()));
    }
    let expanded = expand_tilde(input);
    if !expanded.is_absolute() {
        return Err(Error::InvalidInput(format!(
            "path must be absolute: {input}"
        )));
    }
    let canonical = expanded
        .canonicalize()
        .map_err(|e| Error::InvalidInput(format!("cannot open {input}: {e}")))?;
    if !canonical.is_dir() {
        return Err(Error::InvalidInput(format!(
            "{} is not a folder",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn expand_tilde(input: &str) -> PathBuf {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty());
    match (input.strip_prefix('~'), home) {
        (Some(""), Some(home)) => PathBuf::from(home),
        (Some(rest), Some(home)) if rest.starts_with('/') => {
            PathBuf::from(home).join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(input),
    }
}

/// The entry for a canonical repo folder: checks it is the top folder of a git repo and
/// not one of townhall's worktrees, then checks or detects the base branch. Blocking (git).
pub fn inspect(home: &Home, path: &Path, name: String, base_branch: Option<&str>) -> Result<Repo> {
    check_top_folder(path, &home.root().join(WORKTREES_DIR))?;
    let base_branch = match base_branch.map(str::trim).filter(|b| !b.is_empty()) {
        Some(given) => {
            verify_branch(path, given)?;
            given.to_string()
        }
        None => detect_base_branch(path),
    };
    Ok(Repo {
        name,
        path: path.to_path_buf(),
        base_branch,
    })
}

/// A name someone typed: trimmed, never blank, `[A-Za-z0-9._-]`. Missing means the folder name.
pub fn chosen_name(given: Option<&str>, path: &Path) -> Result<String> {
    match given.map(str::trim) {
        None => Ok(names::from_folder(path)),
        Some("") => Err(Error::InvalidInput("repo name must not be blank".into())),
        Some(name) => names::validate(name).map(|()| name.to_string()),
    }
}

async fn blocking<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Error::InvalidInput(format!("git check failed: {e}")))?
}

#[derive(Debug, Clone)]
pub struct RepoRegistry {
    store: Store,
}

impl RepoRegistry {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// Every registered repo, in the order they were added.
    pub async fn list(&self) -> Result<Vec<Repo>> {
        self.store.repos().await
    }

    /// The repo called `name`, ignoring case.
    pub async fn get(&self, name: &str) -> Result<Repo> {
        self.list()
            .await?
            .into_iter()
            .find(|r| names::same(&r.name, name))
            .ok_or_else(|| Error::missing("repo", name))
    }

    /// `POST /api/repos`: a git folder, a free name (`409` when taken, ignoring case) and a
    /// base branch that resolves.
    pub async fn add(&self, req: &AddRepo) -> Result<Repo> {
        let path = resolve_path(&req.path)?;
        let name = chosen_name(req.name.as_deref(), &path)?;
        let (base, home) = (req.base_branch.clone(), self.store.home().clone());
        let repo = blocking(move || inspect(&home, &path, name, base.as_deref())).await?;
        self.store.add_repo(repo).await
    }

    /// The entry for `path`, registering it under a free name when it is missing.
    pub async fn ensure(&self, path: &Path) -> Result<Repo> {
        let path = path
            .canonicalize()
            .map_err(|e| Error::io(format!("repo path {}", path.display()), e))?;
        if let Some(found) = self.list().await?.into_iter().find(|r| r.path == path) {
            return Ok(found);
        }
        let home = self.store.home().clone();
        let repo = blocking(move || inspect(&home, &path, names::from_folder(&path), None)).await?;
        self.store.ensure_repo(repo).await
    }

    /// Unregisters `name`. Never touches the folder, its worktrees or its branches.
    pub async fn remove(&self, name: &str) -> Result<Repo> {
        self.store.remove_repo(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_never_blank() {
        let path = Path::new("/x/shop");
        assert_eq!(chosen_name(None, path).unwrap(), "shop");
        assert_eq!(chosen_name(Some(" store "), path).unwrap(), "store");
        for blank in ["", "   "] {
            assert!(matches!(
                chosen_name(Some(blank), path),
                Err(Error::InvalidInput(_))
            ));
        }
        assert!(chosen_name(Some("a b"), path).is_err());
    }

    #[test]
    fn tilde_expands_to_home() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_tilde("~"), PathBuf::from(&home));
        assert_eq!(expand_tilde("~/x"), PathBuf::from(&home).join("x"));
        assert_eq!(expand_tilde("~x"), PathBuf::from("~x"));
    }
}
