//! One owner per database: `townhall.lock` next to it holds an exclusive OS lock for as long
//! as the store is open. A second open, from another process or from the same process
//! (the desktop app's in-process daemon), fails at once. The OS drops the lock if townhall
//! crashes. A child process starting at that moment holds a copy of the descriptor until it
//! runs its program, so a busy lock is retried for a moment before it counts as taken.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

use crate::{Error, Result};

pub const LOCK_FILE: &str = "townhall.lock";
const ATTEMPTS: u32 = 10;
const PAUSE: Duration = Duration::from_millis(30);

#[derive(Debug)]
pub struct DbLock {
    _file: File,
}

fn holder(file: &mut File) -> String {
    let mut text = String::new();
    match file.read_to_string(&mut text) {
        Ok(_) if !text.trim().is_empty() => format!(" (pid {})", text.trim()),
        _ => String::new(),
    }
}

pub fn acquire(dir: &Path) -> Result<DbLock> {
    let path = dir.join(LOCK_FILE);
    let context = || format!("lock {}", path.display());
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| Error::io(context(), e))?;
    let mut attempt = 1;
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) if attempt < ATTEMPTS => {
                attempt += 1;
                std::thread::sleep(PAUSE);
            }
            Err(TryLockError::WouldBlock) => {
                return Err(Error::Unavailable(format!(
                    "townhall.db is in use by another townhall{}",
                    holder(&mut file)
                )))
            }
            Err(TryLockError::Error(e)) => return Err(Error::io(context(), e)),
        }
    }
    let wrote = file
        .set_len(0)
        .and_then(|()| file.seek(SeekFrom::Start(0)))
        .and_then(|_| write!(file, "{}", std::process::id()));
    wrote.map_err(|e| Error::io(context(), e))?;
    Ok(DbLock { _file: file })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_fails_until_the_first_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let first = acquire(dir.path()).unwrap();
        let err = acquire(dir.path()).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("in use by another townhall"), "{text}");
        assert!(text.contains(&std::process::id().to_string()), "{text}");
        drop(first);
        assert!(acquire(dir.path()).is_ok());
    }
}
