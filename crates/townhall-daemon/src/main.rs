use std::process::ExitCode;

use clap::Parser;
use townhall_core::{Home, Workspace};
use townhall_daemon::config::Config;
use townhall_daemon::{router_with_cors, serve, AppState};

#[tokio::main]
async fn main() -> ExitCode {
    match run(Config::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("townhall-daemon: {msg}");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: Config) -> Result<(), String> {
    let home = Home::from_env().map_err(|e| format!("townhall home: {e}"))?;
    let workspace = Workspace::new(home, &config.repo).map_err(|e| e.to_string())?;
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("bind {}: {e}", config.bind))?;
    eprintln!(
        "townhall-daemon: repo {} (home {}) listening on {}",
        workspace.repo().display(),
        workspace.home().root().display(),
        config.bind
    );
    let shutdown = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("townhall-daemon: signal handler failed: {e}");
        }
    };
    serve(
        listener,
        router_with_cors(AppState::new(workspace)),
        shutdown,
    )
    .await
}
