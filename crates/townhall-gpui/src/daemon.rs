use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use townhall_core::events::EventBus;
use townhall_core::redact::redact;
use townhall_core::store::Store;
use townhall_core::{ContextOptions, Error, Home, Workspace};
use townhall_daemon::{router_with_cors, serve, start_background, AppState, Background};

pub const BIND: &str = "127.0.0.1:7777";
const STOP_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Embedded,
    External,
}

struct Running {
    shutdown: oneshot::Sender<()>,
    handle: JoinHandle<()>,
    background: Background,
}

pub struct Daemon {
    mode: Mode,
    addr: SocketAddr,
    runtime: Runtime,
    running: Option<Running>,
}

impl Daemon {
    pub fn start(addr: SocketAddr, home: Home) -> Result<Self, String> {
        Self::start_with(addr, home, None)
    }

    fn start_with(
        addr: SocketAddr,
        home: Home,
        held: Option<std::net::TcpListener>,
    ) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("townhall-daemon")
            .build()
            .map_err(|e| format!("cannot start the daemon runtime: {e}"))?;
        let running = runtime.block_on(start_serving(addr, home, held))?;
        let mode = if running.is_some() {
            Mode::Embedded
        } else {
            Mode::External
        };
        log_mode(mode);
        Ok(Self {
            mode,
            addr,
            runtime,
            running,
        })
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn stop(mut self) {
        if let Some(Running {
            shutdown,
            mut handle,
            background,
        }) = self.running.take()
        {
            background.stop();
            let _ = shutdown.send(());
            self.runtime.block_on(async {
                if tokio::time::timeout(STOP_GRACE, &mut handle).await.is_err() {
                    tracing::warn!(
                        target: "townhall::native",
                        event = "daemon.stop_timeout",
                        timeout_ms = STOP_GRACE.as_millis() as u64,
                        "daemon shutdown timed out, aborting server task"
                    );
                    handle.abort();
                    let _ = handle.await;
                }
            });
        }
        self.runtime.shutdown_timeout(STOP_GRACE);
    }
}

fn open_store(home: &Home) -> Result<Option<Store>, String> {
    match Store::open(home, EventBus::new()) {
        Ok(store) => Ok(Some(store)),
        Err(Error::Unavailable(msg)) => {
            tracing::info!(
                target: "townhall::native",
                event = "store.held_elsewhere",
                error = %redact(&msg),
                "database is held by another townhall, using its daemon"
            );
            Ok(None)
        }
        Err(e) => Err(e.to_string()),
    }
}

async fn startup_folder(store: &Store, home: &Home) -> Result<PathBuf, String> {
    let saved = store.desktop_repo().await.map_err(|e| e.to_string())?;
    match saved {
        Some(repo) if repo.is_dir() => Ok(repo),
        saved => {
            if let Some(repo) = saved {
                tracing::warn!(
                    target: "townhall::native",
                    event = "repo.unusable",
                    repo = %repo.display(),
                    "saved repo is no longer usable"
                );
            }
            home.ensure().map_err(|e| format!("townhall home: {e}"))?;
            Ok(home.root().to_path_buf())
        }
    }
}

async fn start_serving(
    addr: SocketAddr,
    home: Home,
    held: Option<std::net::TcpListener>,
) -> Result<Option<Running>, String> {
    let store = open_store(&home)?;
    let listener = match held {
        Some(l) => {
            l.set_nonblocking(true)
                .map_err(|e| format!("cannot use the bound port: {e}"))?;
            TcpListener::from_std(l).map_err(|e| format!("cannot use the bound port: {e}"))?
        }
        None => match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) if e.kind() == ErrorKind::AddrInUse => return Ok(None),
            Err(e) => return Err(format!("cannot bind {addr}: {e}")),
        },
    };
    let store = store.ok_or("townhall.db is in use by another townhall")?;
    let folder = startup_folder(&store, &home).await?;
    let workspace = Workspace::new(home, &folder).map_err(|e| e.to_string())?;
    let (tx, rx) = oneshot::channel::<()>();
    let state = AppState::with_store(workspace, ContextOptions::for_addr(addr), store);
    let background = start_background(&state).await;
    let app = router_with_cors(state);
    let handle = tokio::spawn(async move {
        let shutdown = async {
            let _ = rx.await;
        };
        if let Err(e) = serve(listener, app, shutdown).await {
            tracing::error!(
                target: "townhall::native",
                event = "daemon.serve_failed",
                error = %redact(&e.to_string()),
                "daemon server stopped with an error"
            );
        }
    });
    Ok(Some(Running {
        shutdown: tx,
        handle,
        background,
    }))
}

fn log_mode(mode: Mode) {
    let mode = match mode {
        Mode::Embedded => "embedded",
        Mode::External => "external",
    };
    tracing::info!(target: "townhall::native", event = "daemon.mode", mode, "daemon mode is {mode}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Api, Client};

    fn home_in(dir: &std::path::Path) -> Home {
        Home::at(dir.join(".townhall"))
    }

    #[test]
    fn serves_in_process_on_the_given_port_and_releases_it_on_stop() {
        let dir = tempfile::tempdir().unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let daemon = Daemon::start_with(addr, home_in(dir.path()), Some(listener)).unwrap();
        assert_eq!(daemon.mode(), Mode::Embedded);
        let client = Client::new(daemon.base_url());
        assert_eq!(client.conversations().unwrap(), vec![]);
        daemon.stop();
        let mut freed = false;
        for _ in 0..40 {
            if std::net::TcpListener::bind(addr).is_ok() {
                freed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(freed, "port should be released after stop");
    }

    #[test]
    fn adopts_a_daemon_already_on_the_port() {
        let dir = tempfile::tempdir().unwrap();
        let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = taken.local_addr().unwrap();
        let daemon = Daemon::start(addr, home_in(dir.path())).unwrap();
        assert_eq!(daemon.mode(), Mode::External);
        assert_eq!(daemon.base_url(), format!("http://{addr}"));
        daemon.stop();
    }

    #[test]
    fn refuses_to_serve_when_another_townhall_holds_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let home = home_in(dir.path());
        let _held = Store::open(&home, EventBus::new()).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let err = match Daemon::start_with(addr, home, Some(listener)) {
            Ok(_) => panic!("started without the database"),
            Err(e) => e,
        };
        assert!(err.contains("in use by another townhall"), "{err}");
    }
}
