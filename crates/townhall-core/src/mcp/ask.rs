//! Asking you before an agent type is saved: townhall shows the TOML in a `townhall`
//! entry with a short question and records the ask. A save with `confirmed: true` goes
//! through only for that exact TOML, and only once you wrote after the ask.

use serde_json::{json, Value};

use crate::model::EntryBody;
use crate::store::Store;

use super::checks::Checked;

const NEXT: &str = "townhall showed the user this toml and asked them to confirm. end your turn now and wait for their answer. when they say yes, call save_agent_type again with the same source and confirmed: true. when they want changes, change the toml and call save_agent_type without confirmed.";

fn text(e: crate::Error) -> String {
    e.to_string()
}

/// A Markdown fence longer than any run of backticks in `source`.
fn fence(source: &str) -> String {
    let longest = source.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

/// The entry's text: what is saved, the TOML, then the question.
pub(super) fn question(name: &str, source: &str, replaces: bool) -> String {
    let what = match replaces {
        true => format!("replace the saved agent type `{name}` with this one?"),
        false => format!("save the new agent type `{name}`?"),
    };
    let fence = fence(source);
    let body = source.trim_end_matches('\n');
    format!(
        "{what}\n\n{fence}toml\n{body}\n{fence}\n\nanswer yes to save it, or say what to change."
    )
}

/// Shows the TOML and the question in the thread and records the ask.
pub(super) async fn ask(
    store: &Store,
    conversation_id: &str,
    name: &str,
    source: &str,
    replaces: bool,
) -> Checked<Value> {
    let body = EntryBody::Townhall {
        text: question(name, source, replaces),
    };
    let entry = store
        .append_entry(conversation_id, body)
        .await
        .map_err(text)?;
    store
        .ask_agent_type(conversation_id, name, source, entry.id)
        .await
        .map_err(text)?;
    Ok(json!({ "status": "asked", "name": name, "replaces": replaces, "next": NEXT }))
}

/// `Ok` when townhall asked about exactly this `source` and you wrote after it.
pub(super) async fn check_confirmed(
    store: &Store,
    conversation_id: &str,
    name: &str,
    source: &str,
) -> Checked<()> {
    let not_asked = format!(
        "agent type `{name}` was not shown to the user to confirm: call save_agent_type without confirmed first, then again with confirmed: true once they said yes"
    );
    let Some((shown, asked_in)) = store
        .agent_type_ask(conversation_id, name)
        .await
        .map_err(text)?
    else {
        return Err(not_asked);
    };
    if shown != source {
        return Err("this toml differs from the one the user was asked to confirm: pass the same source, or call save_agent_type without confirmed to ask about the new one".into());
    }
    let entries = store.entries(conversation_id).await.map_err(text)?;
    let answered = entries
        .iter()
        .any(|e| e.id > asked_in && matches!(e.body, EntryBody::You { .. }));
    match answered {
        true => Ok(()),
        false => Err(format!(
            "the user has not answered yet: end your turn, and once they said yes to `{name}`, call save_agent_type with confirmed: true"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fence_outlasts_backticks_in_the_toml() {
        assert_eq!(fence("name = \"a\""), "```");
        assert_eq!(fence("prompt = \"use ```x``` and ````\""), "`````");
        let asked = question("a", "name = \"a\"\n", false);
        assert_eq!(
            asked,
            "save the new agent type `a`?\n\n```toml\nname = \"a\"\n```\n\nanswer yes to save it, or say what to change."
        );
        assert!(question("a", "x", true).starts_with("replace the saved agent type `a`"));
    }
}
