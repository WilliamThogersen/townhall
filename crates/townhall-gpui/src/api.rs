use serde::de::DeserializeOwned;
use serde::Deserialize;
use townhall_core::model::{Conversation, ConversationDetail};

#[derive(Deserialize)]
struct Envelope<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

fn get<T: DeserializeOwned>(base: &str, path: &str) -> Result<T, String> {
    let url = format!("{base}/api{path}");
    let mut response = ureq::get(&url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    let envelope: Envelope<T> = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("GET {url}: unexpected response: {e}"))?;
    match (envelope.success, envelope.data) {
        (true, Some(data)) => Ok(data),
        _ => Err(format!(
            "GET {url}: {}",
            envelope.error.unwrap_or_else(|| "no data".into())
        )),
    }
}

pub fn conversations(base: &str) -> Result<Vec<Conversation>, String> {
    get(base, "/conversations")
}

pub fn conversation(base: &str, id: &str) -> Result<ConversationDetail, String> {
    get(base, &format!("/conversations/{id}"))
}
