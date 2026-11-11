use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;

use crate::envelope::{failure, ok};
use crate::state::AppState;

pub async fn get(State(state): State<AppState>) -> Response {
    match state.system().read() {
        Ok(stats) => ok(stats),
        Err(e) => {
            tracing::error!(target: "townhall::system", error = %e, "system.read_failed");
            failure(StatusCode::INTERNAL_SERVER_ERROR, e)
        }
    }
}
