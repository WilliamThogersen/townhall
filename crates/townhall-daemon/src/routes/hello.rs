use axum::response::Response;
use serde::Serialize;

use crate::envelope::ok;

const GREETING: &str = "hello";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloView {
    message: &'static str,
}

/// A fixed greeting, to check the daemon answers.
pub async fn hello() -> Response {
    ok(HelloView { message: GREETING })
}
