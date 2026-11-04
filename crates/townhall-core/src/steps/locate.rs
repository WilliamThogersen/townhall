use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{Error, Result};

const TRANSCRIPT_EXT: &str = "jsonl";

/// Claude's folder for sessions started in `cwd`: every char that is not an ASCII letter or
/// digit becomes `-`, one per UTF-16 unit, as Claude's own JavaScript does.
pub fn project_dir_for(claude_projects: &Path, cwd: &Path) -> PathBuf {
    let name: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_string()
            } else {
                "-".repeat(c.len_utf16())
            }
        })
        .collect();
    claude_projects.join(name)
}

/// The newest `.jsonl` transcript for `cwd`, looking under both `cwd` as given and its
/// resolved path, since Claude may have seen either spelling.
pub fn latest_transcript(claude_projects: &Path, cwd: &Path) -> Result<Option<PathBuf>> {
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
    Ok(found.into_iter().flatten().max().map(|(_, path)| path))
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
