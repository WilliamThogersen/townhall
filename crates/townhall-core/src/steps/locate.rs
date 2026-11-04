use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{Error, Result};

const TRANSCRIPT_EXT: &str = "jsonl";

/// Longest folder name Claude writes before it cuts the name and adds a hash.
const MAX_NAME: usize = 200;
const BASE_36: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// Claude's folder for sessions started in `cwd`, named as Claude Code's `sanitizePath` does:
/// each UTF-16 unit that is not an ASCII letter or digit becomes `-`, and a name over 200
/// chars keeps its first 200, then `-` and the cwd's string hash in base 36.
pub fn project_dir_for(claude_projects: &Path, cwd: &Path) -> PathBuf {
    let cwd = cwd.to_string_lossy();
    let name: String = cwd
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_string()
            } else {
                "-".repeat(c.len_utf16())
            }
        })
        .collect();
    if name.len() <= MAX_NAME {
        return claude_projects.join(name);
    }
    let suffix = hash_suffix(string_hash(&cwd));
    claude_projects.join(format!("{}-{suffix}", &name[..MAX_NAME]))
}

/// Java's string hash over UTF-16 units, wrapping at 32 bits like JavaScript's `| 0`.
fn string_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0, |h: i32, unit| {
        h.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

/// `Math.abs(hash).toString(36)`.
fn hash_suffix(hash: i32) -> String {
    let digits: Vec<char> =
        std::iter::successors(Some(hash.unsigned_abs()), |n| (*n >= 36).then_some(n / 36))
            .map(|n| char::from(BASE_36[(n % 36) as usize]))
            .collect();
    digits.into_iter().rev().collect()
}

/// The newest `.jsonl` transcript for `cwd`, looking under both `cwd` as given and its
/// resolved path, since Claude may have seen either spelling.
pub fn latest_transcript(claude_projects: &Path, cwd: &Path) -> Result<Option<PathBuf>> {
    newest_transcript(claude_projects, cwd, None)
}

/// Like [`latest_transcript`], but only a transcript last written at or after `since`.
pub fn latest_transcript_since(
    claude_projects: &Path,
    cwd: &Path,
    since: SystemTime,
) -> Result<Option<PathBuf>> {
    newest_transcript(claude_projects, cwd, Some(since))
}

fn newest_transcript(
    claude_projects: &Path,
    cwd: &Path,
    since: Option<SystemTime>,
) -> Result<Option<PathBuf>> {
    let given = project_dir_for(claude_projects, cwd);
    let resolved = cwd
        .canonicalize()
        .ok()
        .map(|real| project_dir_for(claude_projects, &real))
        .filter(|dir| *dir != given);
    let found = [Some(given), resolved]
        .into_iter()
        .flatten()
        .map(|dir| newest_in(&dir))
        .collect::<Result<Vec<_>>>()?;
    let newest = found.into_iter().flatten().max();
    Ok(newest
        .filter(|(written, _)| since.is_none_or(|cutoff| *written >= cutoff))
        .map(|(_, path)| path))
}

/// The newest transcript file in `dir`, or none when the folder does not exist.
fn newest_in(dir: &Path) -> Result<Option<(SystemTime, PathBuf)>> {
    let io = |source: std::io::Error| Error::Io {
        context: format!("read transcripts in {}", dir.display()),
        source,
    };
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    let found = entries
        .map(|entry| transcript(entry.map_err(io)?.path()).map_err(io))
        .collect::<Result<Vec<_>>>()?;
    Ok(found.into_iter().flatten().max())
}

/// `path` with its modified time when it is a transcript file that still exists.
fn transcript(path: PathBuf) -> std::io::Result<Option<(SystemTime, PathBuf)>> {
    if path.extension().and_then(|e| e.to_str()) != Some(TRANSCRIPT_EXT) {
        return Ok(None);
    }
    match std::fs::metadata(&path) {
        Ok(meta) if meta.is_file() => Ok(Some((meta.modified()?, path))),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::hash_suffix;

    #[test]
    fn hash_suffix_is_the_absolute_value_in_base_36() {
        assert_eq!(hash_suffix(0), "0");
        assert_eq!(hash_suffix(-35), "z");
        assert_eq!(hash_suffix(36), "10");
        assert_eq!(hash_suffix(i32::MIN), "zik0zk");
        assert_eq!(hash_suffix(i32::MAX), "zik0zj");
    }
}
