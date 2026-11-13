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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_sha: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrStatus {
    Pending,
    Rerunning,
    Red,
    Green,
    Updating,
    Conflict,
    Blocked,
    Merged,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedPr {
    pub repo: String,
    pub number: u64,
    pub url: Option<String>,
    pub branch: String,
    pub base: String,
    pub owner: String,
    pub pusher: Option<String>,
    pub head: Option<String>,
    pub status: PrStatus,
    pub detail: Option<String>,
    pub reruns: Vec<String>,
    pub woke_for: Option<String>,
    pub seen_pending: bool,
    pub updated_at: String,
}

/// Retries a task with a check gets when it names no `maxRetries`.
pub const DEFAULT_MAX_RETRIES: u32 = 2;

fn default_max_retries() -> u32 {
    DEFAULT_MAX_RETRIES
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VerifyPhase {
    /// The agent ended and the check runs (or is about to).
    Verifying,
    /// The check failed and a new agent run is queued or working on it.
    Retrying,
    Passed,
    /// The check failed after the last retry: the task needs the user.
    Failed,
}

/// Where a task's verification stands. `None` on the task until its agent first ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verify {
    pub phase: VerifyPhase,
    /// Retries started so far: 0 while the first run is verified.
    pub retries: u32,
    /// The last failing check's output (capped), kept for the brief and the user.
    pub output: Option<String>,
}

/// A task's "done when" criterion and the check that proves it. Flattened into the task's
/// JSON (`doneWhen`, `check`, `maxRetries`, `verify`), so a task without one reads as before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Done {
    /// Free text: what finished means. Shown to people and put in the brief.
    #[serde(default)]
    pub done_when: Option<String>,
    /// Shell command run in the task's worktree when its agent ends; exit 0 passes.
    #[serde(default)]
    pub check: Option<String>,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub verify: Option<Verify>,
}

impl Default for Done {
    fn default() -> Self {
        Self {
            done_when: None,
            check: None,
            max_retries: DEFAULT_MAX_RETRIES,
            verify: None,
        }
    }
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prs: Vec<TrackedPr>,
    #[serde(flatten)]
    pub done: Done,
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
    /// Its `verify` is ignored: a new task has none.
    pub done: Done,
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
    /// What finished means, as free text.
    #[serde(default)]
    pub done_when: Option<String>,
    /// Shell command run in the worktree when the agent ends; a failure relaunches the task.
    #[serde(default)]
    pub check: Option<String>,
    /// Relaunches after a failing check. Default `DEFAULT_MAX_RETRIES`.
    #[serde(default)]
    pub max_retries: Option<u32>,
    #[serde(default)]
    pub pipeline: Option<String>,
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
                merge_sha: None,
            }),
            agent: "builder".into(),
            blocked: None,
            pr_base: None,
            prs: vec![],
            done: Default::default(),
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

    fn tracked() -> TrackedPr {
        TrackedPr {
            repo: "shop".into(),
            number: 7,
            url: Some("https://github.com/o/r/pull/7".into()),
            branch: "townhall/fix-login".into(),
            base: "main".into(),
            owner: "fix-login".into(),
            pusher: None,
            head: Some("abc123".into()),
            status: PrStatus::Red,
            detail: None,
            reruns: vec![],
            woke_for: None,
            seen_pending: false,
            updated_at: "2026-11-04T10:42:05.000Z".into(),
        }
    }

    #[test]
    fn tracked_prs_are_listed_on_the_task_only_when_there_are_some() {
        let mut t = Task {
            name: "fix-login".into(),
            title: "t".into(),
            prompt: "p".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: vec![],
            state: TaskState::Ended,
            conversation_id: "a1b2c3d4e5f6".into(),
            attachments: vec![],
            created_at: "2026-11-04T10:42:05.000Z".into(),
            pr: None,
            agent: "lander".into(),
            blocked: None,
            pr_base: None,
            prs: vec![],
            done: Default::default(),
        };
        let v = serde_json::to_value(&t).unwrap();
        assert!(v.get("prs").is_none());
        t.prs.push(tracked());
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["prs"][0]["status"], "red");
        assert_eq!(v["prs"][0]["wokeFor"], json!(null));
        assert_eq!(v["prs"][0]["seenPending"], false);
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
    fn a_task_without_a_check_reads_as_before() {
        let old = json!({
            "name": "a", "title": "t", "prompt": "p", "repo": "shop", "baseBranch": "main",
            "after": [], "state": "queued", "conversationId": "c", "attachments": [],
            "createdAt": "2026-11-04T10:42:05.000Z", "pr": null, "agent": "builder",
            "blocked": null, "prBase": null
        });
        let t: Task = serde_json::from_value(old).unwrap();
        assert_eq!(t.done, Done::default());
        assert_eq!(t.done.max_retries, DEFAULT_MAX_RETRIES);
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["doneWhen"], json!(null));
        assert_eq!(v["check"], json!(null));
        assert_eq!(v["maxRetries"], 2);
        assert_eq!(v["verify"], json!(null));
    }

    #[test]
    fn a_task_carries_its_done_when_check_and_verify_state() {
        let done = Done {
            done_when: Some("all tests pass".into()),
            check: Some("npm test".into()),
            max_retries: 3,
            verify: Some(Verify {
                phase: VerifyPhase::Retrying,
                retries: 1,
                output: Some("1 failed".into()),
            }),
        };
        let t = Task {
            done,
            ..serde_json::from_value(json!({
                "name": "a", "title": "t", "prompt": "p", "repo": "shop", "baseBranch": "main",
                "after": [], "state": "working", "conversationId": "c", "attachments": [],
                "createdAt": "2026-11-04T10:42:05.000Z", "pr": null, "agent": "builder",
                "blocked": null, "prBase": null
            }))
            .unwrap()
        };
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["doneWhen"], "all tests pass");
        assert_eq!(v["check"], "npm test");
        assert_eq!(v["maxRetries"], 3);
        assert_eq!(
            v["verify"],
            json!({"phase": "retrying", "retries": 1, "output": "1 failed"})
        );
        assert_eq!(serde_json::from_value::<Task>(v).unwrap(), t);
    }

    #[test]
    fn create_task_input_takes_the_check_fields() {
        let i: CreateTaskInput = serde_json::from_value(json!({
            "name": "a", "title": "t", "prompt": "p",
            "doneWhen": "tests pass", "check": "cargo test", "maxRetries": 1
        }))
        .unwrap();
        assert_eq!(i.done_when.as_deref(), Some("tests pass"));
        assert_eq!(i.check.as_deref(), Some("cargo test"));
        assert_eq!(i.max_retries, Some(1));
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
