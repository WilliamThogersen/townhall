//! `GET /api/usage`: Claude plan usage, read with Claude Code's own token.

use axum::extract::State;
use axum::response::Response;

use crate::envelope::ok;
use crate::state::AppState;

/// `{ session, weekly, updatedAt, error }`. Always `200`: a missing sign-in or a failed
/// refresh is the `error` field, next to the last numbers that were read.
pub async fn get(State(state): State<AppState>) -> Response {
    ok(state.usage().read().await)
}
