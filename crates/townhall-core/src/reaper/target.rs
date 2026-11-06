//! The shared cargo target folder of a repo (`Workspace::cargo_target_dir`). Worktrees
//! reuse what is compiled there, so it only ever grows; above the cap it is trimmed, never
//! while an agent of the repo may be building into it.
//!
//! The incremental caches go first: they are the biggest part and the cheapest to rebuild,
//! and the compiled dependencies stay. Only a folder that is still over the cap is emptied.
//! Deleting happens under cargo's own `.cargo-lock` of every profile, so a build that starts
//! meanwhile waits for it instead of losing its output.

use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::{Error, Result, Workspace};

const GB: u64 = 1_000_000_000;

/// Cargo's lock file in a profile folder, held for as long as a build runs there.
const LOCK: &str = ".cargo-lock";

/// What a trim did.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Trim {
    /// Nothing to do: no cap, no folder, or the folder is within the cap.
    Within,
    Trimmed,
    /// Over the cap, but a build may run: an agent of the repo is alive or a profile is locked.
    Busy(u64),
}

/// What a file takes on disk: its allocated blocks, a file with several links once.
#[cfg(unix)]
fn disk_use(meta: &std::fs::Metadata, seen: &mut HashSet<(u64, u64)>) -> u64 {
    use std::os::unix::fs::MetadataExt;
    if meta.nlink() > 1 && !seen.insert((meta.dev(), meta.ino())) {
        return 0;
    }
    meta.blocks() * 512
}

#[cfg(not(unix))]
fn disk_use(meta: &std::fs::Metadata, _seen: &mut HashSet<(u64, u64)>) -> u64 {
    meta.len()
}

/// The size of everything under `dir` in bytes. Symlinks are counted, never followed.
fn size_of(dir: &Path) -> Result<u64> {
    let mut total = 0;
    let mut seen = HashSet::new();
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
                total += disk_use(&meta, &mut seen);
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

/// Everything directly under `dir`, as paths; a symlink is a path like any other.
fn entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let read = |e| Error::io(format!("read {}", dir.display()), e);
    std::fs::read_dir(dir)
        .map_err(read)?
        .map(|e| e.map(|e| e.path()).map_err(read))
        .collect()
}

/// The real folders directly under `dir`; a symlink is never one.
fn folders(dir: &Path) -> Result<Vec<PathBuf>> {
    let read = |e| Error::io(format!("read {}", dir.display()), e);
    let mut found = vec![];
    for entry in std::fs::read_dir(dir).map_err(read)? {
        let entry = entry.map_err(read)?;
        let kind = entry
            .file_type()
            .map_err(|e| Error::io(format!("stat {}", entry.path().display()), e))?;
        if kind.is_dir() {
            found.push(entry.path());
        }
    }
    Ok(found)
}

/// Whether the exclusive lock was taken; `false` when another process holds it.
#[cfg(unix)]
fn try_lock(file: &File, path: &Path) -> Result<bool> {
    use std::os::fd::AsRawFd;
    // `flock` only reads the descriptor, which `file` keeps open for the call.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let cause = std::io::Error::last_os_error();
    match cause.kind() {
        std::io::ErrorKind::WouldBlock => Ok(false),
        _ => Err(Error::io(format!("lock {}", path.display()), cause)),
    }
}

#[cfg(not(unix))]
fn try_lock(_file: &File, path: &Path) -> Result<bool> {
    Err(Error::InvalidInput(format!(
        "{} is not locked on this platform",
        path.display()
    )))
}

/// Exclusive locks on the `.cargo-lock` of every profile folder (`<target>/<profile>` and
/// `<target>/<triple>/<profile>`), the lock a running cargo holds. While they are held no
/// build runs in those folders, and one that starts waits. Released on drop.
struct Profiles {
    held: Vec<(PathBuf, File)>,
}

