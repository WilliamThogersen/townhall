//! What one `claude -p` run of a turn says: finished replies become `townhall` entries,
//! tool requests are answered (townhall's list is allowed, the rest denied), and the
//! result gives the context meter and, for a failure, the error entry's copy.

use std::path::Path;

use tokio::sync::watch;

use super::args::is_allowed;
use super::process::{Exit, Proc};
use super::prompt;
use super::stream::{parse_line, StreamEvent, TurnResult};
use crate::model::{ContextUsage, Conversation, EntryBody};
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
            lost: lost_session(self.result.as_ref(), &exit),
        }
    }
}

/// Whether claude said it has no such session (a resume of a session it lost).
fn lost_session(result: Option<&TurnResult>, exit: &Exit) -> bool {
    let said = |text: Option<&str>| text.is_some_and(|t| t.to_lowercase().contains(LOST));
    !exit.stopped
        && (said(result.and_then(|r| r.message.as_deref())) || said(exit.last_error.as_deref()))
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
    fn a_lost_session_is_told_apart() {
        let gone = Some("No conversation found with session ID: x");
        assert!(lost_session(None, &exit(Some(1), gone, false)));
        assert!(!lost_session(None, &exit(Some(1), gone, true)));
        assert!(!lost_session(None, &exit(Some(1), Some("boom"), false)));
        let said = TurnResult {
            is_error: true,
            message: Some("no conversation found with session id y".into()),
            ..TurnResult::default()
        };
        assert!(lost_session(Some(&said), &exit(Some(1), None, false)));
    }
}
