use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::{oneshot, Mutex};
use townhall_core::events::EventBus;
use townhall_core::store::Store;
use townhall_core::{Error, Home};

use crate::daemon::{Controller, Mode};
use crate::repo::validate_repo;
use crate::settings::{self, Settings};

/// Shared app state managed by Tauri.
pub struct Shared {
    pub controller: Controller,
    pub settings: Mutex<Settings>,
    /// `None` when another townhall (a daemon started on its own) holds the database.
    pub store: Option<Store>,
    pub home: Home,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoStatus {
    pub repo: Option<String>,
    pub mode: Mode,
}

pub type SharedState = Arc<Shared>;

async fn status(shared: &Shared) -> RepoStatus {
    let repo = shared
        .settings
        .lock()
        .await
        .repo
        .as_ref()
        .map(|p| p.display().to_string());
    RepoStatus {
        repo,
        mode: shared.controller.mode().await,
    }
}

/// Starts the daemon at launch: on the saved repo, else with no repo at all, so the page
/// opens on home's first run. A saved repo that is no longer usable is reported and skipped.
pub async fn start_saved(shared: &Shared) -> Result<(), String> {
    let saved = shared.settings.lock().await.repo.clone();
    let folder = match saved.map(|repo| validate_repo(&repo)) {
        Some(Ok(repo)) => repo,
        Some(Err(e)) => {
            eprintln!("townhall-app: warn: saved repo is no longer usable: {e}");
            no_repo_folder(&shared.home)?
        }
        None => no_repo_folder(&shared.home)?,
    };
    shared.controller.start(&folder).await.map(|_| ())
}

/// The daemon always starts in some folder. Townhall's own home is never a git repo, so the
/// daemon registers nothing for it and serves no repo until one is added.
fn no_repo_folder(home: &Home) -> Result<PathBuf, String> {
    home.ensure().map_err(|e| format!("townhall home: {e}"))?;
    Ok(home.root().to_path_buf())
}

/// Applies a new repo: validate, restart the daemon, then persist.
/// On a failed restart the previous repo is restored best-effort.
pub async fn apply_repo(shared: &Shared, path: &Path) -> Result<(), String> {
    let repo = validate_repo(path)?;
    let previous = shared.settings.lock().await.repo.clone();
    let result = match shared.controller.mode().await {
        Mode::Stopped => shared.controller.start(&repo).await,
        _ => shared.controller.restart(&repo).await,
    };
    match result {
        // An existing daemon owns the port and its own repo; nothing to persist.
        Ok(Mode::External) => return Ok(()),
        Ok(_) => {}
        Err(e) => {
            if let Some(prev) = previous {
                // Best effort: bring the old repo back so the UI keeps working.
                let _ = shared.controller.restart(&prev).await;
            }
            return Err(e);
        }
    }
    let next = Settings { repo: Some(repo) };
    let store = shared
        .store
        .as_ref()
        .ok_or("townhall.db is in use by another townhall")?;
    settings::save(store, &next).await?;
    *shared.settings.lock().await = next;
    Ok(())
}

#[tauri::command]
pub async fn repo_status(state: State<'_, SharedState>) -> Result<RepoStatus, String> {
    Ok(status(&state).await)
}

/// The native folder picker. `None` when it was cancelled.
async fn ask_folder(app: &AppHandle, title: &str) -> Result<Option<PathBuf>, String> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().set_title(title).pick_folder(move |picked| {
        let _ = tx.send(picked);
    });
    let picked = rx
        .await
        .map_err(|_| "folder dialog closed unexpectedly".to_string())?;
    picked
        .map(|file| {
            file.into_path()
                .map_err(|e| format!("invalid selection: {e}"))
        })
        .transpose()
}

