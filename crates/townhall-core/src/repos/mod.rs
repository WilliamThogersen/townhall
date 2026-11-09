//! The repo registry: the `repos` table plus the git checks around it.
//! Removing a repo only unregisters it: the folder, its worktrees and branches stay.

pub mod accounts;
mod auth_env;
mod clone;
mod discard;
mod git;
pub mod names;
pub mod publish;
pub mod source;

use std::path::{Path, PathBuf};

pub(crate) use auth_env::remote_env;
use clone::clone_or_reuse;
use source::{Remote, Source};

pub use git::{
    check_top_folder, detect_base_branch, init_plain, is_plain_folder, verify_branch,
    DEFAULT_BRANCH,
};

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
        github_account: None,
        gitlab_account: None,
    })
}

/// `git init` for a plain folder, once nothing else about the request is known to be wrong
/// (the name is free), so a refused request leaves the folder as it was.
fn init_folder(known: &[Repo], path: &Path, name: &str, base: Option<&str>) -> Result<()> {
    if names::taken(name, known) {
        return Err(Error::Conflict(format!(
            "a repo named `{name}` already exists"
        )));
    }
    let branch = base
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .unwrap_or(DEFAULT_BRANCH);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    init_plain(path, branch, home.as_deref())
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

    /// `POST /api/repos`: a folder (`git init` first when it is in no repo) or a git link
    /// (cloned), a free name (`409` when taken, ignoring case) and a base branch that resolves.
    pub async fn add(&self, req: &AddRepo) -> Result<Repo> {
        match source::classify(&req.path)? {
            Source::Path(input) => self.add_folder(&input, req).await,
            Source::Remote(remote) => self.add_remote(&remote, req).await,
        }
    }

    /// A link: cloned into `<home>/repos` (or the clone there is reused), then registered
    /// like any folder. Without a name the repo is called after the link, made unique.
    async fn add_remote(&self, remote: &Remote, req: &AddRepo) -> Result<Repo> {
        let known = self.list().await?;
        let name = match req.name.as_deref() {
            None => names::unique(&remote.name, &known),
            Some(_) => chosen_name(req.name.as_deref(), Path::new(&remote.name))?,
        };
        if names::taken(&name, &known) {
            return Err(Error::Conflict(format!(
                "a repo named `{name}` already exists"
            )));
        }
        let (remote, base) = (remote.clone(), req.base_branch.clone());
        let home = self.store.home().clone();
        let repo = blocking(move || {
            let path = clone_or_reuse(&home, &remote)?;
            inspect(&home, &path, name, base.as_deref())
        })
        .await?;
        self.store.add_repo(repo).await
    }

    async fn add_folder(&self, input: &str, req: &AddRepo) -> Result<Repo> {
        let path = resolve_path(input)?;
        let name = chosen_name(req.name.as_deref(), &path)?;
        let known = self.list().await?;
        let (base, home) = (req.base_branch.clone(), self.store.home().clone());
        let repo = blocking(move || {
            if is_plain_folder(&path)? {
                init_folder(&known, &path, &name, base.as_deref())?;
            }
            inspect(&home, &path, name, base.as_deref())
        })
        .await?;
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

    /// Refuses unless `name` is a clean clone townhall made (see `discard::check_deletable`).
    pub async fn check_deletable(&self, name: &str) -> Result<()> {
        let repo = self.get(name).await?;
        let home = self.store.home().clone();
        blocking(move || discard::check_deletable(&home, &repo)).await
    }

    /// Unregisters `name` and deletes its clone in `<home>/repos`. Every check runs first, so a
    /// refusal leaves the repo registered and the folder untouched.
    pub async fn remove_with_clone(&self, name: &str) -> Result<Repo> {
        self.check_deletable(name).await?;
        let repo = self.store.remove_repo(name).await?;
        let (home, gone) = (self.store.home().clone(), repo.clone());
        blocking(move || discard::delete_clone(&home, &gone)).await?;
        Ok(repo)
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
