//! `create_task`, `set_title`, `task_status` (`status.rs`), the agent type tools
//! (`library.rs`) and the log tools (`logs.rs`) for one conversation's session.
//! None takes a permission mode: a task's agent gets the saved setting when the scheduler
//! starts it, narrowed by its agent type.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::context::Context;
use crate::model::{
    AgentEvent, Conversation, CreateTaskInput, Done, EntryBody, NewTask, Task, TaskState,
    DEFAULT_MAX_RETRIES,
};
use crate::plugins::hooks;
use crate::redact::redact;
use crate::repos::names;
use crate::scheduler::pipeline;
use crate::store::NewRun;

use super::adviser::{self, Caller, ASK_ADVISER};
use super::artifacts::{self, ADD_ARTIFACT, LIST_ARTIFACTS};
use super::checks::{self, Checked};
use super::hand_back::{self, HAND_BACK};
use super::graph::{self, AGENT_STATS, HISTORY_GRAPH};
use super::history::{self, READ_CONVERSATION, SEARCH_HISTORY};
use super::insights::{self as insight_tools, CONVERSATION_INSIGHTS};
use super::library::{
    self, LIST_AGENT_TYPES, LIST_PIPELINES, READ_AGENT_TYPE, READ_PIPELINE, SAVE_AGENT_TYPE,
    SAVE_PIPELINE,
};
use super::logs::{self as log_tools, LOG_SUMMARY, READ_LOGS};
use super::open_pr::{self, OPEN_PR};
use super::rpc::ToolRunner;
use super::{
    status, task_tools_list, tools, ToolDef, CREATE_TASK, INSTRUCTIONS, MAX_TITLE_LEN, SET_TITLE,
    TASK_STATUS,
};

const MAX_FAILED_TEXT: usize = 300;

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
            LIST_PIPELINES => library::list_pipelines(self.ctx, parse(arguments)?).await,
            READ_PIPELINE => library::read_pipeline(self.ctx, parse(arguments)?).await,
            SAVE_PIPELINE => {
                library::save_pipeline(self.ctx, self.conversation_id, parse(arguments)?).await
            }
            READ_LOGS => log_tools::read(self.ctx, parse(arguments)?).await,
            LOG_SUMMARY => log_tools::summary(self.ctx, parse(arguments)?).await,
            CONVERSATION_INSIGHTS => insight_tools::insights(self.ctx, parse(arguments)?).await,
            SEARCH_HISTORY => history::search(self.ctx, parse(arguments)?).await,
            READ_CONVERSATION => history::read(self.ctx, parse(arguments)?).await,
            HISTORY_GRAPH => graph::graph(self.ctx, parse(arguments)?).await,
            AGENT_STATS => graph::stats(self.ctx, parse(arguments)?).await,
            ASK_ADVISER => {
                adviser::ask(
                    self.ctx,
                    Caller::Conversation(self.conversation_id),
                    parse(arguments)?,
                )
                .await
            }
            other => Err(unknown_tool(other, &tools())),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TitleInput {
    title: String,
}

pub async fn create_task_as_plugin(
    ctx: &Context,
    conversation_id: &str,
    plugin: &str,
    raw: CreateTaskInput,
) -> crate::Result<Task> {
    let store = ctx.store()?;
    store.conversation(conversation_id).await?;
    let tools = TaskTools {
        ctx,
        conversation_id,
    };
    let task = tools
        .create_task(raw)
        .await
        .map_err(crate::Error::InvalidInput)?;
    let body = EntryBody::Plugin {
        plugin: plugin.to_string(),
        text: format!("created by plugin {plugin}: task `{}`", task.name),
    };
    store.append_entry(conversation_id, body).await?;
    Ok(task)
}

pub(crate) async fn create_step_task(
    ctx: &Context,
    conversation_id: &str,
    raw: CreateTaskInput,
) -> Result<Task, String> {
    let tools = TaskTools {
        ctx,
        conversation_id,
    };
    tools.make_task(raw).await
}

