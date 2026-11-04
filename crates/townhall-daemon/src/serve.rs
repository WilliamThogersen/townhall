use std::future::Future;

use axum::Router;
use tokio::net::TcpListener;

/// Serves `app` on `listener` until `shutdown` resolves.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), String> {
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|e| format!("server: {e}"))
}
