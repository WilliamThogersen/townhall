# Daemon API (phase 2 contract)

The contract every phase 2 unit builds on. Rust types live in `crates/townhall-core/src/model/` (serde camelCase) and are mirrored field for field in `web/src/api/types.ts`. Read this file and the code; you should not need to ask.

## Conventions

- Base: `http://127.0.0.1:7777/api` (desktop) or same-origin `/api` (browser, Vite proxy).
- Every HTTP response is the envelope `{ "success": bool, "data": T | null, "error": string | null }` (`crates/townhall-daemon/src/envelope.rs`). `data` is shown below as the response.
- JSON field names are camelCase. A Rust `Option` is always present on the wire, as `null` when empty. Timestamps are RFC 3339 UTC strings with milliseconds (`2026-11-04T10:42:05.000Z`, `townhall_core::clock`), so they sort as text.
- Bodies go through `townhall_daemon::extract::JsonBody<T>`: a body that is not JSON, has the wrong content type or the wrong fields is `400` in the envelope, with the error `expected content-type application/json` or `invalid json body: <what serde said>` (for example ``invalid json body: missing field `path` at line 1 column 2``). Every phase 2 request body type refuses unknown fields (`deny_unknown_fields`); phase 1's agent bodies (`POST /api/agents`, `/send`) still ignore them.
- Error codes: `400` invalid input or name, `404` not found, `409` conflict (taken name, ambiguous agent, repo in use), `413` attachment over 20 MB, `501 not implemented yet` (a route whose unit has not landed), `503` the database is held by another townhall, `500` anything else (logged).
- Copy in `error` strings is lowercase and has no `error:` prefix; the UI adds it.
- CORS allows only the desktop and Vite origins (`cors::ALLOWED_ORIGINS`), methods `GET POST PUT PATCH DELETE`. CORS does not cover websockets or multipart posts: `/api/events` and the upload route must call `cors::origin_allowed(&headers)` and answer `403` when it is false.

## Routes

`?repo=<name>` is accepted by every `/api/agents/{name}...` route. Without it the name is looked up across every registered repo (ignoring case, as macOS folders do; an agent whose name differs only by case is `404`, never acted on); two repos with that agent is `409` (`agent `x` exists in several repos (a, b): pass ?repo=<name>`). Repo names match ignoring case.