impl Profiles {
    /// `None` when a build holds one of the locks.
    fn lock(target: &Path) -> Result<Option<Profiles>> {
        let mut held = vec![];
        for top in folders(target)? {
            let nested = match top.join(LOCK).exists() {
                true => vec![top],
                false => folders(&top)?,
            };
            for dir in nested {
                let lock = dir.join(LOCK);
                let file = match File::open(&lock) {
                    Ok(file) => file,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(Error::io(format!("open {}", lock.display()), e)),
                };
                if !try_lock(&file, &lock)? {
                    return Ok(None);
                }
                held.push((dir, file));
            }
        }
        Ok(Some(Profiles { held }))
    }

    fn holds(&self, dir: &Path) -> bool {
        self.held.iter().any(|(held, _)| held == dir)
    }

    fn holds_under(&self, dir: &Path) -> bool {
        self.held.iter().any(|(held, _)| held.parent() == Some(dir))
    }

    fn remove_incremental(&self) -> Result<()> {
        for (dir, _) in &self.held {
            let cache = dir.join("incremental");
            if std::fs::symlink_metadata(&cache).is_ok() {
                remove(&cache)?;
            }
        }
        Ok(())
    }

    /// Everything in the target but the lock files, which stay so that a build that starts
    /// now locks the very file held here.
    fn empty(&self, target: &Path) -> Result<()> {
        for top in entries(target)? {
            if self.holds(&top) {
                self.clear(&top)?;
            } else if self.holds_under(&top) {
                for entry in entries(&top)? {
                    match self.holds(&entry) {
                        true => self.clear(&entry)?,
                        false => remove(&entry)?,
                    }
                }
            } else {
                remove(&top)?;
            }
        }
        Ok(())
    }

    fn clear(&self, profile: &Path) -> Result<()> {
        for entry in entries(profile)? {
            if entry.file_name() != Some(LOCK.as_ref()) {
                remove(&entry)?;
            }
        }
        Ok(())
    }
}

/// `cap` of `0` is no cap. `busy` is whether an agent of the repo is known to run. Without
/// one, the profile locks are taken and held until the deleting is done.
fn trim_blocking(target: &Path, cap: u64, busy: bool) -> Result<Trim> {
    if cap == 0 || !target.is_dir() {
        return Ok(Trim::Within);
    }
    let size = size_of(target)?;
    if size <= cap {
        return Ok(Trim::Within);
    }
    if busy {
        return Ok(Trim::Busy(size));
    }
    let Some(profiles) = Profiles::lock(target)? else {
        return Ok(Trim::Busy(size));
    };
    profiles.remove_incremental()?;
    if size_of(target)? <= cap {
        return Ok(Trim::Trimmed);
    }
    profiles.empty(target)?;
    Ok(Trim::Trimmed)
}

