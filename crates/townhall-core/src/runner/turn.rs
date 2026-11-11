//! One turn of townhall's own session: resolve the repo, run `claude -p` (`session.rs`),
//! then hand the turn back. A resume of a session claude lost starts a fresh one, once.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::json;

use super::args::{self, TurnArgs};
use super::content::user_content;
use super::persona::Persona;
use super::prompt::{self, RepoNote};
use super::quote::{self, with_quote};
use super::repo::{self, Message, Resolved};
use super::session::{talk, TurnEnd};
use super::turns::TurnGuard;
use crate::model::{
    Conversation, ConversationStatus, EntryBody, NewMessage, Repo, CONVERSATION_AGENT_TYPE,
};
use crate::plugins::{claude_mcp, declared, hooks};
use crate::redact::redact;
use crate::{clock, ids, mcp, Context, Result};

pub const LOST_SESSION: &str = "claude no longer has this conversation's history, so townhall started a fresh session: earlier context is gone";

/// What one run of the turn sends.
#[derive(Debug, Clone, Copy)]
struct Attempt<'a> {
    note: &'a RepoNote,
    text: &'a str,
    attachments: &'a [String],
    /// The session was started over: the system prompt says so.
    lost: bool,
}

/// Runs the turn for `message` (already in the thread) and gives the conversation back.
pub(super) async fn run(
    ctx: Context,
    id: String,
    message: NewMessage,
    event: bool,
    guard: TurnGuard,
) {
    let started = Instant::now();
    let started_at = clock::now_rfc3339();
    // Lengths only: the message is the user's words.
    tracing::info!(
        target: "townhall::runner", event = "turn.start", conversation = %id,
        prompt_chars = message.text.chars().count(), attachments = message.attachments.len(),
        task_event = event, "turn started"
    );
    let end = match drive(&ctx, &id, &message, event, &guard).await {
        Ok(end) => end,
        Err(e) => TurnEnd::failed(format!("townhall failed: {e}")),
    };
    log_end(&id, started, &end);
    record(&ctx, &id, &started_at, &end).await;
    finish(&ctx, &id, guard, end).await;
}

async fn record(ctx: &Context, id: &str, started_at: &str, end: &TurnEnd) {
    if !end.ran {
        return;
    }
    let state = match (&end.error, end.stopped) {
        (Some(_), _) => "failed",
        (None, true) => "stopped",
        (None, false) => "done",
    };
    let saved = match ctx.store() {
        Ok(store) => {
            let ended_at = clock::now_rfc3339();
            store
                .record_turn(id, state, end.tokens, started_at, &ended_at)
                .await
        }
        Err(e) => Err(e),
    };
    if let Err(e) = saved {
        warn_turn(
            "turn.not_recorded",
            id,
            &e,
            "cannot record the turn as a run",
        );
    }
}

fn log_end(id: &str, started: Instant, end: &TurnEnd) {
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let outcome = match (&end.error, end.stopped) {
        (Some(_), _) => "failed",
        (None, true) => "stopped",
        (None, false) => "ok",
    };
    if let Some(error) = &end.error {
        tracing::warn!(
            target: "townhall::runner", event = "turn.failed", conversation = id,
            error = %redact(error), "turn failed"
        );
    }
    tracing::info!(
        target: "townhall::runner", event = "turn.end", conversation = id, outcome,
        duration_ms, cost_usd = end.cost_usd, "turn ended"
    );
}

async fn drive(
    ctx: &Context,
    id: &str,
    message: &NewMessage,
    event: bool,
    guard: &TurnGuard,
) -> Result<TurnEnd> {
    let conversation = ctx.store()?.conversation(id).await?;
    let asked = Message {
        text: &message.text,
        attachments: &message.attachments,
        reply_to: message.reply_to,
    };
    let resolved = match event {
        true => Resolved::Run {
            note: repo::for_event(ctx, &conversation).await?,
            text: message.text.clone(),
            attachments: vec![],
            reply_to: None,
        },
        false => repo::resolve(ctx, &conversation, asked).await?,
    };
    let Resolved::Run {
        note,
        text,
        attachments,
        reply_to,
    } = resolved
    else {
        return Ok(TurnEnd::default());
    };
    let text = match reply_to {
        Some(reply_to) => {
            let store = ctx.store()?;
            with_quote(&quote::of_entry(store, id, reply_to).await?, &text)
        }
        None => text,
    };
    let text = with_plugin_context(ctx, id, text, event).await;
    let attempt = Attempt {
        note: &note,
        text: &text,
        attachments: &attachments,
        lost: false,
    };
    let end = run_once(ctx, &conversation, attempt, guard).await?;
    if !end.lost || !conversation.session_started || guard.stop_requested() {
        return Ok(end);
    }
    let fresh = start_over(ctx, id).await?;
    let again = Attempt {
        lost: true,
        ..attempt
    };
    run_once(ctx, &fresh, again, guard).await
}

async fn with_plugin_context(ctx: &Context, id: &str, text: String, event: bool) -> String {
    let kind = if event { "event" } else { "you" };
    let fields = json!({"conversationId": id, "text": text, "kind": kind});
    let asked = hooks::ask(ctx, "message.before_turn", fields).await;
    asked.context.iter().fold(text, |text, (plugin, more)| {
        format!("{text}\n\nFrom plugin {plugin}: {more}")
    })
}

