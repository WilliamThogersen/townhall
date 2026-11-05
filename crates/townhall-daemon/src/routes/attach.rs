//! The `/api/agents/{name}/attach` websocket: a real terminal on the agent's tmux session.
//! It runs `tmux attach-session` in a PTY and pipes it both ways. Closing the socket ends
//! only that attach client; the session and the agent keep running. This socket types
//! into a shell, so a foreign `Origin` is a `403` before the upgrade, as for `/events`.

use std::io::{Read, Write};
use std::sync::mpsc as std_mpsc;

use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc;
use townhall_core::redact::redact;
use townhall_core::{agent, fleet};

use super::agents::RepoQuery;
use super::attach_frames::{read, Frame};
use super::events::{FOREIGN_ORIGIN, NOT_UPGRADED};
use crate::cors::origin_allowed;
use crate::envelope::failure;
use crate::error::ApiError;
use crate::middleware::WsSession;
use crate::state::AppState;

const START_SIZE: PtySize = PtySize {
    rows: 24,
    cols: 80,
    pixel_width: 0,
    pixel_height: 0,
};
const CHUNK: usize = 16 * 1024;
const CLOSE_NORMAL: u16 = 1000;
const CLOSE_ERROR: u16 = 1011;
const SESSION_ENDED: &str = "session ended";

/// Checks origin, upgrade, agent and live session (403/404/400/409 before upgrade).
pub async fn attach(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<RepoQuery>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response, ApiError> {
    if !origin_allowed(&headers) {
        return Ok(failure(StatusCode::FORBIDDEN, FOREIGN_ORIGIN.into()));
    }
    let ws = match upgrade {
        Ok(ws) => ws,
        Err(rejection) => return Ok(failure(rejection.status(), NOT_UPGRADED.into())),
    };
    let at = fleet::locate(state.ctx(), &name, q.repo.as_deref()).await?;
    let args = agent::attach_args(&at.workspace, &name).await?;
    let session = WsSession::open("/api/agents/{name}/attach", &headers, Some(&name));
    Ok(ws.on_upgrade(move |socket| async move {
        let _session = session;
        run(socket, args).await
    }))
}

/// An attach client running in a PTY.
struct Pty {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    output: mpsc::Receiver<Vec<u8>>,
    input: std_mpsc::Sender<Vec<u8>>,
}

fn open_error(e: impl std::fmt::Display) -> String {
    format!("cannot open the terminal: {e}")
}

/// Starts `tmux args...` in a PTY with a reader and a writer thread.
fn open(args: &[String]) -> Result<Pty, String> {
    let master = native_pty_system()
        .openpty(START_SIZE)
        .map_err(open_error)?;
    let mut cmd = CommandBuilder::new("tmux");
    cmd.args(args);
    cmd.env("TERM", "xterm-256color");
    cmd.env_remove("TMUX");
    let child = master.slave.spawn_command(cmd).map_err(open_error)?;
    let mut reader = master.master.try_clone_reader().map_err(open_error)?;
    let mut writer = master.master.take_writer().map_err(open_error)?;
    let (out_tx, output) = mpsc::channel(64);
    std::thread::spawn(move || {
        let mut buf = vec![0u8; CHUNK];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || out_tx.blocking_send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let (input, in_rx) = std_mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        while let Ok(bytes) = in_rx.recv() {
            if writer
                .write_all(&bytes)
                .and_then(|_| writer.flush())
                .is_err()
            {
                break;
            }
        }
    });
    Ok(Pty {
        master: master.master,
        child,
        output,
        input,
    })
}

impl Pty {
    fn resize(&self, cols: u16, rows: u16) {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        if let Err(e) = self.master.resize(size) {
            tracing::warn!(
                target: "townhall::http",
                event = "attach.resize_failed",
                error = %redact(&e.to_string()),
                "cannot resize the terminal"
            );
        }
    }

    /// Ends the attach client (a detach) and reaps it. The session is not touched.
    fn finish(mut self) {
        let _ = self.child.kill();
        tokio::task::spawn_blocking(move || {
            let _ = self.child.wait();
        });
    }
}

fn close(code: u16, reason: &str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}

/// Pipes the PTY and the socket until either side ends.
async fn run(mut socket: WebSocket, args: Vec<String>) {
    let mut pty = match open(&args) {
        Ok(pty) => pty,
        Err(reason) => {
            tracing::warn!(
                target: "townhall::http",
                event = "attach.open_failed",
                error = %redact(&reason),
                "cannot open the terminal"
            );
            let _ = socket
                .send(close(CLOSE_ERROR, "cannot open the terminal"))
                .await;
            return;
        }
    };
    let ended = pipe(&mut socket, &mut pty).await;
    if ended {
        let _ = socket.send(close(CLOSE_NORMAL, SESSION_ENDED)).await;
    }
    pty.finish();
}

/// True when the PTY ended (the session is gone), false when the client left or failed.
async fn pipe(socket: &mut WebSocket, pty: &mut Pty) -> bool {
    loop {
        tokio::select! {
            out = pty.output.recv() => match out {
                None => return true,
                Some(bytes) => {
                    if socket.send(Message::Binary(bytes.into())).await.is_err() {
                        return false;
                    }
                }
            },
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) => return false,
                Some(Ok(message)) => match read(message) {
                    Frame::Close => return false,
                    Frame::Ignore => {}
                    Frame::Resize { cols, rows } => pty.resize(cols, rows),
                    Frame::Input(bytes) => {
                        if pty.input.send(bytes).is_err() {
                            return true;
                        }
                    }
                },
            },
        }
    }
}
