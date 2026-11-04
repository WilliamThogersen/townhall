use axum::extract::State;
use axum::response::Response;
use serde::Serialize;

use crate::envelope::ok;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoView {
    home: String,
    repo: String,
    repo_key: String,
}

/// Where townhall keeps things for the current repo.
pub async fn info(State(state): State<AppState>) -> Response {
    let ws = state.workspace();
    ok(InfoView {
        home: ws.home().root().to_string_lossy().into_owned(),
        repo: ws.repo().to_string_lossy().into_owned(),
        repo_key: ws.key().to_string(),
    })
}
