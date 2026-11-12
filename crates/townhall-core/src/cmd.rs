use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::process::Command;

use crate::redact::redact;
use crate::{Error, Result};

/// The first two args (`worktree add`), never the rest: they can hold paths and prompts.
fn head_args(args: &[&str]) -> String {
    args.iter().take(2).copied().collect::<Vec<_>>().join(" ")
}

/// One `cmd.run` line, on `townhall::tmux` for tmux and `townhall::git` for the rest:
/// debug on success, warn with the exit code and redacted stderr on failure.
fn log_run(
    program: &str,
    args: &[&str],
    started: Instant,
    result: &Result<String>,
    expected: bool,
) {
    macro_rules! emit {
        ($target:literal) => {{
            let duration_ms = started.elapsed().as_millis() as u64;
            let args = head_args(args);
            match result {
                Ok(_) => tracing::debug!(
                    target: $target, event = "cmd.run", program, args = %args,
                    duration_ms, exit_code = 0, "command finished"
                ),
                Err(Error::Command { code, stderr, .. }) if expected => tracing::debug!(
                    target: $target, event = "cmd.run", program, args = %args,
                    duration_ms, exit_code = code.unwrap_or(-1), stderr = %redact(stderr),
                    error_kind = "command", "probe answered no"
                ),
                Err(Error::Command { code, stderr, .. }) => {
                    if first_in_window(program, &args, *code, stderr) {
                        tracing::warn!(
                            target: $target, event = "cmd.run", program, args = %args,
                            duration_ms, exit_code = code.unwrap_or(-1),
                            stderr = %redact(stderr), error_kind = "command", "command failed"
                        )
                    }
                }
                Err(e) => tracing::warn!(
                    target: $target, event = "cmd.run", program, args = %args,
                    duration_ms, error_kind = e.kind(), error = %redact(&e.to_string()),
                    "command not run"
                ),
            }
        }};
    }
    match program {
        "tmux" => emit!("townhall::tmux"),
        _ => emit!("townhall::git"),
    }
}

const REPEAT_WINDOW: Duration = Duration::from_secs(60);
const MAX_SEEN: usize = 256;

static SEEN: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

/// True when this exact failure was not warned about in the last minute, so a tick that
/// fails the same way every 2s logs once.
fn first_in_window(program: &str, args: &str, code: Option<i32>, stderr: &str) -> bool {
    let key = format!("{program}\0{args}\0{code:?}\0{stderr}");
    let now = Instant::now();
    let mut guard = SEEN.lock().unwrap_or_else(|p| p.into_inner());
    let seen = guard.get_or_insert_with(HashMap::new);
    if seen.len() >= MAX_SEEN {
        seen.retain(|_, at| now.duration_since(*at) < REPEAT_WINDOW);
        if seen.len() >= MAX_SEEN {
            seen.clear();
        }
    }
    match seen.get(&key) {
        Some(at) if now.duration_since(*at) < REPEAT_WINDOW => false,
        _ => {
            seen.insert(key, now);
            true
        }
    }
}

/// Run `program args...` (optionally in `cwd`) and return trimmed stdout.
pub async fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let started = Instant::now();
    let result = exec(program, args, cwd).await;
    log_run(program, args, started, &result, false);
    result
}

/// `run` for a question: a non-zero exit is a normal answer, so it logs at debug, not warn.
pub async fn probe(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let started = Instant::now();
    let result = exec(program, args, cwd).await;
    log_run(program, args, started, &result, true);
    result
}

async fn exec(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut command = Command::new(program);
    command.args(args);
    if program == "git" {
        command.env("LC_MESSAGES", "C").env_remove("LC_ALL");
    }
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let output = command.output().await.map_err(|source| Error::Spawn {
        program: program.to_string(),
        source,
    })?;
    if !output.status.success() {
        return Err(Error::Command {
            command: format!("{program} {}", args.join(" ")),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// `run` for code that is not async (the store's import, `spawn_blocking` closures).
pub fn run_blocking(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    run_blocking_env(program, args, cwd, &[])
}

/// `run_blocking` with extra environment variables for the child.
pub fn run_blocking_env(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    envs: &[(&str, &str)],
) -> Result<String> {
    let started = Instant::now();
    let result = exec_blocking(program, args, cwd, envs);
    log_run(program, args, started, &result, false);
    result
}

/// `run_blocking` for a question: a non-zero exit logs at debug, not warn.
pub fn probe_blocking(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    probe_blocking_env(program, args, cwd, &[])
}

/// `probe_blocking` with extra environment variables for the child.
pub fn probe_blocking_env(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    envs: &[(&str, &str)],
) -> Result<String> {
    let started = Instant::now();
    let result = exec_blocking(program, args, cwd, envs);
    log_run(program, args, started, &result, true);
    result
}

fn exec_blocking(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    envs: &[(&str, &str)],
) -> Result<String> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    if program == "git" {
        command.env("LC_MESSAGES", "C").env_remove("LC_ALL");
    }
    command.envs(envs.iter().copied());
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let output = command.output().map_err(|source| Error::Spawn {
        program: program.to_string(),
        source,
    })?;
    if !output.status.success() {
        return Err(Error::Command {
            command: format!("{program} {}", args.join(" ")),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Agent names become branch, directory and tmux session names.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidName(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW_ENV: [&str; 3] = ["-c", "alias.e=!env", "e"];

    #[test]
    fn blocking_git_runs_with_the_c_message_locale() {
        let out = run_blocking("git", &SHOW_ENV, None).unwrap();
        assert!(out.lines().any(|l| l == "LC_MESSAGES=C"), "{out}");
    }

    #[tokio::test]
    async fn async_git_runs_with_the_c_message_locale() {
        let out = run("git", &SHOW_ENV, None).await.unwrap();
        assert!(out.lines().any(|l| l == "LC_MESSAGES=C"), "{out}");
    }
}
