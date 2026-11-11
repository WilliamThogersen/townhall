//! Asking you before an agent type or a pipeline is saved: townhall shows the TOML in a
//! `townhall` entry with a short question and records the ask. A save with `confirmed: true`
//! goes through only for that exact TOML, and only once you wrote after the ask.

use serde_json::{json, Value};

use crate::model::EntryBody;
use crate::store::Store;

use super::checks::Checked;

/// What is being saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    AgentType,
    Pipeline,
}

impl Kind {
    fn noun(self) -> &'static str {
        match self {
            Kind::AgentType => "agent type",
            Kind::Pipeline => "pipeline",
        }
    }

    fn tool(self) -> &'static str {
        match self {
            Kind::AgentType => "save_agent_type",
            Kind::Pipeline => "save_pipeline",
        }
    }
}

fn next(kind: Kind) -> String {
    let tool = kind.tool();
    format!("townhall showed the user this toml and asked them to confirm. end your turn now and wait for their answer. when they say yes, call {tool} again with the same source and confirmed: true. when they want changes, change the toml and call {tool} without confirmed.")
}

fn text(e: crate::Error) -> String {
    e.to_string()
}

/// A Markdown fence longer than any run of backticks in `source`.
fn fence(source: &str) -> String {
    let longest = source.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

/// The entry's text: what is saved, the TOML, then the question.
pub(super) fn question(kind: Kind, name: &str, source: &str, replaces: bool) -> String {
    let noun = kind.noun();
    let what = match replaces {
        true => format!("replace the saved {noun} `{name}` with this one?"),
        false => format!("save the new {noun} `{name}`?"),
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
    kind: Kind,
    conversation_id: &str,
    name: &str,
    source: &str,
    replaces: bool,
) -> Checked<Value> {
    let body = EntryBody::Townhall {
        text: question(kind, name, source, replaces),
    };
    let entry = store
        .append_entry(conversation_id, body)
        .await
        .map_err(text)?;
    match kind {
        Kind::AgentType => {
            store
                .ask_agent_type(conversation_id, name, source, entry.id)
                .await
        }
        Kind::Pipeline => {
            store
                .ask_workflow(conversation_id, name, source, entry.id)
                .await
        }
    }
    .map_err(text)?;
    Ok(json!({ "status": "asked", "name": name, "replaces": replaces, "next": next(kind) }))
}

/// `Ok` when townhall asked about exactly this `source` and you wrote after it.
pub(super) async fn check_confirmed(
    store: &Store,
    kind: Kind,
    conversation_id: &str,
    name: &str,
    source: &str,
) -> Checked<()> {
    let (noun, tool) = (kind.noun(), kind.tool());
    let not_asked = format!(
        "{noun} `{name}` was not shown to the user to confirm: call {tool} without confirmed first, then again with confirmed: true once they said yes"
    );
    let open = match kind {
        Kind::AgentType => store.agent_type_ask(conversation_id, name).await,
        Kind::Pipeline => store.workflow_ask(conversation_id, name).await,
    }
    .map_err(text)?;
    let Some((shown, asked_in)) = open else {
        return Err(not_asked);
    };
    if shown != source {
        return Err(format!("this toml differs from the one the user was asked to confirm: pass the same source, or call {tool} without confirmed to ask about the new one"));
    }
    let entries = store.entries(conversation_id).await.map_err(text)?;
    let answered = entries
        .iter()
        .any(|e| e.id > asked_in && matches!(e.body, EntryBody::You { .. }));
    match answered {
        true => Ok(()),
        false => Err(format!(
            "the user has not answered yet: end your turn, and once they said yes to `{name}`, call {tool} with confirmed: true"
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
        let asked = question(Kind::AgentType, "a", "name = \"a\"\n", false);
        assert_eq!(
            asked,
            "save the new agent type `a`?\n\n```toml\nname = \"a\"\n```\n\nanswer yes to save it, or say what to change."
        );
        assert!(question(Kind::AgentType, "a", "x", true)
            .starts_with("replace the saved agent type `a`"));
        assert!(question(Kind::Pipeline, "p", "x", false).starts_with("save the new pipeline `p`"));
    }
}
