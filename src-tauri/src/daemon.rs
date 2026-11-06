use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use townhall_core::redact::redact;
use townhall_core::store::Store;
use townhall_core::{ContextOptions, Home, Workspace};
use townhall_daemon::{router_with_cors, serve, start_background, AppState, Background};

pub const BIND: &str = "127.0.0.1:7777";
const REBIND_ATTEMPTS: u32 = 20;
const REBIND_DELAY: Duration = Duration::from_millis(50);
const STOP_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Served by this app, in-process.
    Embedded,
    /// The port was taken, so an already running daemon is used as is.
    External,
    Stopped,
}

struct Running {
    shutdown: oneshot::Sender<()>,
    handle: JoinHandle<()>,
    background: Background,
}

struct Slot {
    mode: Mode,
    running: Option<Running>,
}

/// Owns the in-process daemon. Stopping it only closes the HTTP server:
/// agents live in tmux and are never touched.
pub struct Controller {
    addr: SocketAddr,
    home: Home,
    /// Opened once by the app and kept across restarts. `None` when another townhall holds it.
    store: Option<Store>,
    slot: Mutex<Slot>,
}

impl Controller {
    pub fn new(addr: SocketAddr, home: Home, store: Option<Store>) -> Self {
        Self {
            addr,
            home,
            store,
            slot: Mutex::new(Slot {
                mode: Mode::Stopped,
                running: None,
            }),
        }
    }

    pub async fn mode(&self) -> Mode {
        self.slot.lock().await.mode
    }

    /// Starts serving `repo`, or adopts a daemon already on the port.
    pub async fn start(&self, repo: &Path) -> Result<Mode, String> {
        let mut slot = self.slot.lock().await;
        if slot.mode != Mode::Stopped {
            return Err("daemon is already started".into());
        }
        start_locked(&mut slot, self, repo, None).await
    }

    /// Like `start`, but serves on a listener the caller already holds, so the port cannot be
    /// taken between choosing it and binding it.
    #[cfg(test)]
    async fn start_on(&self, listener: TcpListener, repo: &Path) -> Result<Mode, String> {
        let mut slot = self.slot.lock().await;
        if slot.mode != Mode::Stopped {
            return Err("daemon is already started".into());
        }
        start_locked(&mut slot, self, repo, Some(listener)).await
    }

    /// Restarts the in-process daemon on a new repo. Refuses when the daemon is external.
    pub async fn restart(&self, repo: &Path) -> Result<Mode, String> {
        let mut slot = self.slot.lock().await;
        if slot.mode == Mode::External {
            return Err(format!(
                "a daemon is already running on {} with its own repo; stop it to change the repo here",
                self.addr
            ));
        }
        let began = Instant::now();
        stop_locked(&mut slot).await;
        let released = wait_released(self.addr).await;
        let mode = start_locked(&mut slot, self, repo, released).await?;
        tracing::info!(
            target: "townhall::app",
            event = "daemon.restart",
            repo = %repo.display(),
            duration_ms = began.elapsed().as_millis() as u64,
            "daemon restarted"
        );
        Ok(mode)
    }

    pub async fn stop(&self) {
        let mut slot = self.slot.lock().await;
        stop_locked(&mut slot).await;
    }
}

/// The OS can keep the port busy for a moment after our own server closed it.
/// The listener once the port binds again, so the caller can keep it and nobody else slips in.
/// `None` when the port stayed busy.
async fn wait_released(addr: SocketAddr) -> Option<TcpListener> {
    for _ in 0..REBIND_ATTEMPTS {
        match TcpListener::bind(addr).await {
            Ok(l) => return Some(l),
            Err(_) => tokio::time::sleep(REBIND_DELAY).await,
        }
    }
    None
}