| Method | Path | Body | Response | Errors | Unit |
|--------|------|------|----------|--------|------|
| GET | `/info` | | `{ home, repo, repoKey, userName }` (`userName` from `git config user.name`, or `null`) | | 2.1 |
| GET | `/repos` | | `Repo[]` in the order added | 503 | 2.1 |
| POST | `/repos` | `AddRepo { path, name?, baseBranch? }` | `201 Repo` | 400 not absolute, missing, not a folder, `not the top folder of a git repo` (a subfolder, a plain folder, a folder under `~/.townhall/worktrees/`, or a linked worktree on a `townhall/*` branch), blank or bad name, base branch not found; 409 path registered, name taken ignoring case | 2.1 |
| DELETE | `/repos/{name}` | | the removed `Repo`. Only unregisters: folder, worktrees and branches stay | 404; 409 it has live agents (tmux sessions), or it is the daemon's startup repo | 2.1 |
| GET | `/agents` | | `AgentInfo[]` across every repo | 503 | 2.1 |
| POST | `/agents` | `{ name, adapter: "claude"\|"shell", base?, prompt?, command?, repo? }` (`repo` defaults to the startup repo, `base` to `HEAD`) | `201 AgentInfo` | 400 bad name, ``base branch `x` not found``; 404 repo; 409 the name exists in any repo, ignoring case | phase 1, 2.1 |
| DELETE | `/agents/{name}` | | the name | 400, 404, 409 | phase 1, 2.1 |
| POST | `/agents/{name}/send` | `{ text }` | the name. When the agent is a task's, adds an `agent` `sent` entry to its conversation | 400, 404, 409 | phase 1, 2.1 |
| GET | `/agents/{name}/output` | | the pane text | 400, 404, 409 | phase 1, 2.1 |
| GET (ws) | `/agents/{name}/stream` | | text frames with the pane, `error: ...` then close when it ends | 400, 404, 409 before the upgrade | phase 1, 2.1 |
| GET | `/agents/{name}/steps` | | `Step[]` | 404 | 2.7 |
| GET | `/stats?repo=` | | `{ days: ShippedDay[] }`: last 730 days, days with activity only, summed across repos unless `repo` | 404 repo | 2.7 |
| GET | `/settings` | | `Settings` | 503 | 2.1 (was 2.0) |
| PUT | `/settings/{key}` | `{ value }` | `Settings` after the change | 400 unknown key or invalid value | 2.1 (was 2.0) |
| GET | `/conversations` | | `Conversation[]`, newest activity first | | 2.2 |
| POST | `/conversations` | `NewMessage { text, attachments? }` | `201 ConversationDetail`: created, your message appended, the first turn started | 400 | 2.2 |
| GET | `/conversations/{id}` | | `ConversationDetail { conversation, entries, tasks, attachments }` | 404 | 2.2 |
| PATCH | `/conversations/{id}` | `ConversationPatch { linkedRepo?, title? }` (`linkedRepo: null` is auto) | `Conversation` | 400, 404 unknown repo | 2.2 |
| POST | `/conversations/{id}/messages` | `NewMessage` | `ThreadEntry` (yours). Clears `ended`, starts a turn | 400, 404, 409 a turn is running | 2.2 |
| POST | `/conversations/{id}/stop` | | `Conversation`: interrupts the running turn | 404 | 2.2 |
| POST | `/conversations/{id}/end` | | `Conversation` with `ended: true`; tasks keep running | 404 | 2.2 |
| GET (ws) | `/events` | | `ServerEvent` JSON text frames, see below | 403 origin | 2.2 |
| POST | `/conversations/{id}/mcp` | JSON-RPC 2.0, see MCP | JSON-RPC answer, or `202` with no body for notifications | 404 | 2.3 |
| GET | `/tasks?conversation=&repo=&state=` | | `Task[]`, oldest first | | 2.3 |
| GET | `/tasks/{name}` | | `Task` | 404 | 2.3 |
| POST | `/conversations/{id}/attachments` | multipart, one field `file` per request | `201 Attachment` | 400, 403 origin, 404, 413 | 2.6 |
| GET | `/conversations/{id}/attachments` | | `Attachment[]` | 404 | 2.6 |
| GET | `/conversations/{id}/attachments/{attachment}` | | the bytes with their `content-type` (not the envelope) | 404 | 2.6 |
| GET | `/library/agents` | | agent definitions | | 2.8b |
| GET, PUT, DELETE | `/library/agents/{name}` | PUT: TOML text | | | 2.8b |
| GET | `/library/workflows` | | workflow definitions | | 2.8b |
| GET, PUT, DELETE | `/library/workflows/{name}` | PUT: TOML text | | | 2.8b |
| POST | `/library/import` | one `.toml` upload | | | 2.8b |
| POST | `/workflows/{name}/runs` | `{ repo, input }` | the run | | 2.8c |
| GET | `/runs/{id}` | | run state | | 2.8c |

Every route above is registered in `crates/townhall-daemon/src/routes/mod.rs` now. A unit replaces the `501` handlers in its own file and keeps the handler names; it adds extractors (`State`, `Path`, `Query`, `JsonBody`) to the signatures as needed. The upload route already carries `DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)` (20 MB plus framing). The definitions routes take TOML, not JSON; 2.8b documents their bodies in this file when it lands.

## Types

All in `townhall_core::model`, mirrored in `web/src/api/types.ts`.

