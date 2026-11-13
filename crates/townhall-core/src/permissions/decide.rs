use std::path::Path;
use std::time::Duration;

use super::deny;
use super::verdict::{self, Kind, Prompt};
use crate::context::Context;
use crate::definitions::PermissionMode;
use crate::model::{
    AgentInfo, AgentType, NewPermissionDecision, PermissionVerdict, Task, ADVISER_AGENT_TYPE,
};
use crate::redact::redact;
use crate::runner::consult;
use crate::{agent, steps, tmux, Workspace};

const STEPS_SHOWN: usize = 6;
const BRIEF_CHARS: usize = 4_000;
const PROMPT_CHARS: usize = 2_000;
const REASON_CHARS: usize = 300;
const KEY_GAP: Duration = Duration::from_millis(300);
const GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Answered,
    Ask { advice: String },
    Gone,
}

pub async fn decide(
    ctx: &Context,
    ws: &Workspace,
    info: &AgentInfo,
    task: &Task,
    question: &str,
    limit: Duration,
) -> Outcome {
    let screen = tmux::screen(ws.key(), &info.name).await.ok();
    let Some(prompt) = verdict::prompt_of(question, screen.as_deref()) else {
        return Outcome::Gone;
    };
    let asked = question
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("Do you want"))
        .unwrap_or_default()
        .to_string();
    let worktree = Path::new(&info.worktree);
    if prompt.command.trim().is_empty() {
        let reason = "townhall could not read what the agent wants to run";
        return escalate(ctx, task, &prompt, PermissionVerdict::AskUser, reason).await;
    }
    if let Some(why) = deny::check(&prompt.command, worktree, &task.base_branch) {
        let reason = format!("always yours to decide: {why}");
        return escalate(ctx, task, &prompt, PermissionVerdict::HardDeny, &reason).await;
    }
    let kind = match ctx.store() {
        Ok(store) => store.agent_type(&task.agent).await.ok().flatten(),
        Err(_) => None,
    };
    let answer = ask_adviser(ctx, info, task, kind.as_ref(), &prompt, limit).await;
    let (verdict, reason) = match answer {
        Err(Failed::Timeout) => {
            let reason = format!("the adviser did not answer within {} s", limit.as_secs());
            return escalate(ctx, task, &prompt, PermissionVerdict::Timeout, &reason).await;
        }
        Err(Failed::Other(why)) => {
            let reason = format!("the adviser could not answer: {why}");
            return escalate(ctx, task, &prompt, PermissionVerdict::AskUser, &reason).await;
        }
        Ok(v) => (v.kind, v.reason),
    };
    let plan_mode = kind
        .as_ref()
        .is_some_and(|t| t.def.permission_mode == Some(PermissionMode::Plan));
    if verdict == Kind::Allow && plan_mode {
        let reason = "the agent type runs in plan mode, which never grants a command".to_string();
        return escalate(ctx, task, &prompt, PermissionVerdict::AskUser, &reason).await;
    }
    if verdict == Kind::AskUser {
        return escalate(ctx, task, &prompt, PermissionVerdict::AskUser, &reason).await;
    }
    let still_there = match tmux::screen(ws.key(), &info.name).await {
        Ok(text) => text.contains(&asked),
        Err(_) => false,
    };
    if !still_there {
        return Outcome::Gone;
    }
    let sent = match verdict {
        Kind::Allow => agent::send_text(ws, &info.name, &prompt.allow_key).await,
        _ => refuse(ws, &info.name, &prompt, &reason).await,
    };
    if let Err(e) = sent {
        let reason = format!("townhall could not answer the prompt: {e}");
        return escalate(ctx, task, &prompt, PermissionVerdict::AskUser, &reason).await;
    }
    let logged = match verdict {
        Kind::Allow => PermissionVerdict::Allow,
        _ => PermissionVerdict::Deny,
    };
    log(ctx, task, &prompt, logged, &reason).await;
    Outcome::Answered
}

async fn refuse(ws: &Workspace, name: &str, prompt: &Prompt, reason: &str) -> crate::Result<()> {
    agent::send_text(ws, name, &prompt.deny_key).await?;
    tokio::time::sleep(KEY_GAP).await;
    let said = one_line(reason);
    let said = match said.is_empty() {
        true => "not allowed here: pick another way".to_string(),
        false => format!("not allowed: {said}. pick another way"),
    };
    agent::send(ws, name, &said).await
}