async fn run_once(
    ctx: &Context,
    conversation: &Conversation,
    attempt: Attempt<'_>,
    guard: &TurnGuard,
) -> Result<TurnEnd> {
    if guard.stop_requested() {
        return Ok(TurnEnd::default());
    }
    let cwd = session_dir(ctx, conversation, attempt.note).await?;
    if !cwd.is_dir() {
        return Ok(TurnEnd::failed(format!(
            "the folder this conversation runs in is gone ({}): start a new conversation",
            cwd.display()
        )));
    }
    let argv = plan(ctx, conversation, &attempt, &cwd).await?;
    let store = ctx.store()?;
    let content = user_content(store, &conversation.id, attempt.text, attempt.attachments).await?;
    let line = prompt::user_line(content);
    talk(ctx, conversation, &cwd, &argv, &line, guard.stop_signal()).await
}

/// A new session id, not started, and an `error` entry that says the history is gone.
async fn start_over(ctx: &Context, id: &str) -> Result<Conversation> {
    let store = ctx.store()?;
    let fresh = |c: Conversation| {
        Ok(Conversation {
            session_id: ids::new_session_id(),
            session_started: false,
            ..c
        })
    };
    let conversation = store.update_conversation(id, fresh).await?;
    let note = EntryBody::Error {
        message: LOST_SESSION.into(),
    };
    store.append_entry(id, note).await?;
    Ok(conversation)
}

fn wanted_dir(ctx: &Context, note: &RepoNote) -> PathBuf {
    match note {
        RepoNote::One(repo) => repo.path.clone(),
        RepoNote::Several(repos) if !repos.is_empty() => repos[0].path.clone(),
        _ => ctx.home().root().to_path_buf(),
    }
}

/// The folder of the turn. A started session keeps the folder it began in, since
/// `--resume` only finds a session there.
async fn session_dir(
    ctx: &Context,
    conversation: &Conversation,
    note: &RepoNote,
) -> Result<PathBuf> {
    let wanted = wanted_dir(ctx, note);
    if !conversation.session_started {
        return Ok(wanted);
    }
    let kept = ctx.store()?.session_dir(&conversation.id).await?;
    Ok(kept.unwrap_or(wanted))
}

/// The conversation's saved type `default`. A type that is missing leaves the built-in
/// behaviour; so does one that no longer parses, with a warning.
async fn persona(ctx: &Context) -> Result<Persona> {
    let found = match ctx.store()?.agent_type(CONVERSATION_AGENT_TYPE).await {
        Ok(found) => found,
        Err(e @ crate::Error::Corrupt { .. }) => {
            warn_turn(
                "turn.default_type_unreadable",
                "",
                &e,
                "agent type default unreadable, the built-in settings are used",
            );
            None
        }
        Err(e) => return Err(e),
    };
    Ok(Persona::from_type(found.as_ref().map(|t| &t.def)))
}

async fn plan(
    ctx: &Context,
    conversation: &Conversation,
    attempt: &Attempt<'_>,
    cwd: &Path,
) -> Result<Vec<String>> {
    if !conversation.session_started {
        ctx.store()?.set_session_dir(&conversation.id, cwd).await?;
    }
    let repos: Vec<Repo> = ctx.repos().await?;
    let add_dirs = repos
        .iter()
        .map(|r| r.path.clone())
        .filter(|p| p != cwd)
        .collect();
    let settings = ctx.settings().await?;
    let profile = match settings.learn_about_me {
        true => ctx.store()?.profile().await?,
        false => String::new(),
    };
    let persona = persona(ctx).await?;
    let notes = declared::planner_notes(ctx.store()?).await?;
    let system = prompt::system_prompt_with(
        &persona.prompt,
        attempt.note,
        &repos,
        &notes,
        Some(&profile),
    );
    let parts = claude_mcp::selected_for_planner(ctx.store()?, &persona.plugins).await?;
    let (mcp_config, tools) = args::with_plugins(
        mcp::config_json(ctx.base_url(), &conversation.id),
        persona.tools,
        &parts,
    );
    let turn = TurnArgs {
        session_id: conversation.session_id.clone(),
        resume: conversation.session_started,
        system_prompt: match attempt.lost {
            true => format!("{system}\n\n{}", prompt::LOST_HISTORY),
            false => system,
        },
        mcp_config,
        add_dirs,
        model: persona.model,
        permission_mode: persona.permission_mode,
        tools,
        effort: None,
        ephemeral: false,
    };
    args::build(&turn)
}

/// Appends the error, if any, then sets `yourTurn` and frees the slot in one write.
async fn finish(ctx: &Context, id: &str, guard: TurnGuard, end: TurnEnd) {
    let store = match ctx.store() {
        Ok(store) => store,
        Err(e) => return warn_turn("turn.not_finished", id, &e, "turn not finished"),
    };
    if let Some(message) = end.error {
        if let Err(e) = store.append_entry(id, EntryBody::Error { message }).await {
            warn_turn("turn.note_failed", id, &e, "cannot note the failed turn");
        }
    }
    let context = end.context;
    let done = store
        .update_conversation(id, move |c| {
            drop(guard);
            Ok(Conversation {
                status: ConversationStatus::YourTurn,
                context: context.or(c.context),
                ..c
            })
        })
        .await;
    if let Err(e) = done {
        warn_turn("turn.not_handed_back", id, &e, "turn not handed back");
    }
}

fn warn_turn(event: &str, id: &str, e: &crate::Error, message: &str) {
    tracing::warn!(
        target: "townhall::runner", event, conversation = id,
        error_kind = e.kind(), error = %redact(&e.to_string()), "{message}"
    );
}
