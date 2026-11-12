use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use townhall_core::model::{Attachment, ThreadEntry};

use super::validate::FileMeta;
use crate::client::parse_envelope;

const CHUNK: usize = 64 * 1024;
const BOUNDARY: &str = "----townhall-native-form-boundary";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadError {
    pub message: String,
    pub retryable: bool,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

pub trait AttachApi: Send + Sync + 'static {
    fn upload(
        &self,
        conversation: &str,
        meta: &FileMeta,
        bytes: &[u8],
        progress: &dyn Fn(u8),
        cancel: &Cancel,
    ) -> Result<Attachment, UploadError>;
    fn fetch(&self, conversation: &str, id: &str) -> Result<Vec<u8>, String>;
    fn send(
        &self,
        conversation: &str,
        text: &str,
        attachments: &[String],
    ) -> Result<ThreadEntry, String>;
}

pub struct Http {
    base: String,
}

impl Http {
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }

    fn attachments_url(&self, conversation: &str) -> String {
        format!(
            "{}/api/conversations/{}/attachments",
            self.base,
            encode_segment(conversation)
        )
    }
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

fn form_body(meta: &FileMeta, bytes: &[u8]) -> Vec<u8> {
    let name = meta.name.replace('"', "%22").replace(['\r', '\n'], " ");
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: {}\r\n\r\n",
        meta.mime
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

struct Progress<'a> {
    body: Vec<u8>,
    sent: usize,
    last: u8,
    report: &'a dyn Fn(u8),
    cancel: &'a Cancel,
}

impl Read for Progress<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "the upload was cancelled",
            ));
        }
        let end = (self.sent + CHUNK.min(buf.len())).min(self.body.len());
        let chunk = &self.body[self.sent..end];
        buf[..chunk.len()].copy_from_slice(chunk);
        self.sent = end;
        let percent = (self.sent * 100 / self.body.len().max(1)).min(99) as u8;
        if percent != self.last {
            self.last = percent;
            (self.report)(percent);
        }
        Ok(chunk.len())
    }
}

fn failure(message: String, retryable: bool) -> UploadError {
    UploadError {
        message,
        retryable,
        cancelled: false,
    }
}

impl AttachApi for Http {
    fn upload(
        &self,
        conversation: &str,
        meta: &FileMeta,
        bytes: &[u8],
        progress: &dyn Fn(u8),
        cancel: &Cancel,
    ) -> Result<Attachment, UploadError> {
        if cancel.is_cancelled() {
            return Err(UploadError {
                message: "the upload was cancelled".into(),
                retryable: false,
                cancelled: true,
            });
        }
        let body = form_body(meta, bytes);
        let length = body.len();
        let mut reader = Progress {
            body,
            sent: 0,
            last: 0,
            report: progress,
            cancel,
        };
        let result = ureq::post(&self.attachments_url(conversation))
            .config()
            .http_status_as_error(false)
            .build()
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .header("Content-Length", length.to_string())
            .send(ureq::SendBody::from_reader(&mut reader));
        if cancel.is_cancelled() {
            return Err(UploadError {
                message: "the upload was cancelled".into(),
                retryable: false,
                cancelled: true,
            });
        }
        let mut response = result.map_err(|_| {
            failure(
                format!(
                    "cannot reach the daemon: the upload of {} stopped",
                    meta.name
                ),
                true,
            )
        })?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().map_err(|e| {
            failure(
                format!("HTTP {status}: cannot read the response: {e}"),
                true,
            )
        })?;
        match parse_envelope::<Attachment>(&text) {
            Ok(attachment) => {
                progress(100);
                Ok(attachment)
            }
            Err(message) => Err(failure(message, status >= 500)),
        }
    }

    fn fetch(&self, conversation: &str, id: &str) -> Result<Vec<u8>, String> {
        let url = format!(
            "{}/{}",
            self.attachments_url(conversation),
            encode_segment(id)
        );
        let mut response = ureq::get(&url)
            .call()
            .map_err(|e| format!("GET {url}: {e}"))?;
        response
            .body_mut()
            .with_config()
            .limit(super::validate::MAX_ATTACHMENT_BYTES)
            .read_to_vec()
            .map_err(|e| format!("GET {url}: cannot read the image: {e}"))
    }

    fn send(
        &self,
        conversation: &str,
        text: &str,
        attachments: &[String],
    ) -> Result<ThreadEntry, String> {
        let url = format!(
            "{}/api/conversations/{}/messages",
            self.base,
            encode_segment(conversation)
        );
        let result = ureq::post(&url)
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(serde_json::json!({ "text": text, "attachments": attachments }));
        let mut response = result.map_err(|e| format!("POST {url}: {e}"))?;
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("POST {url}: cannot read the response: {e}"))?;
        parse_envelope(&body).map_err(|e| format!("POST {url}: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use std::sync::Mutex;

    fn serve_once(status: &str, body: String) -> (String, std::thread::JoinHandle<Vec<u8>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let status = status.to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
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
            request
        });
        (base, handle)
    }

