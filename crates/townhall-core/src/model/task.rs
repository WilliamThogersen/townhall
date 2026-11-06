use serde::{Deserialize, Serialize};

/// Every task's branch is `townhall/<name>`, which is also the agent's branch.
pub const TASK_BRANCH_PREFIX: &str = "townhall/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    #[default]
    Queued,
    Working,
    NeedsYou,
    InCi,
    InReview,
    Merged,
    Ended,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Checks {
    Pending,
    Passed,
    Failed,
    /// The repo runs no checks on the pull request.
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pr {
    pub number: u64,
    pub url: Option<String>,
    pub checks: Checks,
}

/// What an agent picks up. Stored in its conversation's `tasks.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique across every repo: also the agent name and the branch `townhall/<name>`.
    pub name: String,
    pub title: String,
    pub prompt: String,
    /// Repo name from the registry.
    pub repo: String,
    pub base_branch: String,
    /// Names of the tasks that must finish first.
    pub after: Vec<String>,
    pub state: TaskState,
    pub conversation_id: String,
    /// Attachment ids of its conversation; the agent gets their paths.
    pub attachments: Vec<String>,
    pub created_at: String,
    pub pr: Option<Pr>,
    /// The agent type it runs as (`AgentType`), resolved when it was created.
    pub agent: String,
    /// Why a queued task whose `after` tasks finished still waits: its agent type's `requires`
    /// are not met. Set and cleared by the scheduler, `None` when nothing holds it back.
    pub blocked: Option<String>,
    pub pr_base: Option<String>,
}

impl Task {
    pub fn pr_target(&self) -> &str {
        self.pr_base.as_deref().unwrap_or(&self.base_branch)
    }

    pub fn branch(&self) -> String {
        format!("{TASK_BRANCH_PREFIX}{}", self.name)
    }
}

/// The most times one task can be handed back, so two types cannot bounce it forever.
pub const MAX_HANDOFFS: usize = 5;

/// One time an agent handed its task back to another agent type, in the same worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handoff {
    pub task: String,
    pub from: String,
    pub to: String,
    pub reason: String,
    /// What the handing agent did, as it told or as its commits show.
    pub summary: String,
    pub at: String,
}

/// A task to store, with its repo and base branch already resolved (2.3 resolves them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTask {
    pub name: String,
    pub title: String,
    pub prompt: String,
    pub repo: String,
    pub base_branch: String,
    pub after: Vec<String>,
    pub attachments: Vec<String>,
    /// The stored name of its agent type.
    pub agent: String,
    pub pr_base: Option<String>,
}

/// The `create_task` MCP tool's arguments (see `docs/daemon-api.md`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateTaskInput {
    pub name: String,
    pub title: String,
    pub prompt: String,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub base_branch: Option<String>,
    #[serde(default)]
    pub after: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<String>,
    /// The agent type's name. Default: `builder` when it exists, else the first by name.
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub pr_base: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_shape() {
        let t = Task {
            name: "fix-login".into(),
            title: "fix the login redirect".into(),
            prompt: "the login page loops".into(),
            repo: "shop".into(),
            base_branch: "develop".into(),
            after: vec!["add-tests".into()],
            state: TaskState::NeedsYou,
            conversation_id: "a1b2c3d4e5f6".into(),
            attachments: vec![],
            created_at: "2026-11-04T10:42:05.000Z".into(),
            pr: Some(Pr {
                number: 41,
                url: None,
                checks: Checks::Failed,
            }),
            agent: "builder".into(),
            blocked: None,
            pr_base: None,
        };
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["state"], "needsYou");
        assert_eq!(v["blocked"], json!(null));
        assert_eq!(v["baseBranch"], "develop");
        assert_eq!(v["prBase"], json!(null));
        assert_eq!(t.pr_target(), "develop");
        let aimed = Task {
            pr_base: Some("main".into()),
            ..t.clone()
        };
        assert_eq!(aimed.pr_target(), "main");
        assert_eq!(v["conversationId"], "a1b2c3d4e5f6");
        assert_eq!(v["agent"], "builder");
        assert_eq!(
            v["pr"],
            json!({"number": 41, "url": null, "checks": "failed"})
        );
        assert_eq!(t.branch(), "townhall/fix-login");
        assert_eq!(serde_json::from_value::<Task>(v).unwrap(), t);
    }

    #[test]
    fn every_state_name() {
        let names: Vec<String> = [
            TaskState::Queued,
            TaskState::Working,
            TaskState::NeedsYou,
            TaskState::InCi,
            TaskState::InReview,
            TaskState::Merged,
            TaskState::Ended,
            TaskState::Error,
        ]
        .iter()
        .map(|s| {
            serde_json::to_value(s)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
        assert_eq!(
            names,
            ["queued", "working", "needsYou", "inCi", "inReview", "merged", "ended", "error"]
        );
        assert_eq!(serde_json::to_value(Checks::None).unwrap(), "none");
    }

    #[test]
    fn create_task_input_defaults_and_strictness() {
        let i: CreateTaskInput =
            serde_json::from_value(json!({"name": "a", "title": "t", "prompt": "p"})).unwrap();
        assert!(i.repo.is_none() && i.after.is_empty() && i.attachments.is_empty());
        let bad = json!({"name": "a", "title": "t", "prompt": "p", "permissionMode": "bypassPermissions"});
        assert!(serde_json::from_value::<CreateTaskInput>(bad).is_err());
    }
}
