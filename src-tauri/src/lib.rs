//! Townhall desktop shell: hosts the daemon in-process and shows the web UI.

mod commands;
mod daemon;
mod repo;
mod settings;
mod tray;

use tauri::{Manager, RunEvent, WindowEvent};
use townhall_core::redact::redact;

/// GUI apps on macOS start with a minimal PATH; agents need git and tmux.
fn extend_path() {
    let extra = ["/opt/homebrew/bin", "/usr/local/bin"];
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut parts: Vec<std::path::PathBuf> = std::env::split_paths(&current).collect();
    for dir in extra {
        let dir = std::path::PathBuf::from(dir);
        if !parts.contains(&dir) {
            parts.push(dir);
        }
    }
    match std::env::join_paths(parts) {
        Ok(joined) => std::env::set_var("PATH", joined),
        Err(e) => tracing::warn!(
            target: "townhall::app",
            event = "path.extend_failed",
            error = %redact(&e.to_string()),
            "cannot extend PATH"
        ),
    }
}

/// Starts file logging once for the whole process (the embedded daemon shares it).
fn init_logging() -> Option<townhall_daemon::logging::Guard> {
    let logs = townhall_core::Home::from_env()
        .and_then(|home| home.logs_dir())
        .map_err(|e| eprintln!("townhall-app: logging disabled: {e}"))
        .ok()?;
    let guard = townhall_daemon::logging::init(townhall_daemon::logging::LogConfig {
        dir: logs,
        prefix: "app".into(),
        stderr_level: tracing::Level::WARN,
    });
    tracing::info!(
        target: "townhall::app",
        event = "app.start",
        version = env!("CARGO_PKG_VERSION"),
        "app started"
    );
    Some(guard)
}

pub fn run() {
    let _log_guard = init_logging();
    extend_path();
    let built = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::repo_status,
            commands::choose_repo,
            commands::pick_folder
        ])
        .setup(|app| {
            let shared = commands::build_shared(app.handle())?;
            app.manage(shared.clone());
            tray::build(app.handle())?;
            // Starting is just a bind, so finish it before the UI asks for status.
            if let Err(e) = tauri::async_runtime::block_on(commands::start_saved(&shared)) {
                tracing::error!(
                    target: "townhall::app",
                    event = "daemon.start_failed",
                    error = %redact(&e),
                    "cannot start the daemon"
                );
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Close-to-tray: hide instead of closing. Quit lives in the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                match window.hide() {
                    Ok(()) => tracing::info!(
                        target: "townhall::app",
                        event = "window.hide",
                        "window hidden to tray"
                    ),
                    Err(e) => tracing::warn!(
                        target: "townhall::app",
                        event = "window.hide_failed",
                        error = %redact(&e.to_string()),
                        "cannot hide window"
                    ),
                }
            }
        })
        .build(tauri::generate_context!());
    let app = match built {
        Ok(app) => app,
        Err(e) => {
            tracing::error!(
                target: "townhall::app",
                event = "app.build_failed",
                error = %redact(&e.to_string()),
                "failed to build app"
            );
            // The non-blocking writer flushes when its guard drops, which exit skips.
            drop(_log_guard);
            std::process::exit(1);
        }
    };

    app.run(|handle, event| match event {
        RunEvent::Exit => {
            tracing::info!(target: "townhall::app", event = "app.exit", "app exiting");
            if let Some(shared) = handle.try_state::<commands::SharedState>() {
                // Stops only the in-process HTTP server. Agents stay alive in tmux.
                tauri::async_runtime::block_on(shared.controller.stop());
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => tray::show_main(handle),
        _ => {}
    });
}