/// Opens the native folder picker. Cancelling leaves everything unchanged.
#[tauri::command]
pub async fn choose_repo(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<RepoStatus, String> {
    if let Some(path) = ask_folder(&app, "Choose the git repository agents work in").await? {
        apply_repo(&state, &path).await?;
    }
    Ok(status(&state).await)
}

/// The folder picker behind "add repository…": only picks. The page adds the folder with
/// `POST /api/repos`, which checks it, so the running daemon never restarts.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    let picked = ask_folder(&app, "add repository").await?;
    Ok(picked.map(|path| path.display().to_string()))
}

/// Opens the store, unless another townhall holds it: then the app uses that daemon.
fn open_store(home: &Home) -> Result<Option<Store>, String> {
    match Store::open(home, EventBus::new()) {
        Ok(store) => Ok(Some(store)),
        Err(Error::Unavailable(msg)) => {
            eprintln!("townhall-app: info: {msg}, using the daemon that holds it");
            Ok(None)
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn build_shared(app: &AppHandle) -> Result<SharedState, String> {
    let home = Home::from_env().map_err(|e| format!("townhall home: {e}"))?;
    let legacy_dir = app.path().app_config_dir().ok();
    let store = open_store(&home)?;
    let loaded = match &store {
        Some(store) => {
            tauri::async_runtime::block_on(settings::load(store, legacy_dir.as_deref()))?
        }
        None => Settings::default(),
    };
    let addr = crate::daemon::BIND
        .parse()
        .map_err(|e| format!("bind address: {e}"))?;
    Ok(Arc::new(Shared {
        controller: Controller::new(addr, home.clone(), store.clone()),
        settings: Mutex::new(loaded),
        store,
        home,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::process::Command;
    use townhall_core::events::EventBus;

    fn free_addr() -> SocketAddr {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    }

    fn shared_at(dir: &Path, addr: SocketAddr, saved: Option<PathBuf>) -> Shared {
        let home = Home::at(dir.join(".townhall"));
        let store = Store::open(&home, EventBus::new()).unwrap();
        Shared {
            controller: Controller::new(addr, home.clone(), Some(store.clone())),
            settings: Mutex::new(Settings { repo: saved }),
            store: Some(store),
            home,
        }
    }

    /// A probed port can be taken by another process before the daemon binds it, which
    /// reads as External: retry on a fresh port until the daemon is our own.
    async fn started(dir: &Path, saved: Option<PathBuf>) -> (SocketAddr, Shared) {
        for _ in 0..10 {
            let addr = free_addr();
            let shared = shared_at(dir, addr, saved.clone());
            start_saved(&shared).await.unwrap();
            if shared.controller.mode().await == Mode::Embedded {
                return (addr, shared);
            }
        }
        panic!("no port stayed ours after 10 tries");
    }

    async fn get(addr: SocketAddr, path: &'static str) -> String {
        tokio::task::spawn_blocking(move || {
            let mut stream = TcpStream::connect(addr).unwrap();
            write!(
                stream,
                "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            let mut reply = String::new();
            stream.read_to_string(&mut reply).unwrap();
            reply
        })
        .await
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn without_a_saved_repo_the_daemon_serves_no_repo() {
        let dir = tempfile::tempdir().unwrap();
        let (addr, shared) = started(dir.path(), None).await;
        assert!(get(addr, "/api/repos").await.contains(r#""data":[]"#));
        let home = shared.home.root().canonicalize().unwrap();
        let info = get(addr, "/api/info").await;
        assert!(info.contains(&home.display().to_string()), "{info}");
        shared.controller.stop().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_saved_repo_that_is_gone_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone");
        let (addr, shared) = started(dir.path(), Some(gone)).await;
        assert!(get(addr, "/api/repos").await.contains(r#""data":[]"#));
        shared.controller.stop().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_saved_repo_is_served() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("shop");
        let init = Command::new("git").arg("init").arg(&repo).output().unwrap();
        assert!(init.status.success());
        let (addr, shared) = started(dir.path(), Some(repo)).await;
        assert!(get(addr, "/api/repos").await.contains(r#""name":"shop""#));
        shared.controller.stop().await;
    }
}
