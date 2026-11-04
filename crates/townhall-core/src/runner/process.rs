//! One `claude -p` process in its own process group: its stdin for the turn and the
//! answers, its stdout lines, a stop that interrupts and then kills the whole group, and
//! the tail of its stderr. Once it was killed or exited, reading gives up after a short
//! wait, so a child that keeps the pipes open never holds the turn.

use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, timeout_at, Instant};

use super::prompt::interrupt_line;
use crate::{Error, Result};

/// An interrupted CLI writes its result within a second; this long after a stop it is killed.
pub const STOP_GRACE: Duration = Duration::from_secs(3);
/// After its result the CLI shuts its MCP client down and exits; one that hangs is killed.
pub const EXIT_GRACE: Duration = Duration::from_secs(10);
/// How long the pipes may stay open once the CLI was killed or exited.
pub const KILL_WAIT: Duration = Duration::from_secs(2);
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

type Tail = Arc<Mutex<Vec<u8>>>;

#[derive(Debug)]
pub struct Proc {
    child: Child,
    /// The process group: the child's pid, since it leads its own group.
    group: Option<u32>,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    line: Vec<u8>,
    stderr: (Tail, JoinHandle<()>),
    kill_at: Option<Instant>,
    give_up_at: Option<Instant>,
    /// The exit code once the child was reaped (`None` inside: killed by a signal).
    reaped: Option<Option<i32>>,
    stopped: bool,
    watching: bool,
}

async fn tail(mut stderr: ChildStderr, out: Tail) {
    let mut buf = [0u8; 4096];
    while let Ok(n @ 1..) = stderr.read(&mut buf).await {
        let mut kept = out.lock().unwrap_or_else(|p| p.into_inner());
        kept.extend_from_slice(&buf[..n]);
        let extra = kept.len().saturating_sub(STDERR_TAIL);
        kept.drain(..extra);
    }
}

fn last_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(str::to_string)
}

/// SIGKILL to every process in the group; a group that is already gone is fine.
/// `killpg` takes plain integers and touches no memory, so the call is sound.
#[cfg(unix)]
fn kill_group(group: u32) {
    let Ok(pgid) = libc::pid_t::try_from(group) else {
        return;
    };
    if unsafe { libc::killpg(pgid, libc::SIGKILL) } != 0 {
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::ESRCH) {
            eprintln!("warn: cannot kill claude's process group {group}: {e}");
        }
    }
}

#[cfg(not(unix))]
fn kill_group(_group: u32) {}

fn command(program: &str, args: &[String], cwd: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env_remove(NESTED_ENV)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    command
}

impl Proc {
    pub fn spawn(program: &str, args: &[String], cwd: &Path) -> Result<Self> {
        let mut child = command(program, args, cwd)
            .spawn()
            .map_err(|source| Error::Spawn {
                program: program.to_string(),
                source,
            })?;
        let pipe =
            |what: &str| Error::io(format!("claude {what}"), std::io::Error::other("not piped"));
        let stdout = child.stdout.take().ok_or_else(|| pipe("stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| pipe("stderr"))?;
        let kept = Tail::default();
        Ok(Self {
            group: child.id(),
            stdin: child.stdin.take(),
            stdout: BufReader::new(stdout),
            line: Vec::new(),
            stderr: (Arc::clone(&kept), tokio::spawn(tail(stderr, kept))),
            child,
            kill_at: None,
            give_up_at: None,
            reaped: None,
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

    fn give_up_soon(&mut self) {
        self.give_up_at.get_or_insert(Instant::now() + KILL_WAIT);
    }

    /// Kills the whole group while the child is not reaped, so its pid is still ours.
    fn kill(&mut self) {
        self.kill_at = None;
        self.give_up_soon();
        if self.reaped.is_some() {
            return;
        }
        if let Some(group) = self.group {
            kill_group(group);
        }
        if let Err(e) = self.child.start_kill() {
            eprintln!("warn: cannot kill claude: {e}");
        }
    }

    /// The child exited and was reaped, so its pid is no longer ours and its group is
    /// left alone; reading gives up soon if something else keeps the pipes open.
    fn exited(&mut self, status: std::io::Result<ExitStatus>) {
        let code = match status {
            Ok(status) => status.code(),
            Err(e) => {
                eprintln!("warn: cannot wait for claude: {e}");
                None
            }
        };
        self.reaped = Some(code);
        self.kill_at = None;
        self.give_up_soon();
    }

    fn take_line(&mut self, read: std::io::Result<usize>) -> Option<String> {
        match read {
            Ok(0) if self.line.is_empty() => None,
            Ok(_) => {
                let text = String::from_utf8_lossy(&self.line);
                let text = text.strip_suffix('\n').unwrap_or(&text);
                let text = text.strip_suffix('\r').unwrap_or(text).to_string();
                self.line.clear();
                Some(text)
            }
            Err(e) => {
                eprintln!("warn: cannot read claude's output: {e}");
                None
            }
        }
    }

    /// The next stdout line (a bad one decoded lossily), or `None` at the end. A stop
    /// interrupts the turn once; a CLI that does not end in time is killed.
    pub async fn next_line(&mut self, stop: &mut watch::Receiver<bool>) -> Option<String> {
        loop {
            let far = Instant::now() + NEVER;
            let (kill_at, give_up_at) = (self.kill_at, self.give_up_at);
            tokio::select! {
                read = self.stdout.read_until(b'\n', &mut self.line) => return self.take_line(read),
                changed = stop.changed(), if self.watching => match changed {
                    Ok(()) if *stop.borrow() && !self.stopped => self.interrupt().await,
                    Ok(()) => {}
                    Err(_) => self.watching = false,
                },
                status = self.child.wait(), if self.reaped.is_none() => self.exited(status),
                _ = sleep_until(kill_at.unwrap_or(far)), if kill_at.is_some() => self.kill(),
                _ = sleep_until(give_up_at.unwrap_or(far)), if give_up_at.is_some() => {
                    eprintln!("warn: claude's output stayed open after it ended");
                    return None;
                }
            }
        }
    }

    async fn reap(&mut self) -> Option<i32> {
        if let Some(code) = self.reaped {
            return code;
        }
        let deadline = Instant::now() + EXIT_GRACE;
        if let Ok(status) = timeout_at(deadline, self.child.wait()).await {
            self.exited(status);
        } else {
            self.kill();
            let status = self.child.wait().await;
            self.exited(status);
        }
        self.reaped.flatten()
    }

    /// Reaps the child (killing one that outlives its grace) and reads stderr.
    pub async fn finish(mut self) -> Exit {
        self.stdin = None;
        let code = self.reap().await;
        let deadline = self
            .give_up_at
            .unwrap_or_else(|| Instant::now() + KILL_WAIT);
        if timeout_at(deadline, &mut self.stderr.1).await.is_err() {
            self.stderr.1.abort();
        }
        let kept = self.stderr.0.lock().unwrap_or_else(|p| p.into_inner());
        let last_error = last_line(&String::from_utf8_lossy(&kept));
        drop(kept);
        Exit {
            code,
            last_error,
            stopped: self.stopped,
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if let (None, Some(group)) = (self.reaped, self.group) {
            kill_group(group);
        }
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
