use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
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
        start_locked(&mut slot, self, repo).await
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
        stop_locked(&mut slot).await;
        wait_released(self.addr).await;
        start_locked(&mut slot, self, repo).await
    }

    pub async fn stop(&self) {
        let mut slot = self.slot.lock().await;
        stop_locked(&mut slot).await;
    }
}

/// The OS can keep the port busy for a moment after our own server closed it.
/// True once the port binds again, false when it stayed busy.
async fn wait_released(addr: SocketAddr) -> bool {
    for _ in 0..REBIND_ATTEMPTS {
        match TcpListener::bind(addr).await {
            Ok(_) => return true,
            Err(_) => tokio::time::sleep(REBIND_DELAY).await,
        }
    }
    false
}

async fn start_locked(slot: &mut Slot, c: &Controller, repo: &Path) -> Result<Mode, String> {
    let addr = c.addr;
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            slot.mode = Mode::External;
            return Ok(Mode::External);
        }
        Err(e) => return Err(format!("cannot bind {addr}: {e}")),
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
            eprintln!("townhall-app: {e}");
        }
    });
    slot.running = Some(Running {
        shutdown: tx,
        handle,
        background,
    });
    slot.mode = Mode::Embedded;
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
            eprintln!("townhall-app: daemon shutdown timed out, aborting server task");
            handle.abort();
            let _ = handle.await;
        }
    }
    slot.mode = Mode::Stopped;
}

#[cfg(test)]
mod tests {
    use super::*;
    use townhall_core::events::EventBus;

    fn free_addr() -> SocketAddr {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    }

    /// A probed ephemeral port can be grabbed by another process before we bind it, or while
    /// a restart has it released (seen on macOS), which reads as External. Retry with a
    /// fresh port until a start and a restart both stay embedded.
    async fn embedded_through_a_restart(
        home: &Home,
        store: &Store,
        repo: &Path,
    ) -> (SocketAddr, Controller) {
        for _ in 0..10 {
            let addr = free_addr();
            let c = Controller::new(addr, home.clone(), Some(store.clone()));
            if c.start(repo).await.unwrap() != Mode::Embedded {
                continue;
            }
            assert!(
                TcpListener::bind(addr).await.is_err(),
                "port should be held"
            );
            if c.restart(repo).await.unwrap() == Mode::Embedded {
                return (addr, c);
            }
            c.stop().await;
        }
        panic!("no port stayed ours through a restart after 10 tries");
    }

    // One test, in this order: re-binding a port a test just released is unreliable on macOS,
    // so the adopt scenario runs last and never feeds its port to the lifecycle scenario.
    #[tokio::test(flavor = "multi_thread")]
    async fn lifecycle_then_adopt_existing() {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path().join(".townhall"));
        let store = Store::open(&home, EventBus::new()).unwrap();
        let (addr, c) = embedded_through_a_restart(&home, &store, dir.path()).await;
        c.stop().await;
        assert_eq!(c.mode().await, Mode::Stopped);
        assert!(wait_released(addr).await, "port released after stop");

        let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let c = Controller::new(taken.local_addr().unwrap(), home, None);
        assert_eq!(c.start(dir.path()).await.unwrap(), Mode::External);
        let err = c.restart(dir.path()).await.unwrap_err();
        assert!(err.contains("already running"), "{err}");
    }
}