    fn attachment_json() -> serde_json::Value {
        json!({
            "id": "a1", "conversationId": "c 1", "name": "shot.png", "mime": "image/png",
            "size": 3, "kind": "image", "createdAt": "2026-11-12T10:00:00.000Z"
        })
    }

    fn png() -> FileMeta {
        FileMeta {
            name: "shot.png".into(),
            mime: "image/png".into(),
            size: 3,
        }
    }

    #[test]
    fn an_upload_posts_one_multipart_file_and_reports_progress_up_to_100() {
        let (base, request) = serve_once(
            "200 OK",
            json!({"success": true, "data": attachment_json()}).to_string(),
        );
        let seen = Mutex::new(Vec::new());
        let attachment = Http::new(base)
            .upload(
                "c 1",
                &png(),
                &[1, 2, 3],
                &|p| seen.lock().unwrap().push(p),
                &Cancel::default(),
            )
            .unwrap();
        assert_eq!(attachment.id, "a1");
        let request = request.join().unwrap();
        let text = String::from_utf8_lossy(&request).to_string();
        assert!(
            text.starts_with("POST /api/conversations/c%201/attachments "),
            "{text}"
        );
        assert!(text.contains("multipart/form-data; boundary="), "{text}");
        assert!(
            text.contains("name=\"file\"; filename=\"shot.png\""),
            "{text}"
        );
        assert!(text.contains("Content-Type: image/png"), "{text}");
        assert_eq!(seen.lock().unwrap().last(), Some(&100));
    }

    #[test]
    fn a_refusal_carries_the_daemons_words_and_cannot_be_retried() {
        let (base, request) = serve_once(
            "400 Bad Request",
            json!({"success": false, "error": "shot.png is over 20 MB"}).to_string(),
        );
        let err = Http::new(base)
            .upload("c1", &png(), &[1], &|_| {}, &Cancel::default())
            .unwrap_err();
        request.join().unwrap();
        assert_eq!(err.message, "shot.png is over 20 MB");
        assert!(!err.retryable && !err.cancelled);
    }

    #[test]
    fn a_server_error_can_be_retried() {
        let (base, request) = serve_once(
            "500 Internal Server Error",
            json!({"success": false, "error": "disk full"}).to_string(),
        );
        let err = Http::new(base)
            .upload("c1", &png(), &[1], &|_| {}, &Cancel::default())
            .unwrap_err();
        request.join().unwrap();
        assert!(err.retryable);
    }

    #[test]
    fn an_unreachable_daemon_can_be_retried_and_says_which_upload_stopped() {
        let err = Http::new("http://127.0.0.1:1")
            .upload("c1", &png(), &[1], &|_| {}, &Cancel::default())
            .unwrap_err();
        assert_eq!(
            err.message,
            "cannot reach the daemon: the upload of shot.png stopped"
        );
        assert!(err.retryable);
    }

    #[test]
    fn a_cancelled_upload_sends_nothing_and_is_marked_cancelled() {
        let cancel = Cancel::default();
        cancel.cancel();
        let err = Http::new("http://127.0.0.1:1")
            .upload("c1", &png(), &[1], &|_| {}, &cancel)
            .unwrap_err();
        assert!(err.cancelled);
    }

    #[test]
    fn fetching_reads_the_bytes_of_one_attachment() {
        use std::io::Write as _;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 3\r\nconnection: close\r\n\r\nabc")
                .unwrap();
            String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .unwrap()
                .to_string()
        });
        let bytes = Http::new(base).fetch("c 1", "a/1").unwrap();
        assert_eq!(bytes, b"abc");
        assert_eq!(
            handle.join().unwrap(),
            "GET /api/conversations/c%201/attachments/a%2F1 HTTP/1.1"
        );
    }

    #[test]
    fn a_message_goes_out_with_the_ids_of_its_attachments() {
        let entry = json!({
            "id": 7, "at": "2026-11-12T10:00:00.000Z", "kind": "you",
            "text": "see", "attachments": ["a1"]
        });
        let (base, request) = serve_once(
            "200 OK",
            json!({"success": true, "data": entry}).to_string(),
        );
        let sent = Http::new(base).send("c1", "see", &["a1".into()]).unwrap();
        assert_eq!(sent.id, 7);
        let request = String::from_utf8(request.join().unwrap()).unwrap();
        assert!(request.starts_with("POST /api/conversations/c1/messages "));
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(body).unwrap(),
            json!({"text": "see", "attachments": ["a1"]})
        );
    }
}
