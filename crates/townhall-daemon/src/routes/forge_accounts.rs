//! `/api/forge-accounts/detected`: the accounts `gh` and `glab` are logged in with.

use axum::extract::State;
use axum::response::Response;
use townhall_core::repos::detect::detect_blocking;
use townhall_core::repos::publish::Programs;
use townhall_core::Error;

use crate::envelope::ok;
use crate::error::ApiError;
use crate::state::AppState;

/// Runs `gh auth status` and `glab auth status`; never returns a token. A missing program or
/// no login gives an empty list.
pub async fn detected(State(state): State<AppState>) -> Result<Response, ApiError> {
    let registered = state.ctx().settings().await?.forge_accounts;
    let found =
        tokio::task::spawn_blocking(move || detect_blocking(&Programs::from_env(), &registered))
            .await
            .map_err(|e| Error::InvalidInput(format!("detecting accounts failed: {e}")))??;
    Ok(ok(found))
}
