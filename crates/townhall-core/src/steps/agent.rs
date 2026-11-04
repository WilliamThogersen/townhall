//! Where Claude keeps transcripts, and when an agent's worktree was made: a transcript last
//! written before then belongs to an earlier agent with the same name.

use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{Error, Result};

/// Points the daemon at another claude projects folder (tests use a temp one).
pub const CLAUDE_PROJECTS_ENV: &str = "TOWNHALL_CLAUDE_PROJECTS";

/// `$TOWNHALL_CLAUDE_PROJECTS` when set and not empty, else `$HOME/.claude/projects`.
pub fn claude_projects_dir() -> Result<PathBuf> {
    projects_dir_from(
        std::env::var_os(CLAUDE_PROJECTS_ENV),
        std::env::var_os("HOME"),
    )
}

fn projects_dir_from(over: Option<OsString>, home: Option<OsString>) -> Result<PathBuf> {
    if let Some(dir) = over.filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    home.filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".claude").join("projects"))
        .ok_or_else(|| {
            let why = format!("neither {CLAUDE_PROJECTS_ENV} nor HOME is set");
            Error::io(
                "find claude's transcripts",
                io::Error::new(ErrorKind::NotFound, why),
            )
        })
}

/// When the worktree at `path` was made, `None` when the folder is gone. That is the folder's
/// birth time, or where the platform keeps none, the modified time of the worktree's `.git`
/// file, which `git worktree add` writes once. Not the folder's own modified time: that moves
/// whenever a file is added or removed at the top of the worktree, so it would hide a live
/// agent's steps while the agent is idle.
pub fn worktree_made(path: &Path) -> Result<Option<SystemTime>> {
    match std::fs::metadata(path) {
        Ok(meta) => made_from(meta.created(), path).map(Some),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(format!("read worktree {}", path.display()), e)),
    }
}

fn made_from(born: io::Result<SystemTime>, path: &Path) -> Result<SystemTime> {
    born.or_else(|_| std::fs::metadata(path.join(".git")).and_then(|m| m.modified()))
        .map_err(|e| Error::io(format!("read when worktree {} was made", path.display()), e))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_override_wins_then_home() {
        let dir = |over: Option<&str>, home: Option<&str>| {
            projects_dir_from(over.map(OsString::from), home.map(OsString::from))
        };
        assert_eq!(
            dir(Some("/tmp/p"), Some("/home/me")).unwrap(),
            PathBuf::from("/tmp/p")
        );
        let from_home = PathBuf::from("/home/me/.claude/projects");
        assert_eq!(dir(Some(""), Some("/home/me")).unwrap(), from_home);
        assert_eq!(dir(None, Some("/home/me")).unwrap(), from_home);
        let err = dir(None, Some("")).unwrap_err().to_string();
        assert!(err.contains(CLAUDE_PROJECTS_ENV), "{err}");
        assert!(dir(None, None).is_err());
    }

    #[test]
    fn without_a_birth_time_the_git_file_dates_the_worktree() {
        let root = tempfile::tempdir().unwrap();
        let git = root.path().join(".git");
        std::fs::write(&git, "gitdir: /x\n").unwrap();
        let written = SystemTime::now() - Duration::from_secs(90);
        let file = std::fs::File::options().write(true).open(&git).unwrap();
        file.set_modified(written).unwrap();
        let unsupported = || Err(io::Error::from(ErrorKind::Unsupported));
        assert_eq!(made_from(unsupported(), root.path()).unwrap(), written);
        let born = SystemTime::now();
        assert_eq!(made_from(Ok(born), root.path()).unwrap(), born);
        std::fs::remove_file(&git).unwrap();
        let err = made_from(unsupported(), root.path()).unwrap_err();
        assert!(err.to_string().contains("was made"), "{err}");
    }
}