- `Repo { name, path, baseBranch }`. `name`: `[A-Za-z0-9._-]`, starts with a letter or digit, at most 64, unique ignoring case, the folder name by default. `path`: canonical, the top folder of a git work tree, never one of townhall's own worktrees. `baseBranch`: origin's HEAD branch (the local branch when it exists, else `origin/x`), else the current branch, else `HEAD`.
- `AgentInfo { name, repo, kind, branch, worktree, alive, state }`: phase 1's `Agent` plus `repo` and `state`. `AgentState`: `{ kind: "working" }`, `{ kind: "waiting", question }`, `{ kind: "error", exitCode: number | null }`, `{ kind: "ended" }`.
- `Settings { permissionMode, workWithoutAsking, agentsAtOnce, repoUnclear, sessionPanelOpen, notifyWhen }`, the same names, values and defaults as `web/src/settings/settings.ts` (a type test checks it).
- `Conversation { id, sessionId, sessionStarted, title, status: "thinking" | "yourTurn", tasks: string[] (task names, oldest first), linkedRepo: string | null, ended, context: { used, limit } | null, createdAt, updatedAt }`. `title` starts as `new conversation`. `sessionStarted` is false until the first turn went through (that turn uses `--session-id`, later ones `--resume`).
- `ThreadEntry { id, at } & EntryBody`. `id` counts 1, 2, 3 ... per conversation. `EntryBody` is tagged by `kind`:

| kind | fields | shown as |
|------|--------|----------|
| `you` | `text, attachments: string[]` (attachment ids of this conversation) | your message |
| `townhall` | `text` (Markdown) | townhall's reply |
| `repo` | `repo, baseBranch, rule, reason` | `~ repo shop · develop, it has src/search/query.ts` |
| `repoAsk` | `question, candidates: string[], offerAll` | needs-you block from townhall: one button per candidate, plus `both` (2 candidates) or `all` when `offerAll`. A button sends its text as a normal message |
| `tasksCreated` | `tasks: string[]` | task block |
| `needsYou` | `agent, question, choices: string[]` | `◆ <agent> needs you`, the question verbatim, one `send <choice>` per choice (through `/agents/{name}/send`) |
| `agent` | `agent, event: "sent" \| "passed" \| "failed" \| "ended", text` | `→` `✓` `×` `○` lines |
| `error` | `message` | townhall's own failure |

