use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use townhall_daemon::{router_with_cors, serve, AppState};

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
}

struct Slot {
    mode: Mode,
    running: Option<Running>,
}

/// Owns the in-process daemon. Stopping it only closes the HTTP server:
/// agents live in tmux and are never touched.
pub struct Controller {
    addr: SocketAddr,
    slot: Mutex<Slot>,
}

impl Controller {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
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
        start_locked(&mut slot, self.addr, repo).await
    }

    /// Marks the daemon external when something already listens on the port.
    pub async fn detect_external(&self) -> Mode {
        let mut slot = self.slot.lock().await;
        if slot.mode == Mode::Stopped {
            if let Err(e) = TcpListener::bind(self.addr).await {
                if e.kind() == ErrorKind::AddrInUse {
                    slot.mode = Mode::External;
                }
            }
        }
        slot.mode
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
        start_locked(&mut slot, self.addr, repo).await
    }

    pub async fn stop(&self) {
        let mut slot = self.slot.lock().await;
        stop_locked(&mut slot).await;
    }
}

/// The OS can keep the port busy for a moment after our own server closed it.
async fn wait_released(addr: SocketAddr) {
    for _ in 0..REBIND_ATTEMPTS {
        match TcpListener::bind(addr).await {
            Ok(_) => return,
            Err(_) => tokio::time::sleep(REBIND_DELAY).await,
        }
    }
}

async fn start_locked(slot: &mut Slot, addr: SocketAddr, repo: &Path) -> Result<Mode, String> {
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            slot.mode = Mode::External;
            return Ok(Mode::External);
        }
        Err(e) => return Err(format!("cannot bind {addr}: {e}")),
    };
    let (tx, rx) = oneshot::channel::<()>();
    let app = router_with_cors(AppState::new(repo.to_path_buf()));
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
    });
    slot.mode = Mode::Embedded;
    Ok(Mode::Embedded)
}

async fn stop_locked(slot: &mut Slot) {
    if let Some(Running {
        shutdown,
        mut handle,
    }) = slot.running.take()
    {
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

    fn free_addr() -> SocketAddr {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    }

    // One test, in this order: re-binding a port a test just released is unreliable on macOS,
    // so the adopt scenario runs last and never feeds its port to the lifecycle scenario.
    #[tokio::test(flavor = "multi_thread")]
    async fn lifecycle_then_adopt_existing() {
        let dir = tempfile::tempdir().unwrap();
        // A probed ephemeral port can be grabbed by another process before we bind it
        // (seen on macOS), which reads as External. Retry with a fresh port.
        let (addr, c) = {
            let mut found = None;
            for _ in 0..10 {
                let addr = free_addr();
                let c = Controller::new(addr);
                if c.start(dir.path()).await.unwrap() == Mode::Embedded {
                    found = Some((addr, c));
                    break;
                }
            }
            found.expect("no free port after 10 tries")
        };
        assert!(
            TcpListener::bind(addr).await.is_err(),
            "port should be held"
        );
        assert_eq!(c.restart(dir.path()).await.unwrap(), Mode::Embedded);
        c.stop().await;
        assert_eq!(c.mode().await, Mode::Stopped);
        TcpListener::bind(addr)
            .await
            .expect("port released after stop");

        let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let c = Controller::new(taken.local_addr().unwrap());
        assert_eq!(c.start(dir.path()).await.unwrap(), Mode::External);
        let err = c.restart(dir.path()).await.unwrap_err();
        assert!(err.contains("already running"), "{err}");
    }
}
