//! Bounded git calls: each runs against a deadline, and a child that outlives
//! its call is killed.

use std::path::Path;
use std::process::{Output, Stdio};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio::time::{timeout_at, Instant};

use crate::{Error, Result};

/// Files the prefilter lists before it stops. Up to this many get the token
/// regex by name (well under the argument length limit); more get one pass
/// over the whole repo that stops at the first hit.
pub(crate) const BATCH: usize = 128;
const REGEX_SPECIAL: &str = "\\.[](){}*+?|^$";
/// Before a name (`kill_on_drop`): `.`, `:` and `/` may come first.
const BEFORE_NAME: &str = "[^[:alnum:]_@-]";
/// Before anything else (`/info`): a `.` makes it another path (`./info`).
const BEFORE_OTHER: &str = "[^[:alnum:]_./@-]";
/// After: `/` may follow (`@shop/ui/button`); a full stop ends the token
/// unless more of a name follows (`query.ts.bak`).
const AFTER: &str = "($|[^[:alnum:]_.@-]|[.]($|[^[:alnum:]_]))";

/// The first file `git grep` found, no file, or out of time.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Grep {
    Hit(String),
    Miss,
    TimedOut,
}

fn spawn(dir: &Path, args: &[&str]) -> std::io::Result<Child> {
    Command::new("git")
        .args(["-c", "core.quotePath=false"])
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
}

fn spawn_error(source: std::io::Error) -> Error {
    Error::Spawn {
        program: "git".to_string(),
        source,
    }
}

fn io_error(dir: &Path, source: std::io::Error) -> Error {
    Error::Io {
        context: format!("git in {}", dir.display()),
        source,
    }
}

fn failed(args: &[&str], out: &Output) -> Error {
    Error::Command {
        command: format!("git {}", args.join(" ")),
        code: out.status.code(),
        stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    }
}

fn nul_list(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// Run git to completion; `None` when out of time.
async fn output(dir: &Path, args: &[&str], deadline: Instant) -> Result<Option<Output>> {
    if Instant::now() >= deadline {
        return Ok(None);
    }
    let child = spawn(dir, args).map_err(spawn_error)?;
    let Ok(out) = timeout_at(deadline, child.wait_with_output()).await else {
        return Ok(None);
    };
    out.map(Some).map_err(|source| io_error(dir, source))
}

/// Whether `dir` is a git work tree. A missing folder, a folder outside git
/// or a repo that does not answer in time is not.
pub(crate) async fn is_work_tree(dir: &Path, deadline: Instant) -> Result<bool> {
    if !dir.is_dir() {
        return Ok(false);
    }
    let out = match output(dir, &["rev-parse", "--is-inside-work-tree"], deadline).await {
        Ok(out) => out,
        Err(Error::Spawn { .. }) if !dir.is_dir() => return Ok(false),
        Err(e) => return Err(e),
    };
    Ok(out.is_some_and(|out| {
        out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true"
    }))
}

/// Tracked paths under `dir`, or `None` when out of time.
pub(crate) async fn ls_files(dir: &Path, deadline: Instant) -> Result<Option<Vec<String>>> {
    let args = ["ls-files", "-z"];
    let Some(out) = output(dir, &args, deadline).await? else {
        return Ok(None);
    };
    if !out.status.success() {
        return Err(failed(&args, &out));
    }
    Ok(Some(nul_list(&out.stdout)))
}

/// An extended regex for `needle` as a whole token.
pub(crate) fn token_pattern(needle: &str) -> String {
    let escaped: String = needle
        .chars()
        .flat_map(|c| {
            REGEX_SPECIAL
                .contains(c)
                .then_some('\\')
                .into_iter()
                .chain([c])
        })
        .collect();
    let name = needle
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_');
    let before = if name { BEFORE_NAME } else { BEFORE_OTHER };
    format!("(^|{before}){escaped}{AFTER}")
}

/// The first tracked text file under `dir` that has `needle` as a whole token.
/// A fixed-string prefilter finds whether any file holds it at all; only then
/// does the slower token regex run, on those files or, when there are many,
/// over the repo until its first hit.
pub(crate) async fn grep_first(dir: &Path, needle: &str, deadline: Instant) -> Result<Grep> {
    let prefilter = ["grep", "-l", "-z", "-F", "-I", "-e", needle];
    let Some(files) = list_names(dir, &prefilter, BATCH + 1, deadline).await? else {
        return Ok(Grep::TimedOut);
    };
    if files.is_empty() {
        return Ok(Grep::Miss);
    }
    let pattern = token_pattern(needle);
    let regex = [
        "--literal-pathspecs",
        "grep",
        "-l",
        "-z",
        "-E",
        "-I",
        "-e",
        &pattern,
    ];
    let args: Vec<&str> = if files.len() <= BATCH {
        let names = files.iter().map(String::as_str);
        regex.into_iter().chain(["--"]).chain(names).collect()
    } else {
        regex.to_vec()
    };
    Ok(match list_names(dir, &args, 1, deadline).await? {
        None => Grep::TimedOut,
        Some(found) => found.into_iter().next().map_or(Grep::Miss, Grep::Hit),
    })
}

/// Up to `max` of the names a `git grep -l -z` prints, read as they come; the
/// child is killed at `max`. Empty when nothing matches, `None` when out of time.
pub(crate) async fn list_names(
    dir: &Path,
    args: &[&str],
    max: usize,
    deadline: Instant,
) -> Result<Option<Vec<String>>> {
    if Instant::now() >= deadline {
        return Ok(None);
    }
    let mut child = spawn(dir, args).map_err(spawn_error)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io_error(dir, std::io::Error::other("git grep has no stdout")))?;
    let Ok(names) = timeout_at(deadline, read_names(stdout, max)).await else {
        return Ok(None);
    };
    let names = names.map_err(|source| io_error(dir, source))?;
    if names.len() >= max {
        return Ok(Some(names));
    }
    let Ok(out) = timeout_at(deadline, child.wait_with_output()).await else {
        return Ok(None);
    };
    let out = out.map_err(|source| io_error(dir, source))?;
    match out.status.code() {
        Some(0 | 1) => Ok(Some(names)),
        _ => Err(failed(args, &out)),
    }
}

async fn read_names(stdout: ChildStdout, max: usize) -> std::io::Result<Vec<String>> {
    let mut reader = BufReader::new(stdout);
    let mut names = Vec::new();
    let mut name = Vec::new();
    while names.len() < max {
        name.clear();
        if reader.read_until(b'\0', &mut name).await? == 0 {
            break;
        }
        let bytes = name.strip_suffix(b"\0").unwrap_or(&name);
        if !bytes.is_empty() {
            names.push(String::from_utf8_lossy(bytes).into_owned());
        }
    }
    Ok(names)
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
