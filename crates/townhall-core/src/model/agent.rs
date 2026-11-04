use serde::{Deserialize, Serialize};

use crate::agent::Agent;

/// What an agent is doing. 2.1 knows only alive (`working`) and dead (`ended`); 2.5 adds the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentState {
    Working,
    /// Waits for you; `question` is the agent's text, verbatim.
    Waiting {
        question: String,
    },
    /// Its process exited with a failure; the code when known.
    Error {
        exit_code: Option<i32>,
    },
    Ended,
}

/// `GET /api/agents` rows: the phase 1 fields plus `repo` and `state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub name: String,
    /// Repo name from the registry.
    pub repo: String,
    pub kind: Option<String>,
    pub branch: String,
    pub worktree: String,
    pub alive: bool,
    pub state: AgentState,
}

impl AgentInfo {
    pub fn new(agent: Agent, repo: &str, state: AgentState) -> Self {
        Self {
            name: agent.name,
            repo: repo.to_string(),
            kind: agent.kind,
            branch: agent.branch,
            worktree: agent.worktree.to_string_lossy().into_owned(),
            alive: agent.alive,
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn states_are_tagged_by_kind() {
        let cases = [
            (AgentState::Working, json!({"kind": "working"})),
            (
                AgentState::Waiting {
                    question: "proceed?".into(),
                },
                json!({"kind": "waiting", "question": "proceed?"}),
            ),
            (
                AgentState::Error { exit_code: Some(2) },
                json!({"kind": "error", "exitCode": 2}),
            ),
            (AgentState::Ended, json!({"kind": "ended"})),
        ];
        for (state, want) in cases {
            assert_eq!(serde_json::to_value(&state).unwrap(), want);
            assert_eq!(serde_json::from_value::<AgentState>(want).unwrap(), state);
        }
    }

    #[test]
    fn info_keeps_phase_one_fields() {
        let agent = Agent {
            name: "a".into(),
            kind: None,
            branch: "townhall/a".into(),
            worktree: "/w/a".into(),
            alive: true,
        };
        let v = serde_json::to_value(AgentInfo::new(agent, "shop", AgentState::Working)).unwrap();
        assert_eq!(
            v,
            json!({
                "name": "a", "repo": "shop", "kind": null, "branch": "townhall/a",
                "worktree": "/w/a", "alive": true, "state": {"kind": "working"}
            })
        );
    }
}
