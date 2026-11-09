//! `/api/forge-accounts/detected`: the accounts `gh` and `glab` are logged in with.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use townhall_core::repos::detect::{detect_blocking, Detected};
use townhall_core::repos::publish::Programs;
use townhall_core::Error;

use crate::error::ApiError;
use crate::state::AppState;

/// The usual `{success, data, error}` envelope, with `warnings` beside `data` only when a
/// program yielded less than expected, so clients that read `data` alone keep working.
#[derive(Serialize)]
struct DetectedEnvelope {
    success: bool,
    data: Vec<Detected>,
    error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

/// Reads `gh auth status` and glab's config file; never returns a token. A missing program or
/// no login gives an empty list, and `warnings` says why.
pub async fn detected(State(state): State<AppState>) -> Result<Response, ApiError> {
    let registered = state.ctx().settings().await?.forge_accounts;
    let found =
        tokio::task::spawn_blocking(move || detect_blocking(&Programs::from_env(), &registered))
            .await
            .map_err(|e| Error::InvalidInput(format!("detecting accounts failed: {e}")))?;
    Ok((
        StatusCode::OK,
        Json(DetectedEnvelope {
            success: true,
            data: found.accounts,
            error: None,
            warnings: found.warnings,
        }),
    )
        .into_response())
}
