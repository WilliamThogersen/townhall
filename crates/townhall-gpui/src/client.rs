use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use townhall_core::model::{
    Conversation, ConversationDetail, Repo, ServerEvent, Task, ThreadEntry,
};
use townhall_core::search::SearchHit;
use tungstenite::Message;

const RECONNECT_FIRST: Duration = Duration::from_millis(500);
const RECONNECT_MAX: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub user_name: Option<String>,
}

pub trait Api: Send + Sync + 'static {
    fn conversations(&self) -> Result<Vec<Conversation>, String>;
    fn conversation(&self, id: &str) -> Result<ConversationDetail, String>;
    fn send_message(&self, id: &str, text: &str) -> Result<ThreadEntry, String>;
    fn tasks(&self) -> Result<Vec<Task>, String>;
    fn repos(&self) -> Result<Vec<Repo>, String>;
    fn info(&self) -> Result<Info, String>;
    fn search(&self, query: &str) -> Result<Vec<SearchHit>, String>;
    fn end_conversation(&self, id: &str) -> Result<Conversation, String>;
    fn link_repo(&self, id: &str, repo: Option<&str>) -> Result<Conversation, String>;
    fn start_conversation(&self, text: &str) -> Result<ConversationDetail, String>;
    fn add_repo(&self, path: &str) -> Result<Repo, String>;
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

    fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}/api{path}", self.base);
        let result = ureq::post(&url)
            .config()
            .http_status_as_error(false)
            .build()
            .send_empty();
        read_envelope("POST", &url, result)
    }

    fn patch<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T, String> {
        let url = format!("{}/api{path}", self.base);
        let result = ureq::patch(&url)
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(body);
        read_envelope("PATCH", &url, result)
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

    fn tasks(&self) -> Result<Vec<Task>, String> {
        self.get("/tasks")
    }

    fn repos(&self) -> Result<Vec<Repo>, String> {
        self.get("/repos")
    }

    fn info(&self) -> Result<Info, String> {
        self.get("/info")
    }

    fn search(&self, query: &str) -> Result<Vec<SearchHit>, String> {
        self.get(&format!("/search?q={}", encode_segment(query)))
    }

    fn end_conversation(&self, id: &str) -> Result<Conversation, String> {
        self.post_empty(&format!("/conversations/{}/end", encode_segment(id)))
    }

    fn link_repo(&self, id: &str, repo: Option<&str>) -> Result<Conversation, String> {
        self.patch(
            &format!("/conversations/{}", encode_segment(id)),
            &serde_json::json!({ "linkedRepo": repo }),
        )
    }

    fn start_conversation(&self, text: &str) -> Result<ConversationDetail, String> {
        self.post("/conversations", &serde_json::json!({ "text": text }))
    }

    fn add_repo(&self, path: &str) -> Result<Repo, String> {
        self.post("/repos", &serde_json::json!({ "path": path }))
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

    fn request_of(request: std::thread::JoinHandle<String>) -> (String, serde_json::Value) {
        let request = request.join().unwrap();
        let line = request.lines().next().unwrap().to_string();
        let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
        let body = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        (line, body)
    }

    fn task_json(name: &str) -> serde_json::Value {
        json!({
            "name": name, "title": "t", "prompt": "p", "repo": "shop", "baseBranch": "main",
            "after": [], "state": "working", "conversationId": "c1", "attachments": [],
            "createdAt": "2026-11-12T09:00:00.000Z", "pr": null, "agent": "Coder",
            "blocked": null, "prBase": null
        })
    }

    fn ok(data: serde_json::Value) -> String {
        json!({"success": true, "data": data}).to_string()
    }

    #[test]
    fn tasks_come_from_the_tasks_endpoint() {
        let (base, request) = serve_once("200 OK", ok(json!([task_json("fix")])));
        let tasks = Client::new(base).tasks().unwrap();
        assert_eq!(tasks[0].name, "fix");
        assert_eq!(request_of(request).0, "GET /api/tasks HTTP/1.1");
    }

    #[test]
    fn repos_come_from_the_repos_endpoint() {
        let repo = json!({"name": "shop", "path": "/x/shop", "baseBranch": "develop"});
        let (base, request) = serve_once("200 OK", ok(json!([repo])));
        let repos = Client::new(base).repos().unwrap();
        assert_eq!(repos[0].base_branch, "develop");
        assert_eq!(request_of(request).0, "GET /api/repos HTTP/1.1");
    }

    #[test]
    fn info_gives_the_user_name_and_ignores_the_rest() {
        let (base, request) = serve_once(
            "200 OK",
            ok(
                json!({"home": "/h", "repo": "/r", "repoKey": "k", "userName": "Ada Lovelace", "version": "1"}),
            ),
        );
        let info = Client::new(base).info().unwrap();
        assert_eq!(info.user_name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(request_of(request).0, "GET /api/info HTTP/1.1");
    }

    #[test]
    fn search_encodes_the_query_and_reads_title_and_message_hits() {
        let hits = json!([
            {"conversationId": "a", "title": "t", "match": "title", "entryId": null, "snippet": null, "updatedAt": "2026-11-12T10:00:00.000Z"},
            {"conversationId": "b", "title": "u", "match": "message", "entryId": 4, "snippet": "…x…", "updatedAt": "2026-11-12T10:00:00.000Z"}
        ]);
        let (base, request) = serve_once("200 OK", ok(hits));
        let found = Client::new(base).search("login loop&x").unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].entry_id, Some(4));
        assert_eq!(
            request_of(request).0,
            "GET /api/search?q=login%20loop%26x HTTP/1.1"
        );
    }

    #[test]
    fn ending_posts_to_the_end_endpoint() {
        let (base, request) = serve_once("200 OK", ok(conversation_json("c 1")));
        let ended = Client::new(base).end_conversation("c 1").unwrap();
        assert_eq!(ended.id, "c 1");
        assert_eq!(
            request_of(request).0,
            "POST /api/conversations/c%201/end HTTP/1.1"
        );
    }

    #[test]
    fn linking_a_repo_patches_the_conversation_and_null_sets_auto() {
        let (base, request) = serve_once("200 OK", ok(conversation_json("c1")));
        Client::new(base).link_repo("c1", Some("shop")).unwrap();
        let (line, body) = request_of(request);
        assert_eq!(line, "PATCH /api/conversations/c1 HTTP/1.1");
        assert_eq!(body, json!({"linkedRepo": "shop"}));
        let (base, request) = serve_once("200 OK", ok(conversation_json("c1")));
        Client::new(base).link_repo("c1", None).unwrap();
        assert_eq!(request_of(request).1, json!({"linkedRepo": null}));
    }

    #[test]
    fn starting_posts_the_text_and_returns_the_new_conversation() {
        let detail = json!({
            "conversation": conversation_json("new"),
            "entries": [], "tasks": [], "attachments": []
        });
        let (base, request) = serve_once("200 OK", ok(detail));
        let started = Client::new(base).start_conversation("fix it").unwrap();
        assert_eq!(started.conversation.id, "new");
        let (line, body) = request_of(request);
        assert_eq!(line, "POST /api/conversations HTTP/1.1");
        assert_eq!(body, json!({"text": "fix it"}));
    }

    #[test]
    fn adding_a_repo_posts_its_path() {
        let repo = json!({"name": "shop", "path": "/x/shop", "baseBranch": "main"});
        let (base, request) = serve_once("200 OK", ok(repo));
        let added = Client::new(base).add_repo("/x/shop").unwrap();
        assert_eq!(added.name, "shop");
        let (line, body) = request_of(request);
        assert_eq!(line, "POST /api/repos HTTP/1.1");
        assert_eq!(body, json!({"path": "/x/shop"}));
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
