//! Agent state detection. The 2.1 baseline: a live session is working, a dead one ended.
//! Unit 2.5 replaces `detect` (waiting with the question text, error with the exit code)
//! and keeps this signature, so callers do not change.

use super::Agent;
use crate::model::AgentState;
use crate::Workspace;

pub async fn detect(_ws: &Workspace, agent: &Agent) -> AgentState {
    if agent.alive {
        AgentState::Working
    } else {
        AgentState::Ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Home;

    #[tokio::test]
    async fn alive_is_working_and_dead_is_ended() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::new(Home::at(dir.path().join("h")), dir.path()).unwrap();
        let agent = |alive| Agent {
            name: "a".into(),
            kind: None,
            branch: "townhall/a".into(),
            worktree: dir.path().join("a"),
            alive,
        };
        assert_eq!(detect(&ws, &agent(true)).await, AgentState::Working);
        assert_eq!(detect(&ws, &agent(false)).await, AgentState::Ended);
    }
}
