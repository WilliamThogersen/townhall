use axum::extract::State;
use axum::response::Response;
use serde::Serialize;
use townhall_core::cmd::run;

use crate::envelope::ok;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoView {
    home: String,
    repo: String,
    repo_key: String,
    /// `git config user.name`, for home's greeting; `null` when unset.
    user_name: Option<String>,
    /// The build's version, with `+<sha>` on dev builds.
    version: &'static str,
}

/// Where townhall keeps things for the startup repo, and who you are.
pub async fn info(State(state): State<AppState>) -> Response {
    let ws = state.workspace();
    let user_name = run("git", &["config", "--get", "user.name"], Some(ws.repo()))
        .await
        .ok()
        .filter(|n| !n.is_empty());
    ok(InfoView {
        home: ws.home().root().to_string_lossy().into_owned(),
        repo: ws.repo().to_string_lossy().into_owned(),
        repo_key: ws.key().to_string(),
        user_name,
        version: townhall_core::version::VERSION,
    })
}
