//! Agent type tools (2.12, in unit 2.8b's file): `list_agent_types`, `read_agent_type` and
//! `save_agent_type`, plus the type `create_task` runs a task as. A save that replaces a
//! type, or any save while `workWithoutAsking` is off, is not saved: townhall shows the TOML
//! in the thread and asks (`ask.rs`); the session saves it with `confirmed: true` once you
//! answered.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::context::Context;
use crate::definitions::DefinitionError;
use crate::model::{AgentType, AgentTypeSummary, DEFAULT_AGENT_TYPE};
use crate::store::Store;

use super::ask;
use super::checks::Checked;
use super::{ToolDef, MAX_NAME_LEN, NAME_PATTERN};

pub const LIST_AGENT_TYPES: &str = "list_agent_types";
pub const READ_AGENT_TYPE: &str = "read_agent_type";
pub const SAVE_AGENT_TYPE: &str = "save_agent_type";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListInput {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadInput {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SaveInput {
    source: String,
    #[serde(default)]
    confirmed: bool,
}

pub fn tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: LIST_AGENT_TYPES,
            description: "The saved agent types: name, description, adapter, model, effort and permission mode, plus the default create_task uses. Every task runs as one of them; reuse a type before you design a new one.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
            read_only: true,
        },
        ToolDef {
            name: READ_AGENT_TYPE,
            description: "One saved agent type: its TOML as written (`source`) and the parsed fields.",
            input_schema: read_schema(),
            read_only: true,
        },
        ToolDef {
            name: SAVE_AGENT_TYPE,
            description: "Save an agent type written as TOML. Returns its errors to fix, `saved`, or `asked`: townhall showed the user the TOML and asked them to confirm, so end your turn and wait. Once they say yes, call it again with the same source and `confirmed: true`.",
            input_schema: save_schema(),
            read_only: false,
        },
    ]
}

fn read_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "pattern": NAME_PATTERN, "minLength": 1, "maxLength": MAX_NAME_LEN }
        },
        "required": ["name"],
        "additionalProperties": false
    })
}

fn save_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "source": {
                "type": "string", "minLength": 1, "maxLength": Store::MAX_AGENT_SOURCE,
                "description": "The whole type as TOML, in the shape read_agent_type shows: name, description, adapter (claude or shell), model, effort (low, medium, high, xhigh or max; left out, claude's own default), tools (claude --allowedTools entries), prompt (its personality, appended to claude's system prompt); command for shell; requires (agent type names whose tasks must finish with a clearing report first)."
            },
            "confirmed": {
                "type": "boolean",
                "description": "True only after townhall asked the user to confirm this exact TOML and they said yes."
            }
        },
        "required": ["source"],
        "additionalProperties": false
    })
}

fn text(e: crate::Error) -> String {
    e.to_string()
}

/// `builder, reviewer`, or what to do when there is none.
fn names(types: &[AgentType]) -> String {
    match types.is_empty() {
        true => "none: save one with save_agent_type first".into(),
        false => types
            .iter()
            .map(|t| t.def.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// `builder` when it exists, else the first type for tasks by name: never the
/// conversation's own type.
fn default_of(types: &[AgentType]) -> Option<&AgentType> {
    types
        .iter()
        .find(|t| t.def.name.eq_ignore_ascii_case(DEFAULT_AGENT_TYPE))
        .or_else(|| types.iter().find(|t| t.for_tasks()))
}

/// The stored name of the type a task runs as: `wanted` (ignoring case), else the default.
pub(super) async fn resolve(ctx: &Context, wanted: Option<&str>) -> Checked<String> {
    let types = ctx
        .store()
        .map_err(text)?
        .agent_types()
        .await
        .map_err(text)?;
    let found = match wanted {
        Some(name) => types.iter().find(|t| t.def.name.eq_ignore_ascii_case(name)),
        None => default_of(&types),
    }
    .map(|t| t.def.name.clone());
    let for_tasks: Vec<AgentType> = types.into_iter().filter(AgentType::for_tasks).collect();
    found.ok_or_else(|| match wanted {
        Some(name) => format!(
            "unknown agent type `{name}`: pass `agent`, one of: {}",
            names(&for_tasks)
        ),
        None => format!("no agent type to run the task as: {}", names(&for_tasks)),
    })
}

pub(super) async fn list(ctx: &Context, _input: ListInput) -> Checked<Value> {
    let types = ctx
        .store()
        .map_err(text)?
        .agent_types()
        .await
        .map_err(text)?;
    let rows: Vec<AgentTypeSummary> = types.iter().map(AgentType::summary).collect();
    let default = default_of(&types).map(|t| t.def.name.clone());
    Ok(json!({ "types": rows, "default": default }))
}

pub(super) async fn read(ctx: &Context, input: ReadInput) -> Checked<Value> {
    let store = ctx.store().map_err(text)?;
    match store.agent_type(input.name.trim()).await.map_err(text)? {
        Some(found) => serde_json::to_value(found).map_err(|e| e.to_string()),
        None => {
            let types = store.agent_types().await.map_err(text)?;
            Err(format!(
                "no agent type named `{}`: the saved types are {}",
                input.name,
                names(&types)
            ))
        }
    }
}

fn errors_text(errors: &[DefinitionError]) -> String {
    let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
    format!(
        "{}\nfix the toml and call save_agent_type again.",
        lines.join("\n")
    )
}

/// Saves at once only a new type while `workWithoutAsking` is on, or a type the user
/// confirmed; anything else asks in the thread.
pub(super) async fn save(ctx: &Context, conversation_id: &str, input: SaveInput) -> Checked<Value> {
    let parsed = Store::check_agent_type(&input.source).map_err(|e| errors_text(&e))?;
    let store = ctx.store().map_err(text)?;
    let name = parsed.def.name.clone();
    let replaces = store.agent_type(&name).await.map_err(text)?.is_some();
    let on_its_own = !replaces && ctx.settings().await.map_err(text)?.work_without_asking;
    if !on_its_own && !input.confirmed {
        return ask::ask(store, conversation_id, &name, &parsed.source, replaces).await;
    }
    if !on_its_own {
        ask::check_confirmed(store, conversation_id, &name, &parsed.source).await?;
    }
    let (saved, replaced) = store.save_agent_type(&parsed.source).await.map_err(text)?;
    store
        .forget_agent_type_ask(conversation_id, &name)
        .await
        .map_err(text)?;
    Ok(json!({ "status": "saved", "name": saved.def.name, "replaced": replaced }))
}
