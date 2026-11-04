use std::process::ExitCode;

use castle_daemon::config::Config;
use castle_daemon::{router, AppState};
use clap::Parser;

#[tokio::main]
async fn main() -> ExitCode {
    match run(Config::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("castle-daemon: {msg}");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: Config) -> Result<(), String> {
    let repo = config
        .repo
        .canonicalize()
        .map_err(|e| format!("repo path {}: {e}", config.repo.display()))?;
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("bind {}: {e}", config.bind))?;
    eprintln!("castle-daemon: repo {} listening on {}", repo.display(), config.bind);
    axum::serve(listener, router(AppState::new(repo)))
        .with_graceful_shutdown(async {
            if let Err(e) = tokio::signal::ctrl_c().await {
                eprintln!("castle-daemon: signal handler failed: {e}");
            }
        })
        .await
        .map_err(|e| format!("server: {e}"))
}
