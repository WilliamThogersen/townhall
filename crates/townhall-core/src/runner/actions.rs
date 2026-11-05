//! What the conversation routes do: create, send, stop, end and patch. Input is checked
//! here, so the routes stay thin and core tests cover it.

use std::time::Duration;

use super::process::{KILL_WAIT, STOP_GRACE};
use super::turn;
use crate::model::{
    Conversation, ConversationDetail, ConversationPatch, ConversationStatus, EntryBody, NewMessage,
    ThreadEntry,
};
use crate::redact::redact;
use crate::store::Store;
use crate::{ids, mcp, reaper, Context, Error, Result};

pub const MAX_MESSAGE_CHARS: usize = 100_000;
pub const MAX_ATTACHMENTS: usize = 20;
/// How long `stop` waits for the turn to wind down before it answers: the grace, the
/// wait for the pipes after the kill, and a moment to hand the turn back.
const STOP_WAIT: Duration = Duration::from_secs(STOP_GRACE.as_secs() + KILL_WAIT.as_secs() + 2);
const NOT_STUCK: &str = "not stuck";

fn check_message(m: &NewMessage) -> Result<()> {
    let bad = |m: String| Err(Error::InvalidInput(m));
    if m.text.trim().is_empty() && m.attachments.is_empty() {
        return bad("a message needs text or an attachment".into());
    }
    if m.text.chars().count() > MAX_MESSAGE_CHARS {
        return bad(format!(
            "a message is at most {MAX_MESSAGE_CHARS} characters"
        ));
    }
    if m.attachments.len() > MAX_ATTACHMENTS {
        return bad(format!("at most {MAX_ATTACHMENTS} attachments per message"));
    }
    m.attachments
        .iter()
        .try_for_each(|id| ids::validate_id("attachment", id))
}

async fn check_attachments(store: &Store, id: &str, wanted: &[String]) -> Result<()> {
    if wanted.is_empty() {
        return Ok(());
    }
    let known = store.attachments(id).await?;
    match wanted.iter().find(|w| !known.iter().any(|a| &a.id == *w)) {
        Some(unknown) => Err(Error::InvalidInput(format!(
            "unknown attachment `{unknown}`"
        ))),
        None => Ok(()),
    }
}

fn check_title(title: &str) -> Result<String> {
    let title = title.trim();
    let max = usize::try_from(mcp::MAX_TITLE_LEN).unwrap_or(usize::MAX);
    if title.is_empty() {
        return Err(Error::InvalidInput("a title needs text".into()));
    }
    if title.chars().count() > max {
        return Err(Error::InvalidInput(format!(
            "a title is at most {max} characters"
        )));
    }
    if title.chars().any(char::is_control) {
        return Err(Error::InvalidInput("a title is one line of text".into()));
    }
    Ok(title.to_string())
}

/// Creates the conversation and starts its first turn with `message`.
pub async fn create(ctx: &Context, message: NewMessage) -> Result<ConversationDetail> {
    check_message(&message)?;
    if let Some(id) = message.attachments.first() {
        return Err(Error::InvalidInput(format!("unknown attachment `{id}`")));
    }
    let store = ctx.store()?;
    let conversation = store.create_conversation().await?;
    send(ctx, &conversation.id, message).await?;
    store.detail(&conversation.id).await
}

