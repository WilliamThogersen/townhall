//! One turn of townhall's own session: resolve the repo, run `claude -p` (`session.rs`),
//! then hand the turn back. A resume of a session claude lost starts a fresh one, once.

use std::path::{Path, PathBuf};

use super::args::{self, TurnArgs};
use super::content::user_content;
use super::prompt::{self, RepoNote};
use super::repo::{self, Message, Resolved};
use super::session::{talk, TurnEnd};
use super::turns::TurnGuard;
use crate::model::{Conversation, ConversationStatus, EntryBody, NewMessage, Repo};
use crate::{ids, mcp, Context, Result};

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
pub(super) async fn run(ctx: Context, id: String, message: NewMessage, guard: TurnGuard) {
    let end = match drive(&ctx, &id, &message, &guard).await {
        Ok(end) => end,
        Err(e) => TurnEnd::failed(format!("townhall failed: {e}")),
    };
    finish(&ctx, &id, guard, end).await;
}

async fn drive(
    ctx: &Context,
    id: &str,
    message: &NewMessage,
    guard: &TurnGuard,
) -> Result<TurnEnd> {
    let conversation = ctx.store()?.conversation(id).await?;
    let asked = Message {
        text: &message.text,
        attachments: &message.attachments,
    };
    let Resolved::Run {
        note,
        text,
        attachments,
    } = repo::resolve(ctx, &conversation, asked).await?
    else {
        return Ok(TurnEnd::default());
    };
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
    let system = prompt::system_prompt(attempt.note, &repos);
    let model = ctx.settings().await?.townhall_model;
    let turn = TurnArgs {
        session_id: conversation.session_id.clone(),
        resume: conversation.session_started,
        system_prompt: match attempt.lost {
            true => format!("{system}\n\n{}", prompt::LOST_HISTORY),
            false => system,
        },
        mcp_config: mcp::config_json(ctx.base_url(), &conversation.id),
        add_dirs,
        model,
    };
    args::build(&turn)
}

/// Appends the error, if any, then sets `yourTurn` and frees the slot in one write.
async fn finish(ctx: &Context, id: &str, guard: TurnGuard, end: TurnEnd) {
    let store = match ctx.store() {
        Ok(store) => store,
        Err(e) => return eprintln!("warn: turn of {id} not finished: {e}"),
    };
    if let Some(message) = end.error {
        if let Err(e) = store.append_entry(id, EntryBody::Error { message }).await {
            eprintln!("warn: cannot note the failed turn of {id}: {e}");
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
        eprintln!("warn: turn of {id} not handed back: {e}");
    }
}
