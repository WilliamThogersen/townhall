//! One turn of townhall's own session: resolve the repo, run `claude -p`, store each
//! finished reply as a `townhall` entry, answer its tool requests, then hand the turn back.

use std::path::{Path, PathBuf};

use tokio::sync::watch;

use super::args::{self, is_allowed, TurnArgs};
use super::content::user_content;
use super::process::{Exit, Proc};
use super::prompt::{self, RepoNote};
use super::repo::{self, Message, Resolved};
use super::stream::{parse_line, StreamEvent, TurnResult};
use super::turns::TurnGuard;
use crate::model::{ContextUsage, Conversation, ConversationStatus, EntryBody, NewMessage, Repo};
use crate::{mcp, Context, Result};

/// The context window when the CLI does not say (older CLIs).
pub const DEFAULT_CONTEXT_LIMIT: u64 = 200_000;

/// What a finished turn leaves behind.
#[derive(Debug, Default)]
struct TurnEnd {
    error: Option<String>,
    context: Option<ContextUsage>,
}

impl TurnEnd {
    fn failed(message: String) -> Self {
        Self {
            error: Some(message),
            context: None,
        }
    }
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
    let store = ctx.store()?;
    let conversation = store.conversation(id).await?;
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
    if guard.stop_requested() {
        return Ok(TurnEnd::default());
    }
    let cwd = session_dir(ctx, &conversation, &note).await?;
    if !cwd.is_dir() {
        return Ok(TurnEnd::failed(format!(
            "the folder this conversation runs in is gone ({}): start a new conversation",
            cwd.display()
        )));
    }
    let argv = plan(ctx, &conversation, &note, &cwd).await?;
    let content = user_content(store, id, &text, &attachments).await?;
    let line = prompt::user_line(content);
    talk(ctx, &conversation, &cwd, &argv, &line, guard.stop_signal()).await
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
    note: &RepoNote,
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
    let turn = TurnArgs {
        session_id: conversation.session_id.clone(),
        resume: conversation.session_started,
        system_prompt: prompt::system_prompt(note, &repos),
        mcp_config: mcp::config_json(ctx.base_url(), &conversation.id),
        add_dirs,
    };
    Ok(args::build(&turn))
}

/// What the running turn has seen so far.
struct Session<'a> {
    ctx: &'a Context,
    id: &'a str,
    started: bool,
    used: Option<u64>,
    limit: Option<u64>,
    result: Option<TurnResult>,
}

impl Session<'_> {
    async fn mark_started(&mut self) -> Result<()> {
        if self.started {
            return Ok(());
        }
        self.started = true;
        let started = |c: Conversation| {
            Ok(Conversation {
                session_started: true,
                ..c
            })
        };
        self.ctx
            .store()?
            .update_conversation(self.id, started)
            .await
            .map(|_| ())
    }

    async fn on(&mut self, event: StreamEvent, proc: &mut Proc) -> Result<()> {
        match event {
            StreamEvent::Init => self.mark_started().await?,
            StreamEvent::Text(text) => {
                let reply = EntryBody::Townhall { text };
                self.ctx.store()?.append_entry(self.id, reply).await?;
            }
            StreamEvent::Usage(used) => self.used = Some(used),
            StreamEvent::ToolRequest {
                request_id,
                tool,
                input,
            } => {
                let line = match is_allowed(&tool) {
                    true => prompt::allow_line(&request_id, &input),
                    false => prompt::deny_line(&request_id, &tool),
                };
                proc.write(&line).await;
            }
            StreamEvent::OtherRequest {
                request_id,
                subtype,
            } => proc.write(&prompt::error_line(&request_id, &subtype)).await,
            StreamEvent::Result(result) => {
                if !result.is_error && !result.stopped {
                    self.mark_started().await?;
                }
                self.result = Some(result);
                proc.close_stdin();
            }
        }
        Ok(())
    }

    fn end(self, exit: Exit) -> TurnEnd {
        let context = self.result.as_ref().and_then(|r| {
            let used = self.used.or(r.used)?;
            let limit = r.limit.or(self.limit).unwrap_or(DEFAULT_CONTEXT_LIMIT);
            Some(ContextUsage { used, limit })
        });
        TurnEnd {
            error: failure(self.result.as_ref(), &exit),
            context,
        }
    }
}

/// The error entry's copy, or `None` for a turn that ended well or was stopped.
fn failure(result: Option<&TurnResult>, exit: &Exit) -> Option<String> {
    if exit.stopped {
        return None;
    }
    match result {
        Some(r) if r.is_error => {
            let detail = r
                .message
                .as_deref()
                .unwrap_or("the turn ended with an error");
            Some(format!("claude failed: {detail}"))
        }
        Some(_) => None,
        None => Some(match (&exit.last_error, exit.code) {
            (Some(line), _) => format!("claude failed: {line}"),
            (None, Some(code)) => format!("claude exited with {code} before answering"),
            (None, None) => "claude was killed before answering".to_string(),
        }),
    }
}

async fn talk(
    ctx: &Context,
    conversation: &Conversation,
    cwd: &Path,
    argv: &[String],
    user_line: &str,
    mut stop: watch::Receiver<bool>,
) -> Result<TurnEnd> {
    let mut proc = match Proc::spawn(ctx.claude_bin(), argv, cwd) {
        Ok(proc) => proc,
        Err(e) => return Ok(TurnEnd::failed(format!("cannot run claude: {e}"))),
    };
    proc.write(user_line).await;
    let mut session = Session {
        ctx,
        id: &conversation.id,
        started: conversation.session_started,
        used: None,
        limit: conversation.context.map(|c| c.limit),
        result: None,
    };
    while let Some(line) = proc.next_line(&mut stop).await {
        for event in parse_line(&line) {
            session.on(event, &mut proc).await?;
        }
    }
    let exit = proc.finish().await;
    Ok(session.end(exit))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn exit(code: Option<i32>, last_error: Option<&str>, stopped: bool) -> Exit {
        Exit {
            code,
            last_error: last_error.map(String::from),
            stopped,
        }
    }

    #[test]
    fn failures_have_lowercase_copy() {
        let ok = TurnResult::default();
        assert_eq!(failure(Some(&ok), &exit(Some(0), None, false)), None);
        let bad = TurnResult {
            is_error: true,
            message: Some("Invalid API key".into()),
            ..TurnResult::default()
        };
        assert_eq!(
            failure(Some(&bad), &exit(Some(1), None, false)).as_deref(),
            Some("claude failed: Invalid API key")
        );
        assert_eq!(failure(Some(&bad), &exit(Some(1), None, true)), None);
        assert_eq!(
            failure(None, &exit(Some(3), Some("boom"), false)).as_deref(),
            Some("claude failed: boom")
        );
        assert_eq!(
            failure(None, &exit(Some(3), None, false)).as_deref(),
            Some("claude exited with 3 before answering")
        );
        assert_eq!(
            failure(None, &exit(None, None, false)).as_deref(),
            Some("claude was killed before answering")
        );
        let silent = TurnResult {
            is_error: true,
            ..TurnResult::default()
        };
        assert_eq!(
            failure(Some(&silent), &exit(Some(1), None, false)).as_deref(),
            Some("claude failed: the turn ended with an error")
        );
    }
}
