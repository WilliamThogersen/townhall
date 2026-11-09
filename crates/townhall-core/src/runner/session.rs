//! What one `claude -p` run of a turn says: finished replies become `townhall` entries,
//! tool requests are answered (townhall's list is allowed, the rest denied), and the
//! result gives the context meter and, for a failure, the error entry's copy.

use std::path::Path;
use std::time::Instant;

use tokio::sync::watch;

use super::args::is_allowed;
use super::process::{Exit, Proc};
use super::prompt;
use super::stream::{parse_line, StreamEvent, TurnResult};
use crate::model::{ContextUsage, Conversation, EntryBody};
use crate::redact::redact;
use crate::{Context, Result};

/// The context window when the CLI does not say (older CLIs).
pub const DEFAULT_CONTEXT_LIMIT: u64 = 200_000;
/// What claude says when `--resume` cannot find the session.
const LOST: &str = "no conversation found";

/// What a finished run leaves behind.
#[derive(Debug, Default)]
pub(super) struct TurnEnd {
    pub error: Option<String>,
    pub context: Option<ContextUsage>,
    /// claude could not find the session it was asked to resume.
    pub lost: bool,
    /// The user stopped the turn.
    pub stopped: bool,
    pub cost_usd: Option<f64>,
    pub ran: bool,
    pub tokens: u64,
}

impl TurnEnd {
    pub fn failed(message: String) -> Self {
        Self {
            error: Some(message),
            ..Self::default()
        }
    }
}

/// What the running turn has seen so far.
struct Session<'a> {
    ctx: &'a Context,
    id: &'a str,
    started: bool,
    used: Option<u64>,
    limit: Option<u64>,
    result: Option<TurnResult>,
    /// The CLI printed `init`: it found (or created) the session.
    saw_init: bool,
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
            StreamEvent::Init => {
                self.saw_init = true;
                self.mark_started().await?;
            }
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
            lost: lost_session(self.result.as_ref(), &exit, self.saw_init),
            stopped: exit.stopped,
            cost_usd: self.result.as_ref().and_then(|r| r.cost_usd),
            ran: true,
            tokens: self.result.as_ref().and_then(|r| r.tokens).unwrap_or(0),
        }
    }
}

/// Whether claude could not find the session to resume. It says so on stderr and exits
/// before any stream output, so only a failed run without `init` counts, and only when
/// stderr or an error result says it: a reply that quotes the words is no such failure.
fn lost_session(result: Option<&TurnResult>, exit: &Exit, saw_init: bool) -> bool {
    let said = |text: Option<&str>| text.is_some_and(|t| t.to_lowercase().contains(LOST));
    let failed = result.is_none_or(|r| r.is_error);
    let error_result = result
        .filter(|r| r.is_error)
        .and_then(|r| r.message.as_deref());
    !exit.stopped && !saw_init && failed && (said(exit.last_error.as_deref()) || said(error_result))
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

pub(super) async fn talk(
    ctx: &Context,
    conversation: &Conversation,
    cwd: &Path,
    argv: &[String],
    user_line: &str,
    mut stop: watch::Receiver<bool>,
) -> Result<TurnEnd> {
    let started = Instant::now();
    let mut proc = match Proc::spawn(ctx.claude_bin(), argv, cwd) {
        Ok(proc) => proc,
        Err(e) => {
            tracing::warn!(
                target: "townhall::runner", event = "claude.spawn", ok = false,
                conversation = %conversation.id, error_kind = e.kind(),
                error = %redact(&e.to_string()), "cannot spawn claude"
            );
            return Ok(TurnEnd::failed(format!("cannot run claude: {e}")));
        }
    };
    tracing::info!(
        target: "townhall::runner", event = "claude.spawn", ok = true,
        conversation = %conversation.id, resume = conversation.session_started, "spawned claude"
    );
    proc.write(user_line).await;
    let mut session = Session {
        ctx,
        id: &conversation.id,
        started: conversation.session_started,
        used: None,
        limit: conversation.context.map(|c| c.limit),
        result: None,
        saw_init: false,
    };
    while let Some(line) = proc.next_line(&mut stop).await {
        for event in parse_line(&line) {
            session.on(event, &mut proc).await?;
        }
    }
    let exit = proc.finish().await;
    log_exit(&conversation.id, started, &exit);
    log_denials(&conversation.id, session.result.as_ref());
    Ok(session.end(exit))
}

fn log_exit(conversation: &str, started: Instant, exit: &Exit) {
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    // `code` is absent when claude was killed by a signal.
    tracing::info!(
        target: "townhall::runner", event = "claude.exit", conversation,
        code = exit.code, stopped = exit.stopped, duration_ms, "claude exited"
    );
}

/// Tool names only: the denied tools' input can hold file contents or commands.
fn log_denials(conversation: &str, result: Option<&TurnResult>) {
    let Some(result) = result.filter(|r| !r.denied.is_empty()) else {
        return;
    };
    tracing::warn!(
        target: "townhall::runner", event = "turn.permission_denials", conversation,
        count = result.denied.len(), tools = %result.denied.join(","), "tools were denied"
    );
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

    #[test]
    fn a_lost_session_is_a_failed_run_before_init() {
        let gone = Some("No conversation found with session ID: x");
        assert!(lost_session(None, &exit(Some(1), gone, false), false));
        assert!(!lost_session(None, &exit(Some(1), gone, true), false));
        assert!(!lost_session(None, &exit(Some(1), gone, false), true));
        assert!(!lost_session(
            None,
            &exit(Some(1), Some("boom"), false),
            false
        ));
        let error = TurnResult {
            is_error: true,
            message: Some("no conversation found with session id y".into()),
            ..TurnResult::default()
        };
        assert!(lost_session(
            Some(&error),
            &exit(Some(1), None, false),
            false
        ));
        assert!(!lost_session(
            Some(&error),
            &exit(Some(1), None, false),
            true
        ));
    }

    #[test]
    fn a_reply_that_quotes_the_words_is_no_lost_session() {
        let reply = TurnResult {
            message: Some("the logs say: No conversation found with session ID 42".into()),
            ..TurnResult::default()
        };
        let quoted = Some("No conversation found with session ID 42");
        assert!(!lost_session(
            Some(&reply),
            &exit(Some(0), quoted, false),
            false
        ));
        assert!(!lost_session(
            Some(&reply),
            &exit(Some(0), None, false),
            true
        ));
    }
}
