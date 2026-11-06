//! The shared cargo target folder of a repo (`Workspace::cargo_target_dir`). Worktrees
//! reuse what is compiled there, so it only ever grows; above the cap it is trimmed, never
//! while an agent of the repo may be building into it.
//!
//! The incremental caches go first: they are the biggest part and the cheapest to rebuild,
//! and the compiled dependencies stay. Only a folder that is still over the cap is emptied.

use std::path::{Path, PathBuf};

use crate::{Error, Result, Workspace};

const GB: u64 = 1_000_000_000;

/// What a trim did.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Trim {
    /// Nothing to do: no cap, no folder, or the folder is within the cap.
    Within,
    Trimmed,
    /// Over the cap, but an agent of the repo is running, so it may be building.
    Busy(u64),
}

/// The size of everything under `dir` in bytes. Symlinks are counted, never followed.
fn size_of(dir: &Path) -> Result<u64> {
    let mut total = 0;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let read = |e| Error::io(format!("read {}", folder.display()), e);
        for entry in std::fs::read_dir(&folder).map_err(read)? {
            let entry = entry.map_err(read)?;
            let meta = entry
                .metadata()
                .map_err(|e| Error::io(format!("stat {}", entry.path().display()), e))?;
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    Ok(total)
}

fn remove(path: &Path) -> Result<()> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| Error::io(format!("stat {}", path.display()), e))?;
    let removed = match meta.is_dir() {
        true => std::fs::remove_dir_all(path),
        false => std::fs::remove_file(path),
    };
    removed.map_err(|e| Error::io(format!("remove {}", path.display()), e))
}

/// `<target>/<profile>/incremental` of every profile.
fn incremental_dirs(target: &Path) -> Result<Vec<PathBuf>> {
    let read = |e| Error::io(format!("read {}", target.display()), e);
    let mut found = vec![];
    for profile in std::fs::read_dir(target).map_err(read)? {
        let cache = profile.map_err(read)?.path().join("incremental");
        if cache.is_dir() {
            found.push(cache);
        }
    }
    Ok(found)
}

fn trim_blocking(target: &Path, max_gb: u32, busy: bool) -> Result<Trim> {
    let cap = u64::from(max_gb) * GB;
    if max_gb == 0 || !target.is_dir() {
        return Ok(Trim::Within);
    }
    let size = size_of(target)?;
    if size <= cap {
        return Ok(Trim::Within);
    }
    if busy {
        return Ok(Trim::Busy(size));
    }
    for cache in incremental_dirs(target)? {
        remove(&cache)?;
    }
    if size_of(target)? <= cap {
        return Ok(Trim::Trimmed);
    }
    let read = |e| Error::io(format!("read {}", target.display()), e);
    for entry in std::fs::read_dir(target).map_err(read)? {
        remove(&entry.map_err(read)?.path())?;
    }
    Ok(Trim::Trimmed)
}

/// Trims the repo's shared target folder to `max_gb` (`0`: no cap). `busy` is whether an
/// agent of the repo runs.
pub(super) async fn trim(ws: &Workspace, max_gb: u32, busy: bool) -> Result<Trim> {
    let target = ws.cargo_target_dir();
    tokio::task::spawn_blocking(move || trim_blocking(&target, max_gb, busy))
        .await
        .map_err(|e| Error::io("trim the shared target", std::io::Error::other(e)))?
}

/// `3.2 GB`.
pub(super) fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / GB as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &Path, bytes: u64) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::File::create(path).unwrap().set_len(bytes).unwrap();
    }

    #[test]
    fn size_adds_every_file_and_ignores_symlink_targets() {
        let tmp = tempfile::tempdir().unwrap();
        file(&tmp.path().join("a/one"), 10);
        file(&tmp.path().join("a/b/two"), 5);
        #[cfg(unix)]
        std::os::unix::fs::symlink("/", tmp.path().join("link")).unwrap();
        let link = if cfg!(unix) {
            std::fs::symlink_metadata(tmp.path().join("link"))
                .unwrap()
                .len()
        } else {
            0
        };
        assert_eq!(size_of(tmp.path()).unwrap(), 15 + link);
    }

    #[test]
    fn a_missing_folder_or_no_cap_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("none");
        assert_eq!(trim_blocking(&missing, 1, false).unwrap(), Trim::Within);
        file(&tmp.path().join("big"), 5 * GB);
        assert_eq!(trim_blocking(tmp.path(), 0, false).unwrap(), Trim::Within);
        assert!(tmp.path().join("big").exists());
    }

    #[test]
    fn a_busy_repo_keeps_everything() {
        let tmp = tempfile::tempdir().unwrap();
        file(&tmp.path().join("debug/incremental/x"), 5 * GB);
        let busy = trim_blocking(tmp.path(), 1, true).unwrap();
        assert_eq!(busy, Trim::Busy(5 * GB));
        assert!(tmp.path().join("debug/incremental/x").exists());
    }

    #[test]
    fn the_size_reads_in_gigabytes() {
        assert_eq!(gb(3_200_000_000), "3.2 GB");
    }
}
