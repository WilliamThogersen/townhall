//! One `claude -p` process: its stdin for the turn and the answers, its stdout lines,
//! a stop that interrupts and then kills, and the tail of its stderr.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, Instant};

use super::prompt::interrupt_line;
use crate::{Error, Result};

/// An interrupted CLI writes its result within a second; this long after a stop it is killed.
pub const STOP_GRACE: Duration = Duration::from_secs(3);
/// After its result the CLI shuts its MCP client down and exits; one that hangs is killed.
pub const EXIT_GRACE: Duration = Duration::from_secs(10);
const STDERR_TAIL: usize = 8 * 1024;
const NEVER: Duration = Duration::from_secs(365 * 24 * 3600);
/// Claude Code refuses to start inside another Claude Code session that set this.
const NESTED_ENV: &str = "CLAUDECODE";

/// How the process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    pub code: Option<i32>,
    /// The last non-empty stderr line.
    pub last_error: Option<String>,
    pub stopped: bool,
}

#[derive(Debug)]
pub struct Proc {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Lines<BufReader<ChildStdout>>,
    stderr: JoinHandle<String>,
    kill_at: Option<Instant>,
    stopped: bool,
    watching: bool,
}

async fn tail(mut stderr: tokio::process::ChildStderr) -> String {
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stderr.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                let extra = out.len().saturating_sub(STDERR_TAIL);
                out.drain(..extra);
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn last_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(str::to_string)
}

impl Proc {
    pub fn spawn(program: &str, args: &[String], cwd: &Path) -> Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .env_remove(NESTED_ENV)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| Error::Spawn {
                program: program.to_string(),
                source,
            })?;
        let pipe =
            |what: &str| Error::io(format!("claude {what}"), std::io::Error::other("not piped"));
        let stdout = child.stdout.take().ok_or_else(|| pipe("stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| pipe("stderr"))?;
        Ok(Self {
            stdin: child.stdin.take(),
            lines: BufReader::new(stdout).lines(),
            stderr: tokio::spawn(tail(stderr)),
            child,
            kill_at: None,
            stopped: false,
            watching: true,
        })
    }

    /// Writes one line on stdin. A CLI that already exited is not an error here: its
    /// exit says what went wrong.
    pub async fn write(&mut self, line: &str) {
        let Some(stdin) = self.stdin.as_mut() else {
            return;
        };
        let written = async {
            stdin.write_all(line.as_bytes()).await?;
            stdin.flush().await
        };
        if let Err(e) = written.await {
            eprintln!("warn: claude's stdin closed early: {e}");
            self.stdin = None;
        }
    }

    /// The turn is over: closing stdin lets the CLI exit; one that hangs is killed.
    pub fn close_stdin(&mut self) {
        self.stdin = None;
        if self.kill_at.is_none() {
            self.kill_at = Some(Instant::now() + EXIT_GRACE);
        }
    }

    async fn interrupt(&mut self) {
        self.stopped = true;
        self.write(&interrupt_line("stop")).await;
        self.kill_at = Some(Instant::now() + STOP_GRACE);
    }

    fn kill(&mut self) {
        if let Err(e) = self.child.start_kill() {
            eprintln!("warn: cannot kill claude: {e}");
        }
        self.kill_at = None;
    }

    /// The next stdout line, or `None` at the end. A stop interrupts the turn once, and
    /// the CLI is killed when it does not end in time.
    pub async fn next_line(&mut self, stop: &mut watch::Receiver<bool>) -> Option<String> {
        loop {
            let kill_at = self.kill_at.unwrap_or_else(|| Instant::now() + NEVER);
            tokio::select! {
                line = self.lines.next_line() => return match line {
                    Ok(line) => line,
                    Err(e) => {
                        eprintln!("warn: cannot read claude's output: {e}");
                        None
                    }
                },
                changed = stop.changed(), if self.watching => match changed {
                    Ok(()) if *stop.borrow() && !self.stopped => self.interrupt().await,
                    Ok(()) => {}
                    Err(_) => self.watching = false,
                },
                _ = sleep_until(kill_at), if self.kill_at.is_some() => self.kill(),
            }
        }
    }

    /// Waits for the exit (killing a CLI that outlives its grace) and reads stderr.
    pub async fn finish(mut self) -> Exit {
        self.stdin = None;
        let code = match tokio::time::timeout(EXIT_GRACE, self.child.wait()).await {
            Ok(Ok(status)) => status.code(),
            Ok(Err(e)) => {
                eprintln!("warn: cannot wait for claude: {e}");
                None
            }
            Err(_) => {
                self.kill();
                self.child.wait().await.ok().and_then(|s| s.code())
            }
        };
        let stderr = match self.stderr.await {
            Ok(text) => text,
            Err(e) => format!("cannot read stderr: {e}"),
        };
        Exit {
            code,
            last_error: last_line(&stderr),
            stopped: self.stopped,
        }
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
