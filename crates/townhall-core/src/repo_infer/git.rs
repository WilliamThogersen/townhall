//! Bounded git calls: each runs against a deadline, and a child that outlives
//! its call is killed.

use std::future::poll_fn;
use std::path::Path;
use std::pin::Pin;
use std::process::{Output, Stdio};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::process::{Child, ChildStdout, Command};
use tokio::time::{timeout_at, Instant};

use crate::{Error, Result};

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

/// Whether `dir` is a git work tree. A missing folder, a folder outside git
/// or a repo that does not answer in time is not.
pub(crate) async fn is_work_tree(dir: &Path, deadline: Instant) -> Result<bool> {
    if !dir.is_dir() {
        return Ok(false);
    }
    let child = match spawn(dir, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(child) => child,
        Err(_) if !dir.is_dir() => return Ok(false),
        Err(source) => return Err(spawn_error(source)),
    };
    let Ok(out) = timeout_at(deadline, child.wait_with_output()).await else {
        return Ok(false);
    };
    let out = out.map_err(|source| io_error(dir, source))?;
    Ok(out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true")
}

/// Tracked paths under `dir`, or `None` when out of time.
pub(crate) async fn ls_files(dir: &Path, deadline: Instant) -> Result<Option<Vec<String>>> {
    let args = ["ls-files", "-z"];
    let child = spawn(dir, &args).map_err(spawn_error)?;
    let Ok(out) = timeout_at(deadline, child.wait_with_output()).await else {
        return Ok(None);
    };
    let out = out.map_err(|source| io_error(dir, source))?;
    if !out.status.success() {
        return Err(failed(&args, &out));
    }
    let files = String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|file| !file.is_empty())
        .map(str::to_string)
        .collect();
    Ok(Some(files))
}

/// Characters that continue a token, so a hit must not touch one.
const TOKEN_CHARS: &str = "[:alnum:]_./@-";
const REGEX_SPECIAL: &str = "\\.[](){}*+?|^$";

/// An extended regex for `needle` as a whole token. A full stop after it
/// still ends it, unless more of a name follows (`query.ts.bak`).
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
    format!("(^|[^{TOKEN_CHARS}]){escaped}($|[^{TOKEN_CHARS}]|[.]($|[^[:alnum:]_]))")
}

/// The first tracked text file under `dir` that has `needle` as a whole token.
pub(crate) async fn grep_first(dir: &Path, needle: &str, deadline: Instant) -> Result<Grep> {
    if Instant::now() >= deadline {
        return Ok(Grep::TimedOut);
    }
    let pattern = token_pattern(needle);
    let args = ["grep", "-l", "-E", "-I", "--max-count=1", "-e", &pattern];
    let mut child = spawn(dir, &args).map_err(spawn_error)?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io_error(dir, std::io::Error::other("git grep has no stdout")))?;
    match timeout_at(deadline, first_line(&mut stdout)).await {
        Err(_) => Ok(Grep::TimedOut),
        Ok(Err(source)) => Err(io_error(dir, source)),
        Ok(Ok(Some(file))) => Ok(Grep::Hit(file)),
        Ok(Ok(None)) => finish_grep(child, dir, &args, deadline).await,
    }
}

/// `git grep` exits 1 when nothing matches; any other failure is an error.
async fn finish_grep(child: Child, dir: &Path, args: &[&str], deadline: Instant) -> Result<Grep> {
    let Ok(out) = timeout_at(deadline, child.wait_with_output()).await else {
        return Ok(Grep::TimedOut);
    };
    let out = out.map_err(|source| io_error(dir, source))?;
    match out.status.code() {
        Some(0 | 1) => Ok(Grep::Miss),
        _ => Err(failed(args, &out)),
    }
}

/// Output up to the first newline; `None` when the output ends empty.
async fn first_line(stdout: &mut ChildStdout) -> std::io::Result<Option<String>> {
    let mut line = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = read_some(stdout, &mut chunk).await?;
        if read == 0 {
            break;
        }
        line.extend_from_slice(&chunk[..read]);
        if let Some(end) = line.iter().position(|&b| b == b'\n') {
            return Ok(Some(String::from_utf8_lossy(&line[..end]).into_owned()));
        }
    }
    Ok((!line.is_empty()).then(|| String::from_utf8_lossy(&line).into_owned()))
}

/// One read into `buf` (tokio's `io-util` helpers are not enabled here).
async fn read_some(stdout: &mut ChildStdout, buf: &mut [u8]) -> std::io::Result<usize> {
    poll_fn(|cx| {
        let mut filled = ReadBuf::new(buf);
        Pin::new(&mut *stdout)
            .poll_read(cx, &mut filled)
            .map_ok(|()| filled.filled().len())
    })
    .await
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
