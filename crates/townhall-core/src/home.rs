//! The one folder townhall owns: `~/.townhall` (or `$TOWNHALL_HOME`).

use std::path::{Path, PathBuf};

use crate::{Error, Result};

const HOME_ENV: &str = "TOWNHALL_HOME";
const HOME_DIR: &str = ".townhall";
/// Townhall's own agent worktrees live under `<home>/worktrees/`.
pub const WORKTREES_DIR: &str = "worktrees";
const CONFIG_FILE: &str = "config.json";
const MAX_BASE_LEN: usize = 32;

/// Root folder for everything townhall creates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$TOWNHALL_HOME` when set and non-empty, else `$HOME/.townhall`.
    pub fn from_env() -> Result<Self> {
        let over = std::env::var_os(HOME_ENV).filter(|v| !v.is_empty());
        if let Some(root) = over {
            return Ok(Self::at(root));
        }
        let home = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| Error::InvalidInput(format!("neither {HOME_ENV} nor HOME is set")))?;
        Ok(Self::at(PathBuf::from(home).join(HOME_DIR)))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Phase 1's `config.json`: only read, once, by the store's import, which then renames it.
    pub fn config_file(&self) -> PathBuf {
        self.root.join(CONFIG_FILE)
    }

    pub fn worktrees_for(&self, key: &str) -> PathBuf {
        self.root.join(WORKTREES_DIR).join(key)
    }

    /// Creates `dir` and missing parents; new directories are 0700 on unix.
    pub fn create_dir(&self, dir: &Path) -> Result<()> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(dir).map_err(|source| Error::Io {
            context: format!("create {}", dir.display()),
            source,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        self.create_dir(&self.root)
    }
}

/// A repo inside a home: the pair every agent operation needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    home: Home,
    repo: PathBuf,
    key: String,
}

impl Workspace {
    /// Canonicalizes `repo` and derives its key.
    pub fn new(home: Home, repo: &Path) -> Result<Self> {
        let repo = repo.canonicalize().map_err(|source| Error::Io {
            context: format!("repo path {}", repo.display()),
            source,
        })?;
        let key = repo_key(&repo);
        Ok(Self { home, repo, key })
    }

    pub fn home(&self) -> &Home {
        &self.home
    }

    pub fn repo(&self) -> &Path {
        &self.repo
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn worktrees_dir(&self) -> PathBuf {
        self.home.worktrees_for(&self.key)
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Stable, filesystem and tmux safe key: `<folder-name>-<8 hex of path hash>`.
/// Expects a canonical path so the same repo always yields the same key.
pub fn repo_key(canonical_repo: &Path) -> String {
    let base: String = canonical_repo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let base = base.trim_matches('-');
    let base: String = base.chars().take(MAX_BASE_LEN).collect();
    let base = if base.is_empty() { "repo" } else { &base };
    let hash = fnv1a(canonical_repo.to_string_lossy().as_bytes()) as u32;
    format!("{base}-{hash:08x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_stable_safe_and_distinct() {
        let a = repo_key(Path::new("/x/my.repo"));
        assert_eq!(a, repo_key(Path::new("/x/my.repo")));
        assert!(a.starts_with("my-repo-"), "{a}");
        assert!(a
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(repo_key(Path::new("/a/app")), repo_key(Path::new("/b/app")));
    }

    #[test]
    fn key_handles_root_and_long_names() {
        assert!(repo_key(Path::new("/")).starts_with("repo-"));
        let long = format!("/x/{}", "a".repeat(100));
        assert_eq!(repo_key(Path::new(&long)).len(), MAX_BASE_LEN + 9);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_creates_private_dir() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let home = Home::at(tmp.path().join("h"));
        home.ensure().unwrap();
        let mode = std::fs::metadata(home.root()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }
}
