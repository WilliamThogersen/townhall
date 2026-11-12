use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use townhall_core::model::{Conversation, ConversationDetail, ServerEvent, ThreadEntry};
use tungstenite::Message;

const RECONNECT_FIRST: Duration = Duration::from_millis(500);
const RECONNECT_MAX: Duration = Duration::from_secs(5);

pub trait Api: Send + Sync + 'static {
    fn conversations(&self) -> Result<Vec<Conversation>, String>;
    fn conversation(&self, id: &str) -> Result<ConversationDetail, String>;
    fn send_message(&self, id: &str, text: &str) -> Result<ThreadEntry, String>;
}

#[derive(Clone)]
pub struct Client {
    base: String,
}

impl Client {
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}/api{path}", self.base);
        let result = ureq::get(&url)
            .config()
            .http_status_as_error(false)
            .build()
            .call();
        read_envelope("GET", &url, result)
    }

    fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T, String> {
        let url = format!("{}/api{path}", self.base);
        let result = ureq::post(&url)
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(body);
        read_envelope("POST", &url, result)
    }
}

fn read_envelope<T: DeserializeOwned>(
    method: &str,
    url: &str,
    result: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<T, String> {
    let mut response = result.map_err(|e| format!("{method} {url}: {e}"))?;
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("{method} {url}: cannot read the response: {e}"))?;
    parse_envelope(&body).map_err(|e| format!("{method} {url}: {e}"))
}

fn encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

impl Api for Client {
    fn send_message(&self, id: &str, text: &str) -> Result<ThreadEntry, String> {
        self.post(
            &format!("/conversations/{}/messages", encode_segment(id)),
            &serde_json::json!({ "text": text }),
        )
    }

    fn conversations(&self) -> Result<Vec<Conversation>, String> {
        self.get("/conversations")
    }

