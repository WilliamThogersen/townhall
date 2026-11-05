//! Claude's folder trust dialog in townhall's own worktrees is answered for you, once per
//! session: `up` onto `Yes, I trust this folder`, then `enter`. The cursor starts on
//! `No, exit`, so text or a bare `enter` would make claude exit. Any other prompt, and any
//! agent outside `<home>/worktrees/`, is left to you.

use std::path::{Component, Path};
use std::time::Duration;

use crate::agent::{self, state};
use crate::model::{AgentEvent, AgentInfo, AgentKey, AgentState, EntryBody};
use crate::{Result, Workspace};

/// The text of the `agent` `sent` entry a task's conversation gets.
pub(super) const TRUSTED: &str = "trusted the worktree";
/// The pause between the two keys, so the dialog reads them as two presses.
const KEY_GAP: Duration = Duration::from_millis(200);

/// True when `info` shows exactly the trust dialog and its worktree is below `worktrees`.
pub(super) fn answers(info: &AgentInfo, worktrees: &Path) -> bool {
    let asks_trust = matches!(
        &info.state,
        AgentState::Waiting { question } if question == state::TRUST_QUESTION
    );
    asks_trust && inside(Path::new(&info.worktree), worktrees)
}

/// `path` is a folder below `root`, with no `..` that could climb back out.
fn inside(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root) && !path.components().any(|c| c == Component::ParentDir)
}

/// Presses `up`, then `enter`.
pub(super) async fn answer(ws: &Workspace, name: &str) -> Result<()> {
    agent::send_key(ws, name, AgentKey::Up).await?;
    tokio::time::sleep(KEY_GAP).await;
    agent::send_key(ws, name, AgentKey::Enter).await
}

pub(super) fn entry(agent: &str) -> EntryBody {
    EntryBody::Agent {
        agent: agent.to_string(),
        event: AgentEvent::Sent,
        text: TRUSTED.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/h/.townhall/worktrees";

    fn info(worktree: &str, state: AgentState) -> AgentInfo {
        AgentInfo {
            name: "fix".into(),
            repo: "shop".into(),
            kind: None,
            branch: "townhall/fix".into(),
            worktree: worktree.into(),
            alive: true,
            state,
        }
    }

    fn asking(question: &str) -> AgentState {
        AgentState::Waiting {
            question: question.into(),
        }
    }

    #[test]
    fn the_trust_dialog_in_a_townhall_worktree_is_answered() {
        let trust = asking(state::TRUST_QUESTION);
        assert!(answers(
            &info("/h/.townhall/worktrees/shop-1/fix", trust),
            Path::new(ROOT)
        ));
    }

    #[test]
    fn the_trust_dialog_outside_townhall_worktrees_is_not() {
        for outside in [
            "/Users/me/code/shop",
            "/h/.townhall/worktrees2/shop-1/fix",
            "/h/.townhall/worktrees/../repos/shop",
            "/h/.townhall/worktrees",
            "/h/.townhall",
            "",
        ] {
            let trust = asking(state::TRUST_QUESTION);
            assert!(
                !answers(&info(outside, trust), Path::new(ROOT)),
                "{outside}"
            );
        }
    }

    #[test]
    fn no_other_prompt_is_answered() {
        let worktree = "/h/.townhall/worktrees/shop-1/fix";
        for state in [
            asking("Do you want to proceed?\n1. Yes\n2. No"),
            asking("Do you trust the files in this folder?\n1. Yes, proceed\n2. No, exit"),
            asking(&format!("{}\n1. Yes", state::TRUST_QUESTION)),
            asking("continue? [y/n]"),
            AgentState::Working,
            AgentState::Ended,
            AgentState::Error { exit_code: Some(1) },
        ] {
            assert!(
                !answers(&info(worktree, state.clone()), Path::new(ROOT)),
                "{state:?}"
            );
        }
    }

    #[test]
    fn the_entry_says_the_worktree_was_trusted() {
        assert_eq!(
            entry("fix"),
            EntryBody::Agent {
                agent: "fix".into(),
                event: AgentEvent::Sent,
                text: "trusted the worktree".into(),
            }
        );
    }
}
