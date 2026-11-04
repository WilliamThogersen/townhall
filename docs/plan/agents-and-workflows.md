# Plan: agents and workflows

Status: planned, not built. Builds on [`storage.md`](storage.md) and on phase 2 of [`build-brief.md`](../style-guide/build-brief.md): 2.2 conversations, 2.3 tasks, 2.5 agent states and 2.7 steps.

## Goal

You define your own agents and workflows.

- An **agent** says how one worker runs: adapter, model, permission mode, tools and prompt.
- A **workflow** says which agents run, in what order, and what each one gets as input.

Nothing about the pipeline is hard coded, so townhall fits whatever the user needs. The conversation can build them for you.

Townhall has no config files: definitions and runs are rows in its SQLite database, see [`storage.md`](storage.md). TOML is only the text format you, the editor and the conversation write definitions in; the database stores that text.

## Tables

This plan adds one migration with these tables to the database from [`storage.md`](storage.md).

```sql
create table agents (
  name        text primary key,
  description text not null default '',
  source      text not null,
  updated_at  text not null
);

create table workflows (
  name        text primary key,
  description text not null default '',
  source      text not null,
  updated_at  text not null
);

create table workflow_agents (
  workflow text not null references workflows(name) on delete cascade,
  agent    text not null references agents(name) on delete restrict,
  primary key (workflow, agent)
);

create table runs (
  id              text primary key,
  conversation_id text references conversations(id),
  workflow        text not null,
  snapshot        text not null,
  repo            text not null,
  input           text not null,
  status          text not null,
  started_at      text not null,
  ended_at        text
);

create table run_steps (
  run_id     text not null references runs(id) on delete cascade,
  step_id    text not null,
  task_id    text references tasks(id),
  agent_name text not null,
  status     text not null,
  primary key (run_id, step_id)
);
```

- `source` is the TOML exactly as written, comments included, so the editor shows what you wrote. It is parsed and validated on save and parsed again on load.
- `workflow_agents` is rewritten on every workflow save. Its `on delete restrict` makes deleting a used agent fail, and the error names the workflows that use it.
- `snapshot` holds the workflow and its agents as they were when the run started, so editing a definition mid-run changes nothing.

## Why TOML as the text format

- The daemon is Rust. The `toml` crate deserializes straight into the serde types the API already uses.
- No indentation rules and no implicit types. YAML turns `no` and `off` into booleans.
- Comments are allowed, so people can annotate their own definitions.
- Export and import use the same text, so a definition can be shared as a `.toml` file without townhall ever reading config files.

## Agent definition

```toml
name = "coder"
description = "implements a spec, tests first"
adapter = "claude"
model = "sonnet"
permission_mode = "acceptEdits"
tools = ["Read", "Edit", "Write", "Bash"]
prompt = """
You implement the spec you are given.
Write tests first. Commit when the tests pass.
"""
```

| field | required | maps to |
|-------|----------|---------|
| `name` | yes | primary key, shown in the ui |
| `description` | no | shown in the library, and to townhall's session when it picks agents |
| `adapter` | yes | `claude` or `shell`, the existing `AgentAdapter` kinds |
| `model` | no | `claude --model` |
| `permission_mode` | no | `claude --permission-mode`, default from settings (accept edits) |
| `tools` | no | `claude --allowedTools` |
| `prompt` | no | `claude --append-system-prompt` |
| `command` | shell only | the `shell` adapter's `command` |

`permission_mode` accepts `default`, `acceptEdits` and `plan`. A definition with `bypassPermissions` is rejected: the build brief allows `skip all` only through the settings endpoint.

Core change: `LaunchOptions` gains `model`, `permission_mode`, `allowed_tools` and `system_prompt`. `ClaudeCodeAdapter` turns them into flags, each quoted with `shell_quote`. 2.5 adds `--permission-mode` too, so the two units share that code.

## Workflow definition

```toml
name = "ship"
description = "plan, build, then test and review in parallel"

[[step]]
id = "plan"
agent = "planner"
prompt = "Write an implementation spec for: {{input}}"

[[step]]
id = "build"
agent = "coder"
after = ["plan"]
prompt = "Implement this spec:\n\n{{steps.plan.result}}"

[[step]]
id = "test"
agent = "tester"
after = ["build"]
from = "build"
prompt = "Test the changes on this branch against the spec:\n\n{{steps.plan.result}}"

[[step]]
id = "review"
agent = "reviewer"
after = ["build"]
from = "build"
prompt = "Review the diff on this branch. Answer go or no-go with reasons."
```

| field | required | meaning |
|-------|----------|---------|
| `id` | yes | unique in the workflow, passes `validate_name` |
| `agent` | yes | the name of a saved agent |
| `after` | no | step ids that must finish first. Empty means start at once |
| `from` | no | step id whose branch this step's worktree starts from. Default: the repo's base branch |
| `prompt` | no | the task prompt, with the variables below |

### Order

- A step starts when every step in `after` has finished and a slot is free (2.3's queue, 2.5's "agents at once").
- Steps with the same `after` run in parallel. In the example, `test` and `review` run at the same time.
- A step with several ids in `after` waits for all of them.
- `from` must name a step that this step waits for, directly or through other steps, so the branch exists when it starts.
- "Finished" means what 2.3 decides for tasks. Workflows add no second definition.

### Variables

| variable | value |
|----------|-------|
| `{{input}}` | the request the run was started with |
| `{{repo}}` | the repo name |
| `{{steps.<id>.result}}` | the last assistant message of that step's Claude session, read with 2.7's transcript reader |
| `{{steps.<id>.branch}}` | that step's branch |

