use std::path::Path;
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

/// Starts the daemon at launch when a repo is already saved.
pub async fn start_saved(shared: &Shared) -> Result<(), String> {
    let saved = shared.settings.lock().await.repo.clone();
    let Some(repo) = saved else {
        shared.controller.detect_external().await;
        return Ok(());
    };
    let repo = validate_repo(&repo).map_err(|e| format!("saved repo is no longer usable: {e}"))?;
    shared.controller.start(&repo).await.map(|_| ())
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

/// Opens the native folder picker. Cancelling leaves everything unchanged.
#[tauri::command]
pub async fn choose_repo(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<RepoStatus, String> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose the git repository agents work in")
        .pick_folder(move |picked| {
            let _ = tx.send(picked);
        });
    let picked = rx
        .await
        .map_err(|_| "folder dialog closed unexpectedly".to_string())?;
    if let Some(file) = picked {
        let path = file
            .into_path()
            .map_err(|e| format!("invalid selection: {e}"))?;
        apply_repo(&state, &path).await?;
    }
    Ok(status(&state).await)
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
        controller: Controller::new(addr, home, store.clone()),
        settings: Mutex::new(loaded),
        store,
    }))
}