fn one_line(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(REASON_CHARS).collect()
}

async fn escalate(
    ctx: &Context,
    task: &Task,
    prompt: &Prompt,
    verdict: PermissionVerdict,
    reason: &str,
) -> Outcome {
    log(ctx, task, prompt, verdict, reason).await;
    Outcome::Ask {
        advice: one_line(reason),
    }
}

async fn log(
    ctx: &Context,
    task: &Task,
    prompt: &Prompt,
    verdict: PermissionVerdict,
    reason: &str,
) {
    let entry = NewPermissionDecision {
        task: task.name.clone(),
        agent_type: task.agent.clone(),
        command: prompt.command.clone(),
        pattern: verdict::pattern_of(&prompt.command),
        verdict,
        reason: one_line(reason),
    };
    let saved = match ctx.store() {
        Ok(store) => store.record_permission_decision(entry).await,
        Err(e) => Err(e),
    };
    if let Err(e) = saved {
        tracing::warn!(
            target: "townhall::agent", event = "agent.permission_not_logged",
            task = %task.name, error = %redact(&e.to_string()), "permission decision not logged"
        );
    }
}

enum Failed {
    Timeout,
    Other(String),
}

async fn ask_adviser(
    ctx: &Context,
    info: &AgentInfo,
    task: &Task,
    kind: Option<&AgentType>,
    prompt: &Prompt,
    limit: Duration,
) -> Result<verdict::Verdict, Failed> {
    let store = ctx.store().map_err(|e| Failed::Other(e.to_string()))?;
    let adviser = store
        .agent_type(ADVISER_AGENT_TYPE)
        .await
        .map_err(|e| Failed::Other(e.to_string()))?
        .ok_or_else(|| Failed::Other(format!("no agent type named `{ADVISER_AGENT_TYPE}`")))?;
    let worktree = Path::new(&info.worktree);
    let recent = recent_steps(worktree).await;
    let text = question_for(task, kind, worktree, &prompt.command, &recent);
    let run = consult(
        ctx.claude_bin(),
        &adviser.def,
        worktree,
        &text,
        limit + GRACE,
    );
    match tokio::time::timeout(limit, run).await {
        Err(_) => Err(Failed::Timeout),
        Ok(Err(why)) => Err(Failed::Other(why)),
        Ok(Ok(advice)) => Ok(verdict::parse(&advice.text)),
    }
}

async fn recent_steps(worktree: &Path) -> Vec<String> {
    let Ok(projects) = steps::claude_projects_dir() else {
        return Vec::new();
    };
    let all = steps::steps_for_agent(&projects, worktree)
        .await
        .unwrap_or_default();
    let from = all.len().saturating_sub(STEPS_SHOWN);
    all[from..]
        .iter()
        .map(|s| format!("{}: {}", s.tool, s.summary))
        .collect()
}

fn cut(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

fn question_for(
    task: &Task,
    kind: Option<&AgentType>,
    worktree: &Path,
    command: &str,
    steps: &[String],
) -> String {
    let (instructions, tools, mode) = match kind {
        Some(t) => (
            cut(t.def.prompt.as_deref().unwrap_or("none"), PROMPT_CHARS),
            match t.def.tools.is_empty() {
                true => "none listed".to_string(),
                false => t.def.tools.join(", "),
            },
            t.def
                .permission_mode
                .map_or("default", PermissionMode::as_str),
        ),
        None => ("unknown".into(), "unknown".into(), "default"),
    };
    let steps = match steps.is_empty() {
        true => "none".to_string(),
        false => steps
            .iter()
            .map(|s| format!("- {s}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    format!(
        "An agent is asking Claude Code for permission. Decide whether this agent should be allowed.\n\n\
         Agent type: {}\nIts instructions: {instructions}\nIts tools: {tools}\nIts permission mode: {mode}\n\
         Its worktree, the only folder it may touch: {}\nBase branch: {}\n\n\
         Task brief:\n{}\n\nIts last steps:\n{steps}\n\nIt asks to:\n{command}\n\n\
         Start your answer with exactly one of these on the first line:\n\
         ALLOW: <why it fits the task>\n\
         DENY: <one line the agent can act on, such as what to do instead>\n\
         ASK USER: <why only the user can decide>\n\
         Allow only what the task needs, inside the worktree. When unsure, ask the user.",
        task.agent,
        worktree.display(),
        task.base_branch,
        cut(&task.prompt, BRIEF_CHARS),
    )
}