impl TaskTools<'_> {
    async fn create_task(&self, raw: CreateTaskInput) -> Checked<Task> {
        if raw.pipeline.is_some() {
            return self.start_run(raw).await;
        }
        let task = self.make_task(raw).await?;
        let store = self.ctx.store().map_err(|e| e.to_string())?;
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

    async fn start_run(&self, raw: CreateTaskInput) -> Checked<Task> {
        let input = checks::input(raw)?;
        let wanted = input.pipeline.clone().unwrap_or_default();
        let mut refused = vec![];
        for (set, field) in [
            (!input.after.is_empty(), "after"),
            (input.agent.is_some(), "agent"),
            (input.pr_base.is_some(), "prBase"),
            (input.check.is_some(), "check"),
            (input.done_when.is_some(), "doneWhen"),
            (input.max_retries.is_some(), "maxRetries"),
            (!input.attachments.is_empty(), "attachments"),
        ] {
            if set {
                refused.push(format!("`{field}`"));
            }
        }
        if !refused.is_empty() {
            return Err(format!(
                "{} cannot be set together with `pipeline`: the pipeline decides them",
                refused.join(", ")
            ));
        }
        let store = self.ctx.store().map_err(|e| e.to_string())?;
        let found = store
            .workflow(&wanted)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!("no pipeline named `{wanted}`: list_pipelines shows the saved ones")
            })?;
        let conversation = store
            .conversation(self.conversation_id)
            .await
            .map_err(|e| e.to_string())?;
        let repo = checks::repo(self.ctx, &conversation, input.repo.as_deref()).await?;
        if !found.def.repos.is_empty()
            && !found.def.repos.iter().any(|r| names::same(r, &repo.name))
        {
            return Err(format!(
                "pipeline `{}` is not for repo `{}`: it is limited to {}",
                found.def.name,
                repo.name,
                found.def.repos.join(", ")
            ));
        }
        let base_branch = checks::base_branch(&repo, input.base_branch.as_deref()).await?;
        for step in &found.def.steps {
            let name = pipeline::task_name(&input.name, &step.id, 1);
            if store
                .find_task(&name)
                .await
                .map_err(|e| e.to_string())?
                .is_some()
            {
                return Err(format!(
                    "a task named `{name}` already exists: pick another run name"
                ));
            }
        }
        let new = NewRun {
            id: input.name.clone(),
            workflow: found.def.name.clone(),
            snapshot: found.source.clone(),
            conversation_id: self.conversation_id.to_string(),
            repo: repo.name,
            base_branch,
            input: input.prompt,
        };
        let run = store.create_run(new).await.map_err(|e| e.to_string())?;
        pipeline::advance(self.ctx, &run)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .ok_or_else(|| format!("run `{}` started no task: its thread says why", input.name))
    }

    async fn make_task(&self, raw: CreateTaskInput) -> Checked<Task> {
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
        self.screen(task).await
    }

    async fn screen(&self, task: Task) -> Checked<Task> {
        let fields = json!({"task": task, "conversationId": self.conversation_id});
        let asked = hooks::ask(self.ctx, "task.created", fields).await;
        if asked.blocked.is_none() && asked.prompt.is_empty() {
            return Ok(task);
        }
        let store = self.ctx.store().map_err(|e| e.to_string())?;
        let moved = "the task moved on";
        let change = move |t: Task| match (t.state == TaskState::Queued, &asked.blocked) {
            (false, _) => Err(crate::Error::Conflict(moved.into())),
            (true, Some(b)) => Ok(Task {
                state: TaskState::Error,
                blocked: Some(format!("plugin {}: {}", b.plugin, b.reason)),
                ..t
            }),
            (true, None) => Ok(Task {
                prompt: asked
                    .prompt
                    .iter()
                    .fold(t.prompt.clone(), |text, (id, more)| {
                        format!(
                            "{}\n\n## From plugin {id}\n\n{}",
                            text.trim_end(),
                            more.trim_end()
                        )
                    }),
                ..t
            }),
        };
        let saved = match store.update_task(&task.name, change).await {
            Ok(saved) => saved,
            Err(crate::Error::Conflict(m)) if m == moved => return Ok(task),
            Err(e) => return Err(e.to_string()),
        };
        if let Some(why) = &saved.blocked {
            let body = EntryBody::Agent {
                agent: saved.name.clone(),
                event: AgentEvent::Failed,
                text: format!("ended by a plugin when it was created: {why}")
                    .chars()
                    .take(MAX_FAILED_TEXT)
                    .collect(),
            };
            store
                .append_entry(self.conversation_id, body)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(saved)
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
            SEARCH_HISTORY => history::search(self.ctx, parse(arguments)?).await,
            READ_CONVERSATION => history::read(self.ctx, parse(arguments)?).await,
            HISTORY_GRAPH => graph::graph(self.ctx, parse(arguments)?).await,
            AGENT_STATS => graph::stats(self.ctx, parse(arguments)?).await,
            ASK_ADVISER => adviser::ask(self.ctx, Caller::Task(self.task), parse(arguments)?).await,
            other => Err(unknown_tool(other, &task_tools_list())),
        }
    }
}
