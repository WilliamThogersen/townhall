//! The one folder townhall owns: `~/.townhall` (or `$TOWNHALL_HOME`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

const HOME_ENV: &str = "TOWNHALL_HOME";
const HOME_DIR: &str = ".townhall";
/// Townhall's own agent worktrees live under `<home>/worktrees/`.
pub const WORKTREES_DIR: &str = "worktrees";
/// Repos townhall cloned from a url live under `<home>/repos/`.
pub const REPOS_DIR: &str = "repos";
/// One cargo target folder per repo lives under `<home>/target/<key>`.
pub const CARGO_TARGETS_DIR: &str = "target";
const LOGS_DIR: &str = "logs";
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

    /// `$TOWNHALL_HOME` when set and non-empty, else `$HOME/.townhall`, made absolute so
    /// every path handed to an agent resolves from any folder.
    pub fn from_env() -> Result<Self> {
        Self::from_vars(std::env::var_os(HOME_ENV), std::env::var_os("HOME"))
    }

    fn from_vars(over: Option<OsString>, home: Option<OsString>) -> Result<Self> {
        let set = |v: Option<OsString>| v.filter(|v| !v.is_empty());
        let root = match (set(over), set(home)) {
            (Some(root), _) => PathBuf::from(root),
            (None, Some(home)) => PathBuf::from(home).join(HOME_DIR),
            (None, None) => {
                return Err(Error::InvalidInput(format!(
                    "neither {HOME_ENV} nor HOME is set"
                )))
            }
        };
        let absolute = std::path::absolute(&root)
            .map_err(|e| Error::io(format!("home {}", root.display()), e))?;
        Ok(Self::at(absolute))
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

    pub fn cargo_target_for(&self, key: &str) -> PathBuf {
        self.root.join(CARGO_TARGETS_DIR).join(key)
    }

    /// `<home>/repos`, created (0700) when missing: where `git clone` puts added remotes.
    pub fn repos_dir(&self) -> Result<PathBuf> {
        let dir = self.root.join(REPOS_DIR);
        self.create_dir(&dir)?;
        Ok(dir)
    }

    /// `<home>/logs`, created (0700) when missing: the JSONL files `townhall-daemon`'s
    /// `logging` writes.
    pub fn logs_dir(&self) -> Result<PathBuf> {
        let dir = self.root.join(LOGS_DIR);
        self.create_dir(&dir)?;
        Ok(dir)
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

    /// The cargo target folder every agent of this repo builds into (`CARGO_TARGET_DIR`).
    pub fn cargo_target_dir(&self) -> PathBuf {
        self.home.cargo_target_for(&self.key)
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
    fn the_home_is_always_absolute() {
        let relative = Home::from_vars(Some("rel/th".into()), Some("/u".into())).unwrap();
        assert!(relative.root().is_absolute(), "{:?}", relative.root());
        assert!(relative.root().ends_with("rel/th"));
        let default = Home::from_vars(Some("".into()), Some("/u".into())).unwrap();
        assert_eq!(default.root(), Path::new("/u/.townhall"));
        assert!(Home::from_vars(None, Some("".into())).is_err());
    }

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

    #[test]
    fn cargo_targets_are_per_repo_and_outside_the_worktrees() {
        let home = Home::at("/h");
        let (a, b) = (home.cargo_target_for("a-1"), home.cargo_target_for("b-2"));
        assert_eq!(a, Path::new("/h/target/a-1"));
        assert_ne!(a, b);
        assert!(!a.starts_with(home.worktrees_for("a-1")));
    }

    #[cfg(unix)]
    #[test]
    fn logs_dir_is_created_private_under_the_home() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let home = Home::at(tmp.path().join("h"));
        let logs = home.logs_dir().unwrap();
        assert_eq!(logs, tmp.path().join("h/logs"));
        let mode = std::fs::metadata(&logs).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        assert_eq!(home.logs_dir().unwrap(), logs, "idempotent");
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