#[derive(Debug)]
pub struct StartFile {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

pub async fn create_with_files(
    ctx: &Context,
    text: String,
    files: Vec<StartFile>,
) -> Result<ConversationDetail> {
    let blank = NewMessage {
        text,
        attachments: vec!["0".repeat(12); files.len()],
    };
    check_message(&blank)?;
    let store = ctx.store()?;
    let conversation = store.create_conversation().await?;
    let mut ids = Vec::with_capacity(files.len());
    for file in files {
        let saved = store
            .save_attachment(&conversation.id, &file.name, &file.mime, file.bytes)
            .await?;
        ids.push(saved.id);
    }
    let message = NewMessage {
        text: blank.text,
        attachments: ids,
    };
    send(ctx, &conversation.id, message).await?;
    store.detail(&conversation.id).await
}

/// Appends your message and starts a turn. `Conflict` while one runs.
pub async fn send(ctx: &Context, id: &str, message: NewMessage) -> Result<ThreadEntry> {
    check_message(&message)?;
    let store = ctx.store()?;
    store.conversation(id).await?;
    check_attachments(store, id, &message.attachments).await?;
    let guard = ctx.turns().begin(id)?;
    let yours = EntryBody::You {
        text: message.text.clone(),
        attachments: message.attachments.clone(),
    };
    open_turn(ctx, id, guard, vec![yours], message, false).await
}

/// Sets `thinking`, appends `entries` (your message, or the task events) and spawns the
/// turn for `message`. Returns the last entry. `event` turns leave `ended` alone and skip
/// repo inference.
pub(super) async fn open_turn(
    ctx: &Context,
    id: &str,
    guard: super::TurnGuard,
    entries: Vec<EntryBody>,
    message: NewMessage,
    event: bool,
) -> Result<ThreadEntry> {
    let store = ctx.store()?;
    let thinking = move |c: Conversation| {
        Ok(Conversation {
            status: ConversationStatus::Thinking,
            ended: c.ended && event,
            ..c
        })
    };
    let opened = match store.update_conversation(id, thinking).await {
        Ok(_) => append_all(store, id, entries).await,
        Err(e) => Err(e),
    };
    let entry = match opened {
        Ok(entry) => entry,
        Err(e) => {
            give_back(store, id, guard).await;
            return Err(e);
        }
    };
    let generation = guard.generation();
    let task = tokio::spawn(turn::run(
        ctx.clone(),
        id.to_string(),
        message,
        event,
        guard,
    ));
    ctx.turns().attach(id, generation, task.abort_handle());
    Ok(entry)
}

async fn append_all(store: &Store, id: &str, entries: Vec<EntryBody>) -> Result<ThreadEntry> {
    let mut last = None;
    for body in entries {
        last = Some(store.append_entry(id, body).await?);
    }
    last.ok_or_else(|| Error::InvalidInput("a turn needs an entry".into()))
}

async fn give_back(store: &Store, id: &str, guard: super::TurnGuard) {
    let idle = move |c: Conversation| {
        drop(guard);
        Ok(Conversation {
            status: ConversationStatus::YourTurn,
            ..c
        })
    };
    if let Err(e) = store.update_conversation(id, idle).await {
        tracing::warn!(
            target: "townhall::runner", event = "turn.give_back_failed", conversation = id,
            error_kind = e.kind(), error = %redact(&e.to_string()),
            "cannot hand the conversation back after a failed send"
        );
    }
}

/// Sets `yourTurn` on a conversation left `thinking` with no turn running. False when
/// it was not stuck.
pub(super) async fn unstick(ctx: &Context, id: &str) -> Result<bool> {
    let (turns, cid) = (ctx.turns().clone(), id.to_string());
    let idle = move |c: Conversation| {
        if c.status != ConversationStatus::Thinking || turns.is_running(&cid) {
            return Err(Error::Conflict(NOT_STUCK.into()));
        }
        Ok(Conversation {
            status: ConversationStatus::YourTurn,
            ..c
        })
    };
    match ctx.store()?.update_conversation(id, idle).await {
        Ok(_) => Ok(true),
        Err(Error::Conflict(m)) if m == NOT_STUCK => Ok(false),
        Err(e) => Err(e),
    }
}

/// Interrupts the running turn and waits a little for it to end.
pub async fn stop(ctx: &Context, id: &str) -> Result<Conversation> {
    let store = ctx.store()?;
    store.conversation(id).await?;
    match ctx.turns().stop(id) {
        Some(mut done) => {
            if tokio::time::timeout(STOP_WAIT, done.changed())
                .await
                .is_err()
            {
                tracing::warn!(
                    target: "townhall::runner", event = "turn.stop_timeout", conversation = id,
                    "the turn did not stop in time"
                );
            }
        }
        None => {
            unstick(ctx, id).await?;
        }
    }
    store.conversation(id).await
}

/// Stops the turn, moves the conversation under older and cancels its queued tasks. The
/// reaper then removes its agents and their worktrees in the background.
pub async fn end(ctx: &Context, id: &str) -> Result<Conversation> {
    stop(ctx, id).await?;
    let ended = |c: Conversation| Ok(Conversation { ended: true, ..c });
    let store = ctx.store()?;
    let conversation = store.update_conversation(id, ended).await?;
    reaper::cancel_queued(store, &store.tasks(id).await?).await?;
    reaper::soon(ctx);
    Ok(conversation)
}

/// `linkedRepo` (a registered repo, `null` for auto) and `title`.
pub async fn patch(ctx: &Context, id: &str, patch: ConversationPatch) -> Result<Conversation> {
    let store = ctx.store()?;
    let current = store.conversation(id).await?;
    let title = patch.title.as_deref().map(check_title).transpose()?;
    let linked = match patch.linked_repo {
        Some(Some(name)) => Some(Some(ctx.repo(&name).await?.name)),
        other => other,
    };
    if title.is_none() && linked.is_none() {
        return Ok(current);
    }
    let change = move |c: Conversation| {
        let title = title.unwrap_or_else(|| c.title.clone());
        let linked_repo = linked.unwrap_or_else(|| c.linked_repo.clone());
        Ok(Conversation {
            title,
            linked_repo,
            ..c
        })
    };
    store.update_conversation(id, change).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(text: &str, attachments: &[&str]) -> NewMessage {
        NewMessage {
            text: text.into(),
            attachments: attachments.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn messages_are_checked() {
        assert!(check_message(&msg("hi", &[])).is_ok());
        assert!(check_message(&msg(" ", &["a1"])).is_ok());
        let bad = [
            msg(" \n", &[]),
            msg(&"x".repeat(MAX_MESSAGE_CHARS + 1), &[]),
            msg("hi", &["../x"]),
            msg("hi", &["a"; MAX_ATTACHMENTS + 1]),
        ];
        for m in bad {
            assert!(
                matches!(check_message(&m), Err(Error::InvalidInput(_))),
                "{m:?}"
            );
        }
    }

    #[test]
    fn titles_are_one_short_line() {
        assert_eq!(check_title("  login loop ").unwrap(), "login loop");
        assert!(check_title(&"é".repeat(80)).is_ok());
        for bad in ["", "  ", "a\nb", &"x".repeat(81)] {
            assert!(check_title(bad).is_err(), "{bad}");
        }
    }
}