async fn start_locked(
    slot: &mut Slot,
    c: &Controller,
    repo: &Path,
    held: Option<TcpListener>,
) -> Result<Mode, String> {
    let addr = c.addr;
    let listener = match held {
        Some(l) => l,
        None => match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) if e.kind() == ErrorKind::AddrInUse => {
                slot.mode = Mode::External;
                log_mode(Mode::External);
                return Ok(Mode::External);
            }
            Err(e) => return Err(format!("cannot bind {addr}: {e}")),
        },
    };
    let store = c
        .store
        .clone()
        .ok_or("townhall.db is in use by another townhall")?;
    let workspace = Workspace::new(c.home.clone(), repo).map_err(|e| e.to_string())?;
    let (tx, rx) = oneshot::channel::<()>();
    let state = AppState::with_store(workspace, ContextOptions::for_addr(addr), store);
    let background = start_background(&state).await;
    let app = router_with_cors(state);
    let handle = tauri::async_runtime::spawn(async move {
        let shutdown = async {
            let _ = rx.await;
        };
        if let Err(e) = serve(listener, app, shutdown).await {
            tracing::error!(
                target: "townhall::app",
                event = "daemon.serve_failed",
                error = %redact(&e.to_string()),
                "daemon server stopped with an error"
            );
        }
    });
    slot.running = Some(Running {
        shutdown: tx,
        handle,
        background,
    });
    slot.mode = Mode::Embedded;
    log_mode(Mode::Embedded);
    Ok(Mode::Embedded)
}

async fn stop_locked(slot: &mut Slot) {
    if let Some(Running {
        shutdown,
        mut handle,
        background,
    }) = slot.running.take()
    {
        background.stop();
        // A send error means the server task already ended.
        let _ = shutdown.send(());
        // Open websockets can hold a graceful shutdown forever, so cap the wait.
        if tokio::time::timeout(STOP_GRACE, &mut handle).await.is_err() {
            tracing::warn!(
                target: "townhall::app",
                event = "daemon.stop_timeout",
                timeout_ms = STOP_GRACE.as_millis() as u64,
                "daemon shutdown timed out, aborting server task"
            );
            handle.abort();
            let _ = handle.await;
        }
    }
    slot.mode = Mode::Stopped;
    log_mode(Mode::Stopped);
}

fn log_mode(mode: Mode) {
    let mode = match mode {
        Mode::Embedded => "embedded",
        Mode::External => "external",
        Mode::Stopped => "stopped",
    };
    tracing::info!(target: "townhall::app", event = "daemon.mode", mode, "daemon mode is {mode}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use townhall_core::events::EventBus;

    // One test, in this order: the adopt scenario runs last and never feeds its port to the
    // lifecycle scenario. The lifecycle port is bound once by the test and handed over, never
    // probed and released, so another process cannot take it before the daemon does.
    #[tokio::test(flavor = "multi_thread")]
    async fn lifecycle_then_adopt_existing() {
        let (capture, _guard) = townhall_daemon::logging::test_capture();
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path().join(".townhall"));
        let store = Store::open(&home, EventBus::new()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let c = Controller::new(addr, home.clone(), Some(store));
        assert_eq!(
            c.start_on(listener, dir.path()).await.unwrap(),
            Mode::Embedded
        );
        assert!(
            TcpListener::bind(addr).await.is_err(),
            "port should be held"
        );
        assert_eq!(c.restart(dir.path()).await.unwrap(), Mode::Embedded);
        c.stop().await;
        assert_eq!(c.mode().await, Mode::Stopped);
        assert!(
            wait_released(addr).await.is_some(),
            "port released after stop"
        );

        let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let c = Controller::new(taken.local_addr().unwrap(), home, None);
        assert_eq!(c.start(dir.path()).await.unwrap(), Mode::External);
        let err = c.restart(dir.path()).await.unwrap_err();
        assert!(err.contains("already running"), "{err}");

        let modes: Vec<String> = capture
            .events("daemon.mode")
            .iter()
            .map(|l| l["mode"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            modes.first().map(String::as_str),
            Some("embedded"),
            "{modes:?}"
        );
        assert_eq!(
            modes.last().map(String::as_str),
            Some("external"),
            "{modes:?}"
        );
        assert!(modes.contains(&"stopped".to_string()), "{modes:?}");
        let restarts = capture.events("daemon.restart");
        assert!(!restarts.is_empty());
        assert!(restarts[0]["duration_ms"].is_number());
        assert!(restarts[0]["repo"].is_string());
    }
}
