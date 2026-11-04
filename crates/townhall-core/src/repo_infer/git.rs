//! Bounded git calls: each runs against a deadline, and a child that outlives
//! its call is killed.

use std::path::Path;
use std::process::{Output, Stdio};

use tokio::process::{Child, Command};
use tokio::time::{timeout_at, Instant};

use crate::{Error, Result};

/// Files per token-regex run, well under the argument length limit.
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
/// A fixed-string pass finds the files that hold it at all; only those get
/// the slower token regex, a batch at a time, in git's file order.
pub(crate) async fn grep_first(dir: &Path, needle: &str, deadline: Instant) -> Result<Grep> {
    let prefilter = ["grep", "-l", "-z", "-F", "-I", "-e", needle];
    let Some(files) = grep_files(dir, &prefilter, deadline).await? else {
        return Ok(Grep::TimedOut);
    };
    let pattern = token_pattern(needle);
    let regex = ["--literal-pathspecs", "grep", "-l", "-z", "-E", "-I"];
    for batch in files.chunks(BATCH) {
        let args: Vec<&str> = regex
            .into_iter()
            .chain(["-e", &pattern, "--"])
            .chain(batch.iter().map(String::as_str))
            .collect();
        match grep_files(dir, &args, deadline).await? {
            None => return Ok(Grep::TimedOut),
            Some(found) => {
                if let Some(file) = found.into_iter().next() {
                    return Ok(Grep::Hit(file));
                }
            }
        }
    }
    Ok(Grep::Miss)
}

/// The files a `git grep -l -z` names; empty when nothing matches (exit 1),
/// `None` when out of time.
async fn grep_files(dir: &Path, args: &[&str], deadline: Instant) -> Result<Option<Vec<String>>> {
    let Some(out) = output(dir, args, deadline).await? else {
        return Ok(None);
    };
    match out.status.code() {
        Some(0 | 1) => Ok(Some(nul_list(&out.stdout))),
        _ => Err(failed(args, &out)),
    }
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