- `RepoRule`: `named`, `code`, `only` (2.4's `repo_infer::Rule`, same names), `picked` (an answer to a `repoAsk`), `linked` (the row menu).
- `Task { name, title, prompt, repo, baseBranch, after: string[], state, conversationId, attachments: string[], createdAt, pr: { number, url: string | null, checks: "pending" | "passed" | "failed" | "none" } | null }`. `name` is the agent name and the branch `townhall/<name>` (`Task::branch()`), unique across every conversation and repo ignoring case. `TaskState`: `queued`, `working`, `needsYou`, `inCi`, `inReview`, `merged`, `ended`, `error`.
- `Attachment { id, conversationId, name, mime, size, kind: "image" | "file", createdAt }`. `kind` is `image` for `image/*`. At most 20 MB (`MAX_ATTACHMENT_BYTES`).
- `Step { id, tool, summary, startedAt, endedAt: string | null, result: "running" | "ok" | "error" }` and `ShippedDay { date: "YYYY-MM-DD", fixes, features, lines, agents }`: the Rust types belong to 2.7 (`steps/`, `stats/`); `types.ts` already mirrors them.
- `CreateTaskInput`, `NewTask`, `NewMessage`, `ConversationPatch`, `AddRepo`, `PutSetting`: request bodies, see the routes and MCP.

## Settings keys

One row per key in the `settings` table, the value as JSON. `PUT /api/settings/{key}` with `{ "value": ... }` is the only way to change one, and so the only way to choose `skip all`. The browser-settings import (a separate web unit) sends each key the settings screen stored in `localStorage` (`th-settings`) once, then clears it.

| key | values | default |
|-----|--------|---------|
| `permissionMode` | `default`, `acceptEdits`, `plan`, `auto`, `bypassPermissions` (`skip all`) | `acceptEdits` |
| `workWithoutAsking` | boolean | `false` |
| `agentsAtOnce` | integer 1 to 50 | `6` |
| `repoUnclear` | `ask`, `guess` | `ask` |
| `sessionPanelOpen` | boolean | `true` |
| `notifyWhen` | `needsYou`, `failed`, `everything` | `needsYou` |

A missing or invalid stored value reads as its default. The store also keeps `desktop.repo` (the desktop app's chosen repo, a path); it is not a setting and not in the API.

## Permission mode

`LaunchOptions` (`townhall_core::agent`) has `prompt, command, permission_mode, session_id, model, allowed_tools, system_prompt`. `ClaudeCodeAdapter` (`agent/claude.rs`) builds this argv:

```
claude [--model=M] [--permission-mode=MODE | --dangerously-skip-permissions]
       [--append-system-prompt=P] [--session-id=ID] [--allowedTools=T1,T2] [-- PROMPT]
```

- Every option and its value is one argv element (`--name=value`), and the launch command shell-quotes every element, so no value can split or start a new flag. `--allowedTools` is one comma-joined element, so its position does not matter.
- The prompt is the only positional, after the end-of-options `--`. A prompt such as `--dangerously-skip-permissions`, `-p`, `--permission-mode=bypassPermissions` or `--` arrives as the prompt, never as a flag.
- `claude` still runs a subcommand when the first positional equals its name, even after `--` (`claude -- mcp` runs `claude mcp`). A prompt without any whitespace therefore gets a trailing newline; no subcommand name has one.
- `model`, `session_id` (a UUID) and each `allowed_tools` entry are refused (`InvalidInput`) when they start with `-` or hold control characters; a tool may not hold a comma. The system prompt may hold anything, since it sits inside its own `--append-system-prompt=` element. Blank values add no flag; no prompt means no `--`.
- `permission_mode`: `default`, `acceptEdits`, `plan` and `auto` become `--permission-mode=<mode>`; `bypassPermissions` becomes `--dangerously-skip-permissions`. `claude` 2.1.318 lists `acceptEdits, auto, bypassPermissions, manual, dontAsk, plan` and no longer lists `default`, but still accepts it, so `default` is passed as is for now.
- Checked against the installed `claude` 2.1.318 (empty environment, throwaway `HOME`, an invalid `--session-id` so it stops right after parsing): `-- --no-such-flag` reaches the session-id check while `--no-such-flag` alone is `unknown option`, and `--allowedTools=Read,Bash(git diff:*),Edit` and the `--name=value` forms parse.
- `ClaudeCodeAdapter::args(&options)` returns the same argv unquoted, for a caller that spawns `claude` without a shell (2.2's runner).

`permission_mode` always comes from `Context::settings()`, never from a request body or a tool call: `POST /api/agents` ignores any such field. The binary is `Context::claude_bin()` (`TOWNHALL_CLAUDE_BIN`, default `claude`); tests point it at a fake script.

## Websocket: `/api/events`

One socket per client. Each text frame is one `ServerEvent`, tagged by `type`:

| type | fields | published by |
|------|--------|--------------|
| `conversationUpdated` | `conversation` | every conversation write, each appended entry, each created task |
| `entryAppended` | `conversationId, entry` | `Store::append_entry` |
| `taskUpdated` | `task` | `Store::create_task`, `Store::update_task` |
| `agentUpdated` | `agent: AgentInfo` | the agent watcher (2.5) when a state changes |
| `resync` | | the socket itself, when its bus receiver lagged (`RecvError::Lagged`): refetch everything |

Store writes publish on `Context::bus()` (`townhall_core::events::EventBus`, a tokio broadcast of capacity 1024) while they hold the connection, so events arrive in write order. An append publishes `entryAppended`, then `conversationUpdated`; a created task publishes `taskUpdated`, then `conversationUpdated`. The socket handler (2.2) subscribes with `ctx.bus().subscribe()` and forwards each event as `serde_json::to_string(&event)`. A client puts entries in order by `id`.

## MCP: `POST /api/conversations/{id}/mcp`

JSON-RPC 2.0 over plain HTTP JSON (no SSE), like Hive's `src/main/plan/mcp.ts` and `rpc.ts`: `initialize` (answers `protocolVersion` (echo the client's, else `2025-06-18`), `capabilities: { tools: {} }`, `serverInfo: { name: "townhall", version }`, `instructions: mcp::INSTRUCTIONS`), `ping`, `tools/list`, `tools/call`, batches, and notifications (no `id`) answered with `202` and no body. A tool error comes back as a result with `isError: true` and a text content, so Claude can fix the call. Results are `{ content: [{ type: "text", text: <JSON> }] }`. The body goes through `JsonBody`, so a browser cannot post it without a CORS preflight.

The names, schemas and config are code in `townhall_core::mcp`: `SERVER_NAME` (`townhall`), `tools()`, `allowed_tools()` (`mcp__townhall__create_task`, `mcp__townhall__set_title`), `endpoint(base_url, id)` and `config_json(base_url, id)`, which 2.2 passes as `--mcp-config`:

```json
{"mcpServers":{"townhall":{"type":"http","url":"http://127.0.0.1:7777/api/conversations/<id>/mcp"}}}
```

`create_task` (input parsed as `model::CreateTaskInput`, result the `Task`):

```json
{
  "type": "object",
  "properties": {
    "name": { "type": "string", "pattern": "^[A-Za-z0-9_-]+$", "minLength": 1, "maxLength": 64,
              "description": "Short kebab-case id, unique across every repo. Also the agent's name and its branch townhall/<name>." },
    "title": { "type": "string", "minLength": 1, "maxLength": 80, "description": "Short imperative title for people." },
    "prompt": { "type": "string", "minLength": 1, "maxLength": 50000,
                "description": "The brief the agent works from alone: goal, context, acceptance criteria." },
    "repo": { "type": "string", "description": "Name of a registered repo. Default: the repo townhall inferred for this conversation." },
    "baseBranch": { "type": "string", "description": "Branch the worktree starts from. Default: the repo's base branch." },
    "after": { "type": "array", "items": { "type": "string" }, "maxItems": 20,
               "description": "Names of tasks that must finish before this one starts." },
    "attachments": { "type": "array", "items": { "type": "string" }, "maxItems": 20,
                     "description": "Attachment ids of this conversation the agent needs." }
  },
  "required": ["name", "title", "prompt"],
  "additionalProperties": false
}
```

`repo` defaults to the conversation's `linkedRepo`, else the repo of its latest `repo` entry; none of those is a tool error that lists the repos. `baseBranch` defaults to the repo's. 2.3 resolves both into a `NewTask`, calls `Store::create_task`, then appends one `tasksCreated` entry per turn. No tool takes a permission mode.

`set_title` (result the `Conversation`):

```json
{ "type": "object", "properties": { "title": { "type": "string", "minLength": 1, "maxLength": 80 } },
  "required": ["title"], "additionalProperties": false }
```

2.8b adds `list_agents`, `list_workflows`, `read_definition`, `save_agent` and `save_workflow` in `mcp/library.rs`; 2.8c adds `run_workflow` in `mcp/runs.rs`. `tools()` and `allowed_tools()` pick them up.

## Storage: `~/.townhall`

```
~/.townhall/                                  (or $TOWNHALL_HOME, mode 0700)
  townhall.db                                 every repo, setting, conversation, entry and task
  townhall.db-wal, townhall.db-shm            SQLite's WAL files
  townhall.lock                               held while a townhall owns the database (holds its pid)
  conversations/<id>/files/<attachment>/<name>   attachment bytes, the only other files
  worktrees/<repo-key>/<agent>/               one git worktree per agent, unchanged
  config.json.imported                        phase 1's config.json after its one-time import
```

- `townhall_core::store::Store` is the only code that opens the database: one connection behind a mutex, every call on `spawn_blocking`, `journal_mode = WAL`, `foreign_keys = ON`, `busy_timeout = 5000`.
- One owner: `townhall.lock` holds an exclusive OS lock (`File::try_lock`) for as long as the store is open. A second open, from another process or the same one, fails with `townhall.db is in use by another townhall (pid N)` after a short retry. The daemon opens it at start (`AppState::open`) and exits on that error. The desktop app opens it once at launch and hands it to every in-process daemon start (`AppState::with_store`); when another townhall holds it, the app uses that daemon.
- Live agent state (alive, branch, worktree) is never stored: it comes from git and tmux.
- Import: when `~/.townhall/config.json` exists at open, its `repo` is registered (when it is still a git repo) and saved as `desktop.repo` (existing rows are never overridden), and the file is renamed to `config.json.imported` (`-2`, `-3` ... when taken). Never deleted. Nothing else is read from it: a `settings` object is ignored, so a file can never choose `skip all`. A corrupt file is left alone with a warning. The desktop app imports the older `<app config dir>/settings.json` into `desktop.repo` once, when the store has none.

### Migrations

Embedded SQL in `crates/townhall-core/src/store/migrations/NNNN_<name>.sql`, applied in number order at open, each in its own transaction. Applied ones are recorded in `schema_migrations (id, name, applied_at)`, so a unit whose numbers are lower but that merges later is still applied; `PRAGMA user_version` holds the highest applied number. A database with a migration this build does not know, or a higher `user_version`, refuses to open with `townhall.db is from a newer townhall`. Never edit a merged migration: add a new file.

Each unit owns a number range and lists its migrations in its own file, so no two units pick the same number or edit a shared list. The ranges are code (`store::migrate::RANGES`) and a test checks every migration is inside its unit's range:

| range | unit | list in |
|-------|------|---------|
| 0001 to 0099 | 2.0 storage and 2.1 contract | `store/migrate.rs` `BASE` |
| 0100 to 0199 | 2.2 conversations | `store/unit_2_2.rs` |
| 0200 to 0299 | 2.3 tasks | `store/unit_2_3.rs` |
| 0500 to 0599 | 2.5 agent states | `store/unit_2_5.rs` |
| 0600 to 0699 | 2.6 attachments | `store/unit_2_6.rs` |
| 0700 to 0799 | 2.7 steps and stats | `store/unit_2_7.rs` |
| 0800 to 0849 | 2.8b library | `store/unit_2_8b.rs` |
| 0850 to 0899 | 2.8c runs | `store/unit_2_8c.rs` |
| 0300 to 0499, 0900 and up | free | ask the orchestrator |

To add one: write `store/migrations/0201_scheduler.sql`, then in your unit file `pub const MIGRATIONS: &[Migration] = &[Migration { id: 201, name: "0201_scheduler", sql: include_str!("migrations/0201_scheduler.sql") }];`.

### Tables

`0001_init` (fixed by the storage plan, `docs/plan/storage.md` on `docs/plans`):

```sql
create table settings (key text primary key, value text not null, updated_at text not null);
create table repos (name text primary key, path text not null unique, base_branch text not null, added_at text not null);
create unique index repos_name_nocase on repos (name collate nocase);
```

`0002_conversations`:

| table | columns | notes |
|-------|---------|-------|
| `conversations` | `id` pk, `session_id` unique, `session_started`, `title`, `status`, `linked_repo`, `ended`, `context_used`, `context_limit`, `last_entry_id`, `created_at`, `updated_at` | `last_entry_id` hands out entry ids; `tasks` is read from `tasks` |
| `messages` | (`conversation_id`, `id`) pk, `author` (`you` or `townhall`), `text`, `attachments` (JSON ids), `at` | your and townhall's entries, searchable by `text` |
| `events` | (`conversation_id`, `id`) pk, `kind`, `data` (the entry body as JSON, without `kind`), `at` | every other entry kind |
| `tasks` | `name` pk `collate nocase`, `conversation_id`, `title`, `prompt`, `repo`, `base_branch`, `after` (JSON names), `state`, `attachments` (JSON ids), `pr_number`, `pr_url`, `pr_checks`, `created_at` | indexed by conversation and by state |
| `attachments` | `id` pk, `conversation_id`, `name`, `mime`, `size`, `kind`, `created_at` | the bytes are files, see the layout |

Every `conversation_id` references `conversations (id) on delete cascade`. Enum columns hold the serde names (`yourTurn`, `needsYou`, `inCi`).

## Store functions

Routes and units call these typed async functions; they never write SQL. A unit that owns tables writes its SQL in its own `store/unit_*.rs` (`impl Store` blocks over `Store::call(|conn| ...)`); `rusqlite` stays inside `store/`.

| area | `townhall_core::store::Store` |
|------|-------------------------------|
| open | `Store::open(&home, bus)`, `Store::path(&home)`, `home()`, `bus()`, `call(f)` |
| repos | `repos()`, `add_repo(repo)`, `ensure_repo(repo)`, `remove_repo(name)`. Prefer `RepoRegistry` (git checks) or `Context::repos()` / `repo(name)` |
| settings | `settings()`, `set_setting(key, value)`, `desktop_repo()`, `set_desktop_repo(path)` |
| conversations | `create_conversation()`, `conversation(id)`, `conversations()`, `update_conversation(id, change)`, `detail(id)` |
| thread | `append_entry(conversation_id, body)`, `entries(conversation_id)` |
| tasks | `create_task(conversation_id, NewTask)`, `tasks(conversation_id)`, `all_tasks()`, `find_task(name)`, `update_task(name, change)` |
| attachments | `save_attachment(conversation_id, name, mime, bytes)`, `attachments(conversation_id)`, `attachment(conversation_id, id)` (record and file path) |

`change` closures take a copy and return the new value; they must not call the store. `update_conversation` keeps `id`, `createdAt` and `tasks` and sets `updatedAt`; `update_task` keeps `name`, `conversationId` and `createdAt`. `create_task` checks the name (`InvalidName`), that it is free ignoring case (`Conflict`), that every `after` task and attachment exists (`InvalidInput`), and dedups both lists.

Shared code around the store:

| item | what |
|------|------|
| `townhall_core::Context` | `startup()`, `store()`, `registry()`, `bus()`, `base_url()`, `claude_bin()`, `settings()`, `ensure_startup_repo()`, `startup_repo_name()`, `repos()`, `repo(name)`, `workspace(&repo)`. Built by `AppState`; core units get it from `start(ctx)` |
| `townhall_core::repos::RepoRegistry` | `list`, `get`, `add(&AddRepo)`, `ensure(path)`, `remove`; `resolve_path`, `inspect`, `names::*` |
| `townhall_core::fleet` | `list(ctx)` every `AgentInfo`, `holders(ctx, name)`, `locate(ctx, name, repo)`, `unregister(ctx, name)` |
| `townhall_core::agent` | `start`, `stop`, `list`, `send`, `output`, `LaunchOptions`, `ClaudeCodeAdapter::new(ctx.claude_bin())`, `ClaudeCodeAdapter::args`, `state::detect(ws, agent)` |
| `townhall_core::mcp` | names, `tools()`, `allowed_tools()`, `config_json()` |
| `townhall_daemon` | `AppState` (`new`, `with_options`, `open`, `with_store`, `ctx()`), `start_background(&state)`, `extract::JsonBody`, `error::ApiError::NotImplemented`, `cors::origin_allowed` |

`start_background(&state)` (daemon `main.rs` and the desktop app's in-process daemon) registers the startup repo, then calls `runner::start(ctx)`, `scheduler::start(ctx)` and `agent::watch::start(ctx)`. Each returns `Option<JoinHandle<()>>`, `None` today; `Background::stop()` aborts them when the desktop app restarts its daemon.

## Ownership map

A unit edits only the files it owns. Shared files (`lib.rs`, `routes/mod.rs`, `model/*`, `store/mod.rs` and the 2.1 store files, `context.rs`, `background.rs`, `types.ts`, this doc) change only through the orchestrator, with one exception: 2.4, 2.7 and 2.8a each add their `pub mod` line to `crates/townhall-core/src/lib.rs`.

| Unit | Owns | Calls |
|------|------|-------|
| 2.1 contract, repos, storage (and 2.0) | `crates/townhall-core/src/{model/, store/mod.rs, store/codec.rs, store/lock.rs, store/migrate.rs, store/import.rs, store/repos.rs, store/settings.rs, store/conversations.rs, store/thread.rs, store/tasks.rs, store/attachments.rs, store/migrations/0001_init.sql, store/migrations/0002_conversations.sql, repos/, fleet.rs, context.rs, events.rs, clock.rs, ids.rs, fsutil.rs, agent/adapter.rs, agent/claude.rs, agent/claude_tests.rs}`, `crates/townhall-daemon/src/{state.rs, background.rs, extract.rs, error.rs, cors.rs, main.rs, routes/mod.rs, routes/repos.rs, routes/settings.rs, routes/agents.rs, routes/stream.rs, routes/info.rs}`, `src-tauri/src/{settings.rs, daemon.rs, commands.rs}`, `web/src/api/types.ts`, `docs/daemon-api.md` | |
| 2.2 conversations | `crates/townhall-core/src/runner/`, `crates/townhall-core/src/store/unit_2_2.rs` and `store/migrations/01xx_*.sql`, `crates/townhall-daemon/src/routes/{conversations.rs, events.rs}`, its tests | `Store::{create_conversation, conversation, conversations, update_conversation, detail, append_entry, entries, attachment}`, `Context::{bus, settings, base_url, claude_bin, repos, repo}`, `mcp::{config_json, allowed_tools}`, `ids::new_session_id`, `repo_infer::infer` (2.4), `cors::origin_allowed`, `JsonBody` |
| 2.3 tasks | `crates/townhall-core/src/scheduler/`, `crates/townhall-core/src/mcp/mod.rs` (the JSON-RPC handling it adds), `store/unit_2_3.rs` and `store/migrations/02xx_*.sql`, `crates/townhall-daemon/src/routes/{tasks.rs, mcp.rs}` | `Store::{create_task, tasks, all_tasks, find_task, update_task, append_entry, conversation, update_conversation, attachment}`, `Context::{repo, settings, workspace, claude_bin}`, `agent::{start, list}`, `LaunchOptions` with `permission_mode` from settings, `fleet::holders`, `model::CreateTaskInput` |
| 2.4 repo inference | `crates/townhall-core/src/repo_infer/` (`Candidate`, `Rule`, `Inference`, `infer`), plus its `pub mod repo_infer;` line | `Context::repos()` for candidates; 2.2 maps `Inference::Found` to `EntryBody::Repo` (`Rule` to `RepoRule`) and `Ask` to `EntryBody::RepoAsk` |
| 2.5 agent states | `crates/townhall-core/src/agent/{state.rs, watch.rs}`, `store/unit_2_5.rs` and `store/migrations/05xx_*.sql`, the permission-mode plumbing into task launches with 2.3 | `fleet::list`, `EventBus::publish(ServerEvent::AgentUpdated)`, `Store::{find_task, update_task, append_entry}` (`needsYou`, `agent` entries), `Context::settings` |
| 2.6 attachments | `crates/townhall-daemon/src/routes/attachments.rs`, `store/unit_2_6.rs` and `store/migrations/06xx_*.sql` | `Store::{save_attachment, attachments, attachment}`, `cors::origin_allowed`, `model::MAX_ATTACHMENT_BYTES`; 2.2 reads images for the session |
| 2.7 steps and stats | `crates/townhall-core/src/{steps/, stats/}` (`Step`, `ShippedDay`), their `pub mod` lines, `store/unit_2_7.rs` and `store/migrations/07xx_*.sql`, `crates/townhall-daemon/src/routes/{steps.rs, stats.rs}` | `fleet::locate`, `Context::{repos, repo}`, `RepoQuery` in `routes/agents.rs` for `?repo=` |
| 2.8a definitions | `crates/townhall-core/src/definitions/`, its `pub mod` line | `LaunchOptions` fields (owned by 2.1, already there), `model::PermissionMode` (refuse `BypassPermissions`) |
| 2.8b library | `store/unit_2_8b.rs` and `store/migrations/080x to 084x_*.sql`, `crates/townhall-daemon/src/routes/library.rs`, `crates/townhall-core/src/mcp/library.rs` | `Store::call` for its tables, `definitions::*` (2.8a), `JsonBody` |
| 2.8c runs | `store/unit_2_8c.rs` and `store/migrations/085x to 089x_*.sql`, `crates/townhall-daemon/src/routes/runs.rs`, `crates/townhall-core/src/mcp/runs.rs` | `Store::{call, create_task, update_task, append_entry}`, 2.3's scheduler, 2.7's transcript reader |

## Open items

- The storage plan asks about backups (`VACUUM INTO`) and per-repo settings; neither is built.
- `GET /api/info` adds `userName`; `types.ts` has it as optional and phase 1's guard does not read it yet (3.1 does).