    fn conversation(&self, id: &str) -> Result<ConversationDetail, String> {
        self.get(&format!("/conversations/{id}"))
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

pub fn parse_envelope<T: DeserializeOwned>(body: &str) -> Result<T, String> {
    let envelope: Envelope<T> =
        serde_json::from_str(body).map_err(|e| format!("unexpected response: {e}"))?;
    match (envelope.success, envelope.data) {
        (true, Some(data)) => Ok(data),
        _ => Err(envelope.error.unwrap_or_else(|| "no data".into())),
    }
}

pub fn parse_event(text: &str) -> Result<ServerEvent, String> {
    serde_json::from_str(text).map_err(|e| format!("unexpected event: {e}"))
}

pub fn events_url(base: &str) -> Result<String, String> {
    let rest = base
        .strip_prefix("http://")
        .ok_or_else(|| format!("{base} is not an http:// daemon url"))?;
    Ok(format!("ws://{}/api/events", rest.trim_end_matches('/')))
}

pub fn subscribe(base: &str) -> Result<async_channel::Receiver<ServerEvent>, String> {
    let url = events_url(base)?;
    let (tx, rx) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("townhall-events".into())
        .spawn(move || run_events(&url, &tx))
        .map_err(|e| format!("cannot start the events thread: {e}"))?;
    Ok(rx)
}

fn run_events(url: &str, tx: &async_channel::Sender<ServerEvent>) {
    let mut delay = RECONNECT_FIRST;
    let mut lost = false;
    while !tx.is_closed() {
        match tungstenite::connect(url) {
            Ok((mut socket, _)) => {
                delay = RECONNECT_FIRST;
                if lost && tx.send_blocking(ServerEvent::Resync).is_err() {
                    return;
                }
                loop {
                    match socket.read() {
                        Ok(Message::Text(text)) => match parse_event(text.as_str()) {
                            Ok(event) => {
                                if tx.send_blocking(event).is_err() {
                                    return;
                                }
                            }
                            Err(e) => tracing::warn!(
                                target: "townhall::native",
                                event = "events.unparsed",
                                error = %e,
                                "ignoring an event"
                            ),
                        },
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!(
                                target: "townhall::native",
                                event = "events.closed",
                                error = %e,
                                "events socket closed"
                            );
                            break;
                        }
                    }
                }
                lost = true;
            }
            Err(e) => tracing::warn!(
                target: "townhall::native",
                event = "events.connect_failed",
                error = %e,
                "cannot connect to the events socket"
            ),
        }
        std::thread::sleep(delay);
        delay = (delay * 2).min(RECONNECT_MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn conversation_json(id: &str) -> serde_json::Value {
        json!({
            "id": id,
            "sessionId": "s",
            "title": "t",
            "status": "yourTurn",
            "tasks": [],
            "linkedRepo": null,
            "ended": false,
            "createdAt": "2026-11-12T10:00:00.000Z",
            "updatedAt": "2026-11-12T10:00:00.000Z"
        })
    }

    #[test]
    fn envelope_with_data_parses_into_model_types() {
        let body = json!({"success": true, "data": [conversation_json("c1")]}).to_string();
        let list: Vec<Conversation> = parse_envelope(&body).unwrap();
        assert_eq!(list[0].id, "c1");
        assert!(!list[0].session_started);
    }

    #[test]
    fn envelope_error_is_reported_with_the_daemons_message() {
        let body = json!({"success": false, "error": "no such conversation"}).to_string();
        let err = parse_envelope::<Vec<Conversation>>(&body).unwrap_err();
        assert_eq!(err, "no such conversation");
    }

    #[test]
    fn envelope_that_is_not_the_expected_shape_is_an_error() {
        let body = json!({"success": true, "data": {"id": 1}}).to_string();
        let err = parse_envelope::<Vec<Conversation>>(&body).unwrap_err();
        assert!(err.starts_with("unexpected response"), "{err}");
    }

    #[test]
    fn envelope_success_without_data_is_an_error() {
        let err = parse_envelope::<Vec<Conversation>>(r#"{"success":true}"#).unwrap_err();
        assert_eq!(err, "no data");
    }

    #[test]
    fn events_parse_by_type() {
        let text = json!({"type": "conversationUpdated", "conversation": conversation_json("c1")})
            .to_string();
        match parse_event(&text).unwrap() {
            ServerEvent::ConversationUpdated { conversation } => assert_eq!(conversation.id, "c1"),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            parse_event(r#"{"type":"resync"}"#).unwrap(),
            ServerEvent::Resync
        );
    }

    #[test]
    fn unknown_event_type_is_an_error() {
        let err = parse_event(r#"{"type":"nope"}"#).unwrap_err();
        assert!(err.starts_with("unexpected event"), "{err}");
    }

    fn serve_once(status: &str, body: String) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let status = status.to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request).to_string();
                if let Some(split) = text.find("\r\n\r\n") {
                    let length = text[..split]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() >= split + 4 + length {
                        break;
                    }
                }
            }
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8(request).unwrap()
        });
        (base, handle)
    }

    #[test]
    fn send_message_posts_the_text_to_the_messages_endpoint() {
        let entry = json!({
            "id": 7,
            "at": "2026-11-12T10:00:00.000Z",
            "kind": "you",
            "text": "hello\nworld",
            "attachments": []
        });
        let (base, request) = serve_once(
            "200 OK",
            json!({"success": true, "data": entry}).to_string(),
        );
        let sent = Client::new(base)
            .send_message("c 1", "hello\nworld")
            .unwrap();
        assert_eq!(sent.id, 7);
        let request = request.join().unwrap();
        assert!(
            request.starts_with("POST /api/conversations/c%201/messages "),
            "{request}"
        );
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(body).unwrap(),
            json!({"text": "hello\nworld"})
        );
    }

    #[test]
    fn send_message_reports_the_daemons_error() {
        let (base, request) = serve_once(
            "400 Bad Request",
            json!({"success": false, "error": "message is empty"}).to_string(),
        );
        let err = Client::new(base).send_message("c1", "x").unwrap_err();
        request.join().unwrap();
        assert!(err.ends_with("message is empty"), "{err}");
    }

    #[test]
    fn events_url_follows_the_daemon_url() {
        assert_eq!(
            events_url("http://127.0.0.1:7777").unwrap(),
            "ws://127.0.0.1:7777/api/events"
        );
        assert_eq!(
            events_url("http://localhost:1/").unwrap(),
            "ws://localhost:1/api/events"
        );
        assert!(events_url("https://x").is_err());
    }
}
