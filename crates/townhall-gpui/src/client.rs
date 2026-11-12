use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use townhall_core::model::{Conversation, ConversationDetail, ServerEvent};
use tungstenite::Message;

const RECONNECT_FIRST: Duration = Duration::from_millis(500);
const RECONNECT_MAX: Duration = Duration::from_secs(5);

pub trait Api: Send + Sync + 'static {
    fn conversations(&self) -> Result<Vec<Conversation>, String>;
    fn conversation(&self, id: &str) -> Result<ConversationDetail, String>;
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
        let mut response = match ureq::get(&url)
            .config()
            .http_status_as_error(false)
            .build()
            .call()
        {
            Ok(response) => response,
            Err(e) => return Err(format!("GET {url}: {e}")),
        };
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("GET {url}: cannot read the response: {e}"))?;
        parse_envelope(&body).map_err(|e| format!("GET {url}: {e}"))
    }
}

impl Api for Client {
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
