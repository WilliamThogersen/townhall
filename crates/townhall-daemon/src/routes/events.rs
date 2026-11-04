//! The `/api/events` websocket (unit 2.2): forwards the event bus as JSON messages.
//! CORS does not cover websockets, so a foreign `Origin` is a `403` before the upgrade.

use std::time::Duration;

use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use townhall_core::model::ServerEvent;

use crate::cors::origin_allowed;
use crate::envelope::failure;
use crate::state::AppState;

pub const FOREIGN_ORIGIN: &str = "origin not allowed";
pub const NOT_UPGRADED: &str = "this route needs a websocket upgrade";
const CLOSE_WAIT: Duration = Duration::from_secs(5);

pub async fn socket(
    State(state): State<AppState>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if !origin_allowed(&headers) {
        return failure(StatusCode::FORBIDDEN, FOREIGN_ORIGIN.into());
    }
    let ws = match upgrade {
        Ok(ws) => ws,
        Err(rejection) => return failure(rejection.status(), NOT_UPGRADED.into()),
    };
    let events = state.ctx().bus().subscribe();
    ws.on_upgrade(move |socket| forward(socket, events))
}

fn frame(event: &ServerEvent) -> Option<Message> {
    match serde_json::to_string(event) {
        Ok(text) => Some(Message::text(text)),
        Err(e) => {
            eprintln!("warn: cannot send an event: {e}");
            None
        }
    }
}

/// Sends each bus event, `resync` when this socket fell behind, until either side ends.
async fn forward(mut socket: WebSocket, mut events: Receiver<ServerEvent>) {
    loop {
        let next = tokio::select! {
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) => return,
                Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => continue,
            },
            event = events.recv() => match event {
                Ok(event) => frame(&event),
                Err(RecvError::Lagged(_)) => frame(&ServerEvent::Resync),
                Err(RecvError::Closed) => {
                    if socket.send(Message::Close(None)).await.is_err() {
                        return;
                    }
                    break;
                }
            },
        };
        let Some(message) = next else { continue };
        if socket.send(message).await.is_err() {
            return;
        }
    }
    finish_close(socket).await;
}

/// Reading on after a close sends the close reply; the read ends once it went out.
async fn finish_close(mut socket: WebSocket) {
    let drain = async { while let Some(Ok(_)) = socket.recv().await {} };
    if tokio::time::timeout(CLOSE_WAIT, drain).await.is_err() {
        eprintln!("warn: an events socket did not close in time");
    }
}
