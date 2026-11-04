//! Small file helpers for the only files townhall writes besides the database: attachments.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Error, Result};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to a temp file next to `path`, then renames it over `path`,
/// so readers see the old file or the new one, never half of one.
/// `durable` also flushes the temp file to disk before the rename.
pub fn write_atomic(path: &Path, bytes: &[u8], durable: bool) -> Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let name = name.unwrap_or_else(|| "file".into());
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_file_name(format!(".{name}.{}-{n}.tmp", std::process::id()));
    let written = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        if durable {
            file.sync_all()?;
        }
        fs::rename(&tmp, path)
    })();
    written.map_err(|source| {
        if let Err(e) = fs::remove_file(&tmp) {
            if e.kind() != ErrorKind::NotFound {
                eprintln!("warn: cannot remove {}: {e}", tmp.display());
            }
        }
        Error::io(format!("write {}", path.display()), source)
    })
}

/// Creates one new folder (0700 on unix). `AlreadyExists` is returned as is, so callers can claim ids.
pub fn create_dir_new(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        write_atomic(&path, b"one", true).unwrap();
        write_atomic(&path, b"two", false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn write_into_missing_folder_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = write_atomic(&dir.path().join("no/such/x"), b"x", false).unwrap_err();
        assert!(matches!(err, Error::Io { .. }), "{err:?}");
    }

    #[test]
    fn create_dir_new_refuses_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d");
        create_dir_new(&path).unwrap();
        assert_eq!(
            create_dir_new(&path).unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
    }
}
