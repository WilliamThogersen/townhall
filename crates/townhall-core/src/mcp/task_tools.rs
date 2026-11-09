//! `create_task`, `set_title`, `task_status` (`status.rs`), the agent type tools
//! (`library.rs`) and the log tools (`logs.rs`) for one conversation's session.
//! None takes a permission mode: a task's agent gets the saved setting when the scheduler
//! starts it, narrowed by its agent type.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::context::Context;
use crate::model::{Conversation, CreateTaskInput, Done, NewTask, Task, DEFAULT_MAX_RETRIES};
use crate::redact::redact;

use super::artifacts::{self, ADD_ARTIFACT, LIST_ARTIFACTS};
use super::checks::{self, Checked};
use super::hand_back::{self, HAND_BACK};
use super::library::{self, LIST_AGENT_TYPES, READ_AGENT_TYPE, SAVE_AGENT_TYPE};
use super::logs::{self as log_tools, LOG_SUMMARY, READ_LOGS};
use super::open_pr::{self, OPEN_PR};
use super::rpc::ToolRunner;
use super::{
    status, task_tools_list, tools, ToolDef, CREATE_TASK, INSTRUCTIONS, MAX_TITLE_LEN, SET_TITLE,
    TASK_STATUS,
};

/// The tools of the conversation `conversation_id`.
pub(crate) struct TaskTools<'a> {
    pub ctx: &'a Context,
    pub conversation_id: &'a str,
}

fn parse<T: DeserializeOwned>(arguments: Value) -> Checked<T> {
    let arguments = match arguments {
        Value::Null => json!({}),
        other => other,
    };
    serde_json::from_value(arguments).map_err(|e| format!("invalid arguments: {e}"))
}

fn as_json<T: serde::Serialize>(value: &T) -> Checked<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

fn unknown_tool(name: &str, list: &[ToolDef]) -> String {
    let names: Vec<&str> = list.iter().map(|t| t.name).collect();
    format!("unknown tool `{name}`: use one of {}", names.join(", "))
}

impl ToolRunner for TaskTools<'_> {
    fn conversation(&self) -> &str {
        self.conversation_id
    }

    fn tools(&self) -> Vec<ToolDef> {
        tools()
    }

    fn instructions(&self) -> &'static str {
        INSTRUCTIONS
    }

    async fn call(&self, name: &str, arguments: Value) -> Checked<Value> {
        match name {
            CREATE_TASK => as_json(&self.create_task(parse(arguments)?).await?),
            SET_TITLE => as_json(&self.set_title(parse(arguments)?).await?),
            TASK_STATUS => {
                status::task_status(self.ctx, self.conversation_id, parse(arguments)?).await
            }
            LIST_AGENT_TYPES => library::list(self.ctx, parse(arguments)?).await,
            READ_AGENT_TYPE => library::read(self.ctx, parse(arguments)?).await,
            SAVE_AGENT_TYPE => {
                library::save(self.ctx, self.conversation_id, parse(arguments)?).await
            }
            READ_LOGS => log_tools::read(self.ctx, parse(arguments)?).await,
            LOG_SUMMARY => log_tools::summary(self.ctx, parse(arguments)?).await,
            other => Err(unknown_tool(other, &tools())),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TitleInput {
    title: String,
}

impl TaskTools<'_> {
    async fn create_task(&self, raw: CreateTaskInput) -> Checked<Task> {
        let input = checks::input(raw)?;
        let store = self.ctx.store().map_err(|e| e.to_string())?;
        if let Some(taken) = store
            .find_task(&input.name)
            .await
            .map_err(|e| e.to_string())?
        {
            return Err(format!(
                "a task named `{}` already exists: pick another name",
                taken.name
            ));
        }
        let conversation = store
            .conversation(self.conversation_id)
            .await
            .map_err(|e| e.to_string())?;
        let repo = checks::repo(self.ctx, &conversation, input.repo.as_deref()).await?;
        let base_branch = checks::base_branch(&repo, input.base_branch.as_deref()).await?;
        let pr_base = checks::pr_base(&repo, input.pr_base.as_deref()).await?;
        let agent = library::resolve(self.ctx, input.agent.as_deref()).await?;
        checks::name_is_free(self.ctx, &repo, &input.name).await?;
        let remedy = "create those tasks first and pass them in `after`";
        checks::requirements(self.ctx, &agent, &input.after, remedy).await?;
        let new = NewTask {
            name: input.name,
            title: input.title,
            prompt: input.prompt,
            repo: repo.name,
            base_branch,
            after: input.after,
            attachments: input.attachments,
            agent,
            pr_base,
            done: Done {
                done_when: input.done_when,
                check: input.check,
                max_retries: input.max_retries.unwrap_or(DEFAULT_MAX_RETRIES),
                verify: None,
            },
        };
        let task = store
            .create_task(self.conversation_id, new)
            .await
            .map_err(|e| e.to_string())?;
        if let Err(e) = store
            .note_task_created(self.conversation_id, &task.name)
            .await
        {
            tracing::warn!(
                target: "townhall::mcp", event = "mcp.task_note_failed",
                task = %task.name, conversation = self.conversation_id,
                error = %redact(&e.to_string()),
                "task created but not shown in its conversation"
            );
        }
        Ok(task)
    }

    async fn set_title(&self, input: TitleInput) -> Checked<Conversation> {
        let title = checks::one_line(&input.title);
        if title.is_empty() {
            return Err("`title` must not be blank".into());
        }
        let len = u64::try_from(title.chars().count()).unwrap_or(u64::MAX);
        if len > MAX_TITLE_LEN {
            return Err(format!("`title` is longer than {MAX_TITLE_LEN} characters"));
        }
        let store = self.ctx.store().map_err(|e| e.to_string())?;
        store
            .update_conversation(self.conversation_id, move |c| {
                Ok(Conversation { title, ..c })
            })
            .await
            .map_err(|e| e.to_string())
    }
}

/// The tools of the agent of the task `task`.
pub(crate) struct HandBackTools<'a> {
    pub ctx: &'a Context,
    pub task: &'a str,
}

impl ToolRunner for HandBackTools<'_> {
    /// A task agent has no conversation of its own: its calls are logged under the task id.
    fn conversation(&self) -> &str {
        self.task
    }

    fn tools(&self) -> Vec<ToolDef> {
        task_tools_list()
    }

    fn instructions(&self) -> &'static str {
        hand_back::INSTRUCTIONS
    }

    async fn call(&self, name: &str, arguments: Value) -> Checked<Value> {
        match name {
            HAND_BACK => hand_back::hand_back(self.ctx, self.task, parse(arguments)?).await,
            ADD_ARTIFACT => artifacts::add(self.ctx, self.task, parse(arguments)?).await,
            LIST_ARTIFACTS => artifacts::list(self.ctx, self.task, parse(arguments)?).await,
            OPEN_PR => open_pr::open_pr(self.ctx, self.task, parse(arguments)?).await,
            other => Err(unknown_tool(other, &task_tools_list())),
        }
    }
}
