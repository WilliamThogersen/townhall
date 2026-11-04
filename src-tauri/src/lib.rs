//! Townhall desktop shell: hosts the daemon in-process and shows the web UI.

mod commands;
mod daemon;
mod repo;
mod settings;
mod tray;

use tauri::{Manager, RunEvent, WindowEvent};

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
        Err(e) => eprintln!("townhall-app: cannot extend PATH: {e}"),
    }
}

pub fn run() {
    extend_path();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::repo_status,
            commands::choose_repo
        ])
        .setup(|app| {
            let shared = commands::build_shared(app.handle())?;
            app.manage(shared.clone());
            tray::build(app.handle())?;
            // Starting is just a bind, so finish it before the UI asks for status.
            if let Err(e) = tauri::async_runtime::block_on(commands::start_saved(&shared)) {
                eprintln!("townhall-app: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Close-to-tray: hide instead of closing. Quit lives in the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(e) = window.hide() {
                    eprintln!("townhall-app: cannot hide window: {e}");
                }
            }
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("townhall-app: failed to build app: {e}");
            std::process::exit(1);
        });

    app.run(|handle, event| match event {
        RunEvent::Exit => {
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