An unknown variable fails validation. It is never left in the prompt as literal text.

### Names at run time

Each step becomes a running agent named `<step id>-<short run id>`, for example `build-3f2a`, on branch `townhall/build-3f2a`. Two runs of the same workflow never collide.

## Validation

Every save, import and load runs the same checks. They live in `townhall-core`, so the daemon, the conversation tools and the tests share one implementation.

1. The TOML parses. Unknown fields are errors (`#[serde(deny_unknown_fields)]`).
2. Names and ids pass `validate_name` (letters, digits, `-`, `_`).
3. Every `agent` is saved. Every `after` and `from` id is a step in the workflow.
4. `after` has no cycles.
5. `from` names a step this step waits for.
6. Every `{{...}}` variable is known, and `steps.<id>` names a step that runs before this one.
7. `permission_mode` is not `bypassPermissions`.

Errors name the definition, the step and the field:

```
error: workflow "ship" step "test": from "deploy" is not a step
```

The conversation's session gets the same text back, so it can fix the definition and save again.

## A run

1. Start it from a conversation ("run ship on: add dark mode to settings") or with `POST /api/workflows/:name/runs` and `{ repo, input }`.
2. The daemon validates the workflow again, then creates one 2.3 task per step. `after` maps to task ids, `from` sets the task's `baseBranch`.
3. One transaction writes the `runs` row with its snapshot and one `run_steps` row per step.
4. A step that ends in 2.5's error state pauses the run. The conversation shows a needs-you block with two answers: retry the step, or stop the run.
5. The run ends when every step has finished. Merging stays with the normal task flow.

## The conversation builds them

Townhall's session gets these tools, through the same mechanism as 2.3's task tool:

| tool | does |
|------|------|
| `list_agents`, `list_workflows` | names and descriptions, so the session reuses before it creates |
| `read_definition` | one definition's TOML |
| `save_agent`, `save_workflow` | takes TOML text, validates it, saves it to the database, returns the errors or the saved name |
| `run_workflow` | `name`, `repo`, `input`: starts a run |

How it goes:

1. You describe what you need: "a planner on opus, then a coder, then a tester that runs cargo test".
2. The session lists what exists, writes the TOML and calls save. On a validation error it fixes the TOML and tries again.
3. Before saving, the conversation shows the TOML as a block with save and edit. With "work without asking" on (2.5 setting) it saves at once. Replacing an existing definition always asks.
4. Saved definitions show in the library right away.

## API

| route | does |
|-------|------|
| `GET /api/library/agents` | list agents |
| `GET /api/library/agents/:name` | read one, as TOML and as parsed JSON |
| `PUT /api/library/agents/:name` | save, TOML body, validated |
| `DELETE /api/library/agents/:name` | delete. Fails and names the workflows that still use it |
| `GET`, `PUT`, `DELETE` `/api/library/workflows/...` | the same for workflows |
| `POST /api/library/import` | one `.toml` upload, agent or workflow by its shape, same validation |
| `POST /api/workflows/:name/runs` | start a run with `{ repo, input }` |
| `GET /api/runs/:id` | run state |

`library` keeps saved definitions apart from `/api/agents`, which lists running agents. Responses use the existing envelope in `envelope.rs`.

## UI

- `/library`: two lists, agents and workflows. A row opens a TOML editor that validates as you type and shows errors under the line they belong to.
- Each row has export (downloads the `.toml`) and the list has import.
- Workflow rows show the order as text: `plan → build → test, review`.
- Conversation: a definition block (TOML preview with save and edit) and a run block (steps with status glyphs).
- Control center: a run's steps show as rows grouped under the run.

These screens are not in `preview.html` yet. Design them there first, then build.

## Units

Add these to the build brief when the work is picked up. Unit 2.0 is in [`storage.md`](storage.md).

| unit | scope | needs |
|------|-------|-------|
| 2.8a definitions | serde types, TOML parsing, validation, new `LaunchOptions` flags, all in `townhall-core` | nothing, can start now |
| 2.8b library api and tools | agent and workflow tables, `/api/library/*`, import and export, the list, read and save tools | 2.0, 2.2, 2.8a |
| 2.8c runs | run tables, workflow to tasks, variables, pause on error, `run_workflow` | 2.3, 2.5, 2.7, 2.8b |
| 3.7 library and run ui | `/library`, definition and run blocks, run rows in the control center | 2.8b, 2.8c, preview section |

## Tests

- 2.8a: one passing and one failing definition per validation rule. Adapter flags with quoting. Every valid workflow has a start order.
- 2.8b: API tests in the style of `crates/townhall-daemon/tests/api.rs`. Deleting a used agent fails and names the workflow. Export then import gives the same `source`.
- 2.8c: a workflow of `shell` steps runs in order, fans out, waits on several steps and pauses on a failing step. Uses the existing tmux test helpers.
- 3.7: Vitest for the editor and the blocks. 4.4's Playwright run adds "build a workflow in the conversation and run it".

## Ask the user before

- adding the `toml` crate (small and serde based, still a new dependency)
- any way to set `bypassPermissions` from a definition

## Open questions

1. Should townhall offer to import `.toml` definitions committed in a repo, so a team shares them? Import only, into the database; townhall still never reads them at startup or writes into the repo.
2. Loops: a reviewer that sends work back to the coder until it says go. Needs a retry field with a max count. Not in the first version.
3. Is passing text (`{{steps.<id>.result}}`) enough, or should steps also hand off files?
4. Should Claude Code subagents (`.claude/agents/*.md`) be importable as agent definitions?
5. Should a workflow be able to end with a merge step, or does merging always stay with you?
