use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::clock;
use crate::context::Context;
use crate::model::{AdviserCall, ADVISER_AGENT_TYPE};
use crate::redact::redact;
use crate::runner::consult;

use super::checks::{self, Checked};
use super::{worktree_of, ToolDef, MAX_PROMPT_LEN};

pub const ASK_ADVISER: &str = "ask_adviser";
pub const MAX_QUESTION_LEN: u64 = 20_000;
pub const ADVICE_TIMEOUT: Duration = Duration::from_secs(600);
pub const NEEDS_THE_USER: &str = "NEEDS THE USER";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AskInput {
    question: String,
    #[serde(default)]
    context: Option<String>,
}

pub(super) enum Caller<'a> {
    Task(&'a str),
    Conversation(&'a str),
}

pub fn tools() -> Vec<ToolDef> {
    vec![ToolDef {
        name: ASK_ADVISER,
        description: "Ask the adviser, a senior sparring partner on a bigger model, instead of asking the user. Use it for design choices, which approach to take and whether you read the brief right. It reads your code (read-only, in your worktree) and answers with a recommendation and short reasoning. It is slow and costly, so ask one focused question with the context it needs. Answer `needsUser` is true when the adviser says only the user can decide: then, and only then, ask the user.",
        input_schema: schema(),
        read_only: true,
    }]
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "question": {
                "type": "string", "minLength": 1, "maxLength": MAX_QUESTION_LEN,
                "description": "One focused question, with the options you see."
            },
            "context": {
                "type": "string", "maxLength": MAX_PROMPT_LEN,
                "description": "What the adviser needs to know that is not in the code: the brief, what you tried, constraints. Optional."
            }
        },
        "required": ["question"],
        "additionalProperties": false
    })
}

fn text(e: crate::Error) -> String {
    e.to_string()
}

fn too_long(field: &str, value: &str, max: u64) -> Checked<()> {
    let len = u64::try_from(value.chars().count()).unwrap_or(u64::MAX);
    match len > max {
        true => Err(format!("`{field}` is longer than {max} characters")),
        false => Ok(()),
    }
}

fn prompt(question: &str, context: Option<&str>) -> String {
    let context = context.map(str::trim).filter(|c| !c.is_empty());
    match context {
        Some(context) => format!(
            "An agent working on this repo asks you:\n\n{question}\n\nWhat it says you need to know:\n{context}"
        ),
        None => format!("An agent working on this repo asks you:\n\n{question}"),
    }
}

struct Where {
    conversation_id: String,
    task: Option<String>,
    repo: String,
    cwd: PathBuf,
}

async fn locate(ctx: &Context, caller: &Caller<'_>) -> Checked<Where> {
    let store = ctx.store().map_err(text)?;
    match caller {
        Caller::Task(name) => {
            let task = store
                .find_task(name)
                .await
                .map_err(text)?
                .ok_or_else(|| format!("no task named `{name}`"))?;
            if task.agent.eq_ignore_ascii_case(ADVISER_AGENT_TYPE) {
                return Err("the adviser cannot ask itself: answer from what you read".into());
            }
            let cwd = worktree_of(ctx, &task)
                .await
                .ok_or_else(|| format!("the worktree of task `{}` is gone", task.name))?;
            Ok(Where {
                conversation_id: task.conversation_id.clone(),
                task: Some(task.name.clone()),
                repo: task.repo.clone(),
                cwd,
            })
        }
        Caller::Conversation(id) => {
            let conversation = store.conversation(id).await.map_err(text)?;
            let repo = checks::repo(ctx, &conversation, None).await?;
            Ok(Where {
                conversation_id: conversation.id,
                task: None,
                repo: repo.name,
                cwd: repo.path,
            })
        }
    }
}

pub(super) async fn ask(ctx: &Context, caller: Caller<'_>, input: AskInput) -> Checked<Value> {
    let question = input.question.trim();
    if question.is_empty() {
        return Err("`question` must not be blank: ask one focused question".into());
    }
    too_long("question", question, MAX_QUESTION_LEN)?;
    if let Some(context) = &input.context {
        too_long("context", context, MAX_PROMPT_LEN)?;
    }
    let store = ctx.store().map_err(text)?;
    let Some(adviser) = store.agent_type(ADVISER_AGENT_TYPE).await.map_err(text)? else {
        return Err(format!(
            "no adviser type: nobody can be asked. save an agent type named `{ADVISER_AGENT_TYPE}` (read_agent_type shows one) or ask the user"
        ));
    };
    let place = locate(ctx, &caller).await?;
    let started_at = clock::now_rfc3339();
    let asked = prompt(question, input.context.as_deref());
    let outcome = consult(
        ctx.claude_bin(),
        &adviser.def,
        &place.cwd,
        &asked,
        ADVICE_TIMEOUT,
    )
    .await;
    let (state, answer, tokens, model) = match &outcome {
        Ok(advice) => (
            "done",
            Some(advice.text.clone()),
            advice.tokens,
            Some(advice.model.clone()),
        ),
        Err(_) => ("failed", None, 0, adviser.def.model.clone()),
    };
    let call = AdviserCall {
        conversation_id: place.conversation_id.clone(),
        task: place.task.clone(),
        agent: adviser.def.name.clone(),
        repo: place.repo,
        question: question.to_string(),
        answer,
        state: state.into(),
        model,
        tokens,
        created_at: started_at,
        ended_at: clock::now_rfc3339(),
    };
    if let Err(e) = store.record_adviser_call(call).await {
        tracing::warn!(
            target: "townhall::mcp", event = "mcp.adviser_call_not_recorded",
            conversation = %place.conversation_id,
            error = %redact(&e.to_string()), "adviser call not recorded"
        );
    }
    match outcome {
        Ok(advice) => Ok(json!({
            "answer": advice.text,
            "needsUser": advice.text.trim_start().starts_with(NEEDS_THE_USER),
            "model": advice.model,
        })),
        Err(e) => Err(format!("the adviser could not answer: {e}")),
    }
}
