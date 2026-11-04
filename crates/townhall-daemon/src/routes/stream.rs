use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::Response;
use townhall_core::agent;

use crate::error::ApiError;
use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Validates the agent exists (404/400 before upgrade), then streams pane changes.
pub async fn stream(
    State(state): State<AppState>,
    Path(name): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let first = agent::output(state.workspace(), &name).await?;
    Ok(ws.on_upgrade(move |socket| pump(socket, state, name, first)))
}

async fn pump(mut socket: WebSocket, state: AppState, name: String, first: String) {
    let mut last = first;
    if socket.send(Message::text(last.clone())).await.is_err() {
        return;
    }
    let mut tick = tokio::time::interval(POLL_INTERVAL);
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            },
            _ = tick.tick() => match agent::output(state.workspace(), &name).await {
                Ok(content) if content != last => {
                    if socket.send(Message::text(content.clone())).await.is_err() {
                        return;
                    }
                    last = content;
                }
                Ok(_) => {}
                Err(e) => {
                    // Session is gone or capture failed: tell the client, then close.
                    let _ = socket.send(Message::text(format!("error: {e}"))).await;
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            },
        }
    }
}