/// Trims the repo's shared target folder to `max_gb` (`0`: no cap). `busy` is whether an
/// agent of the repo runs.
pub(super) async fn trim(ws: &Workspace, max_gb: u32, busy: bool) -> Result<Trim> {
    let target = ws.cargo_target_dir();
    let cap = u64::from(max_gb) * GB;
    tokio::task::spawn_blocking(move || trim_blocking(&target, cap, busy))
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

    const CAP: u64 = 1 << 20;

    fn file(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7u8; bytes]).unwrap();
    }

    fn big(path: &Path) {
        file(path, 2 * CAP as usize);
    }

    fn profile(target: &Path, name: &str) {
        file(&target.join(name).join(LOCK), 0);
    }

    #[test]
    fn size_counts_allocated_blocks_and_ignores_symlink_targets() {
        let tmp = tempfile::tempdir().unwrap();
        file(&tmp.path().join("a/one"), 10);
        file(&tmp.path().join("a/b/two"), 5000);
        #[cfg(unix)]
        std::os::unix::fs::symlink("/", tmp.path().join("link")).unwrap();
        let size = size_of(tmp.path()).unwrap();
        assert!((4096..CAP).contains(&size), "{size}");
    }

    #[cfg(unix)]
    #[test]
    fn a_hard_linked_file_is_counted_once() {
        let tmp = tempfile::tempdir().unwrap();
        file(&tmp.path().join("a/one"), CAP as usize);
        let once = size_of(tmp.path()).unwrap();
        std::fs::hard_link(tmp.path().join("a/one"), tmp.path().join("a/two")).unwrap();
        assert_eq!(size_of(tmp.path()).unwrap(), once);
    }

    #[test]
    fn a_missing_folder_or_no_cap_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("none");
        assert_eq!(trim_blocking(&missing, CAP, false).unwrap(), Trim::Within);
        big(&tmp.path().join("big"));
        assert_eq!(trim_blocking(tmp.path(), 0, false).unwrap(), Trim::Within);
        assert!(tmp.path().join("big").exists());
    }

    #[test]
    fn a_busy_repo_keeps_everything() {
        let tmp = tempfile::tempdir().unwrap();
        big(&tmp.path().join("debug/incremental/x"));
        let busy = trim_blocking(tmp.path(), CAP, true).unwrap();
        assert!(matches!(busy, Trim::Busy(_)));
        assert!(tmp.path().join("debug/incremental/x").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_running_build_keeps_everything_even_when_no_agent_is_known() {
        let tmp = tempfile::tempdir().unwrap();
        profile(tmp.path(), "debug");
        profile(tmp.path(), "release");
        big(&tmp.path().join("debug/incremental/x"));
        big(&tmp.path().join("release/deps/y"));
        let lock = tmp.path().join("release").join(LOCK);
        let build = File::open(&lock).unwrap();
        assert!(try_lock(&build, &lock).unwrap());
        let result = trim_blocking(tmp.path(), CAP, false).unwrap();
        assert!(matches!(result, Trim::Busy(_)));
        assert!(tmp.path().join("debug/incremental/x").exists());
        assert!(tmp.path().join("release/deps/y").exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_locks_taken_for_a_trim_stop_a_build_until_they_drop() {
        let tmp = tempfile::tempdir().unwrap();
        profile(tmp.path(), "debug");
        let lock = tmp.path().join("debug").join(LOCK);
        let held = Profiles::lock(tmp.path()).unwrap().unwrap();
        let build = File::open(&lock).unwrap();
        assert!(!try_lock(&build, &lock).unwrap());
        drop(held);
        assert!(try_lock(&build, &lock).unwrap());
    }

    #[test]
    fn the_incremental_caches_go_first_and_the_lock_files_stay() {
        let tmp = tempfile::tempdir().unwrap();
        profile(tmp.path(), "debug");
        big(&tmp.path().join("debug/incremental/x"));
        file(&tmp.path().join("debug/deps/dep"), 10);
        assert_eq!(
            trim_blocking(tmp.path(), CAP, false).unwrap(),
            Trim::Trimmed
        );
        assert!(!tmp.path().join("debug/incremental").exists());
        assert!(tmp.path().join("debug/deps/dep").exists());

        big(&tmp.path().join("debug/deps/huge"));
        assert_eq!(
            trim_blocking(tmp.path(), CAP, false).unwrap(),
            Trim::Trimmed
        );
        assert!(!tmp.path().join("debug/deps").exists());
        assert!(tmp.path().join("debug").join(LOCK).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_profile_is_never_followed_out_of_the_target() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        file(&outside.join(LOCK), 0);
        big(&outside.join("incremental/keep"));
        file(&outside.join("deps/keep"), 10);
        let target = tmp.path().join("target");
        profile(&target, "debug");
        big(&target.join("debug/incremental/x"));
        big(&target.join("debug/deps/huge"));
        std::os::unix::fs::symlink(&outside, target.join("evil")).unwrap();
        assert_eq!(trim_blocking(&target, CAP, false).unwrap(), Trim::Trimmed);
        assert!(outside.join("incremental/keep").exists());
        assert!(outside.join("deps/keep").exists());
        assert!(!target.join("evil").exists());
    }

    #[test]
    fn the_size_reads_in_gigabytes() {
        assert_eq!(gb(3_200_000_000), "3.2 GB");
    }
}
