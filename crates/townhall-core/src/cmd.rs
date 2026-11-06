use std::path::Path;
use std::time::Instant;

use tokio::process::Command;

use crate::redact::redact;
use crate::{Error, Result};

/// The first two args (`worktree add`), never the rest: they can hold paths and prompts.
fn head_args(args: &[&str]) -> String {
    args.iter().take(2).copied().collect::<Vec<_>>().join(" ")
}

/// One `cmd.run` line, on `townhall::tmux` for tmux and `townhall::git` for the rest:
/// debug on success, warn with the exit code and redacted stderr on failure.
fn log_run(program: &str, args: &[&str], started: Instant, result: &Result<String>) {
    macro_rules! emit {
        ($target:literal) => {{
            let duration_ms = started.elapsed().as_millis() as u64;
            let args = head_args(args);
            match result {
                Ok(_) => tracing::debug!(
                    target: $target, event = "cmd.run", program, args = %args,
                    duration_ms, exit_code = 0, "command finished"
                ),
                Err(Error::Command { code, stderr, .. }) => tracing::warn!(
                    target: $target, event = "cmd.run", program, args = %args,
                    duration_ms, exit_code = code.unwrap_or(-1), stderr = %redact(stderr),
                    error_kind = "command", "command failed"
                ),
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

/// Run `program args...` (optionally in `cwd`) and return trimmed stdout.
pub async fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let started = Instant::now();
    let result = exec(program, args, cwd).await;
    log_run(program, args, started, &result);
    result
}

async fn exec(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut command = Command::new(program);
    command.args(args);
    if program == "git" {
        command.env("LC_ALL", "C");
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
    log_run(program, args, started, &result);
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
        command.env("LC_ALL", "C");
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
