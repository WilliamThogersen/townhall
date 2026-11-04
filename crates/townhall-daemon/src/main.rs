use std::process::ExitCode;

use clap::Parser;
use townhall_core::{ContextOptions, Home, Workspace};
use townhall_daemon::config::Config;
use townhall_daemon::{router_with_cors, serve, start_background, AppState};

#[tokio::main]
async fn main() -> ExitCode {
    match run(Config::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: Config) -> Result<(), String> {
    let home = Home::from_env().map_err(|e| format!("townhall home: {e}"))?;
    let workspace = Workspace::new(home, &config.repo).map_err(|e| e.to_string())?;
    let (repo, root) = (
        workspace.repo().display().to_string(),
        workspace.home().root().display().to_string(),
    );
    let state = AppState::open(workspace, ContextOptions::for_addr(config.bind))
        .map_err(|e| e.to_string())?;
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("bind {}: {e}", config.bind))?;
    eprintln!(
        "townhall-daemon: repo {repo} (home {root}) listening on {}",
        config.bind
    );
    let background = start_background(&state).await;
    let served = serve(listener, router_with_cors(state), stop_signal()).await;
    background.stop();
    served
}

/// Ctrl-C or SIGTERM (`kill <pid>`): either runs the clean shutdown, which stops the
/// background units and so every running turn of townhall's own session.
async fn stop_signal() {
    let interrupt = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("townhall-daemon: ctrl-c handler failed: {e}");
            std::future::pending::<()>().await;
        }
    };
    tokio::select! {
        () = interrupt => {}
        () = terminate() => {}
    }
}

#[cfg(unix)]
async fn terminate() {
    use tokio::signal::unix::{signal, SignalKind};
    match signal(SignalKind::terminate()) {
        Ok(mut term) => {
            term.recv().await;
        }
        Err(e) => {
            eprintln!("townhall-daemon: SIGTERM handler failed: {e}");
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(not(unix))]
async fn terminate() {
    std::future::pending::<()>().await;
}
