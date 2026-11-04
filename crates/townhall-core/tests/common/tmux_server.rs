//! Each test process gets a tmux server of its own, so no test shares the default server
//! with real agents or inherits the environment of whoever started it. Shared by the core
//! and daemon test harnesses.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use townhall_core::tmux::{server_args, SOCKET_ENV};

/// Points `TOWNHALL_TMUX_SOCKET` at a server only this process uses, once per process,
/// and kills that server when the process exits. Harnesses call it before any tmux call.
pub fn private_tmux() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let own = format!("townhall-test-{}-", std::process::id());
        let current = std::env::var(SOCKET_ENV).unwrap_or_default();
        if current.starts_with(&own) {
            return;
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        std::env::set_var(SOCKET_ENV, format!("{own}{nanos}"));
        // SAFETY: registers a plain function that captures nothing.
        if unsafe { libc::atexit(kill_private_tmux) } != 0 {
            eprintln!("warn: the test tmux server is not killed at exit");
        }
    });
}

/// `tmux` aimed at this process's own server.
pub fn tmux() -> Command {
    private_tmux();
    let mut command = Command::new("tmux");
    command.args(server_args().expect("a valid test tmux socket name"));
    command
}

/// Where tmux puts the socket of this process's server: tmux leaves the file behind.
fn socket_file() -> Option<PathBuf> {
    let name = std::env::var(SOCKET_ENV).ok()?;
    let dir = std::env::var_os("TMUX_TMPDIR")
        .filter(|d| !d.is_empty())
        .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    Some(dir.join(format!("tmux-{uid}")).join(name))
}

/// Runs at exit. A server whose last session ended is gone already, so a failure here
/// only means there was nothing left to kill or remove.
extern "C" fn kill_private_tmux() {
    let _ = tmux().arg("kill-server").output();
    if let Some(file) = socket_file() {
        let _ = std::fs::remove_file(file);
    }
}
