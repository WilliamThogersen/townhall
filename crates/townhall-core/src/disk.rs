//! Free space on the volume townhall writes to: a task does not start on a nearly full
//! disk, and a start that still runs out of space says so instead of quoting an os error.

use std::path::Path;

use crate::{Error, Result};

const GB: u64 = 1_000_000_000;
const NO_SPACE_TEXT: &str = "No space left on device";

/// Bytes a normal user can still write on the volume of `path`, or of its nearest folder
/// that exists (the home is created on first use).
#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
pub fn free_bytes(path: &Path) -> Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let folder = path
        .ancestors()
        .find(|p| p.exists())
        .ok_or_else(|| Error::InvalidInput(format!("no folder of {} exists", path.display())))?;
    let c_path = std::ffi::CString::new(folder.as_os_str().as_bytes())
        .map_err(|_| Error::InvalidInput(format!("path {} holds a NUL", folder.display())))?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // `statvfs` reads the NUL-terminated path and fills `stat` when it returns 0.
    let status = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if status != 0 {
        let cause = std::io::Error::last_os_error();
        return Err(Error::io(
            format!("free space of {}", folder.display()),
            cause,
        ));
    }
    let stat = unsafe { stat.assume_init() };
    Ok((stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64))
}

#[cfg(not(unix))]
pub fn free_bytes(path: &Path) -> Result<u64> {
    Err(Error::InvalidInput(format!(
        "free space of {} is not read on this platform",
        path.display()
    )))
}

/// `floor_gb` of `0` turns the check off.
pub fn below_floor(free: u64, floor_gb: u32) -> bool {
    free < u64::from(floor_gb) * GB
}

/// What the planner and the UI read when the disk is nearly full.
pub fn low_message(free: u64) -> String {
    format!("disk almost full: {:.1} GB free", free as f64 / GB as f64)
}

/// Whether a failure is the disk running full, from the os error or from a command's stderr.
pub fn out_of_space(e: &Error) -> bool {
    let full = |io: &std::io::Error| io.raw_os_error() == Some(libc::ENOSPC);
    match e {
        Error::Io { source, .. } | Error::Spawn { source, .. } => full(source),
        Error::Command { stderr, .. } => stderr.contains(NO_SPACE_TEXT),
        _ => false,
    }
}

/// The text for a failure that is the disk running full; `None` for any other failure.
pub fn explain(e: &Error, free: Option<u64>) -> Option<String> {
    out_of_space(e).then(|| match free {
        Some(free) => low_message(free),
        None => "disk full: no space left on device".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_space_is_read_from_the_nearest_existing_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let free = free_bytes(tmp.path()).unwrap();
        assert!(free > 0);
        let missing = tmp.path().join("not/yet/there");
        assert_eq!(
            free_bytes(&missing).unwrap(),
            free_bytes(tmp.path()).unwrap()
        );
    }

    #[test]
    fn the_message_says_how_much_is_free() {
        assert_eq!(low_message(3_200_000_000), "disk almost full: 3.2 GB free");
        assert_eq!(low_message(0), "disk almost full: 0.0 GB free");
        assert_eq!(low_message(260_000_000), "disk almost full: 0.3 GB free");
    }

    #[test]
    fn the_floor_is_in_gigabytes_and_zero_is_off() {
        assert!(below_floor(4_999_999_999, 5));
        assert!(!below_floor(5_000_000_000, 5));
        assert!(!below_floor(0, 0));
    }

    #[test]
    fn only_a_full_disk_counts_as_out_of_space() {
        let full = || std::io::Error::from_raw_os_error(libc::ENOSPC);
        assert!(out_of_space(&Error::io("write the brief", full())));
        let spawn = Error::Spawn {
            program: "git".into(),
            source: full(),
        };
        assert!(out_of_space(&spawn));
        let cmd = |stderr: &str| Error::Command {
            command: "git worktree add".into(),
            code: Some(128),
            stderr: stderr.into(),
        };
        assert!(out_of_space(&cmd(
            "fatal: write error: No space left on device"
        )));
        assert!(!out_of_space(&cmd("fatal: not a git repository")));
        let denied = std::io::Error::from_raw_os_error(libc::EACCES);
        assert!(!out_of_space(&Error::io("x", denied)));
        assert!(!out_of_space(&Error::InvalidInput("No space left".into())));
    }

    #[test]
    fn a_full_disk_failure_reads_as_the_disk_message() {
        let full = Error::io("write", std::io::Error::from_raw_os_error(libc::ENOSPC));
        let text = explain(&full, Some(120_000_000)).unwrap();
        assert_eq!(text, "disk almost full: 0.1 GB free");
        assert_eq!(
            explain(&full, None).unwrap(),
            "disk full: no space left on device"
        );
        assert!(explain(&Error::Conflict("x".into()), Some(1)).is_none());
    }
}
