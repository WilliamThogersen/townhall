# Daemon API (phase 2 contract)

The contract every phase 2 unit builds on. Rust types live in `crates/townhall-core/src/model/` (serde camelCase) and are mirrored field for field in `web/src/api/types.ts`. Read this file and the code; you should not need to ask.

## Conventions

- Base: `http://127.0.0.1:7777/api` (desktop) or same-origin `/api` (browser, Vite proxy).
- Every HTTP response is the envelope `{ "success": bool, "data": T | null, "error": string | null }` (`crates/townhall-daemon/src/envelope.rs`). `data` is shown below as the response.
- JSON field names are camelCase. A Rust `Option` is always present on the wire, as `null` when empty. Timestamps are RFC 3339 UTC strings with milliseconds (`2026-11-04T10:42:05.000Z`, `townhall_core::clock`), so they sort as text.
- Bodies go through `townhall_daemon::extract::JsonBody<T>`: a body that is not JSON, has the wrong content type or the wrong fields is `400` in the envelope, with the error `expected content-type application/json` or `invalid json body: <what serde said>` (for example ``invalid json body: missing field `path` at line 1 column 2``). Every phase 2 request body type refuses unknown fields (`deny_unknown_fields`); phase 1's `/send` body still ignores them.
- Error codes: `400` invalid input or name, `404` not found, `409` conflict (taken name, ambiguous agent, repo in use), `413` attachment over 20 MB, `501 not implemented yet` (a route whose unit has not landed), `503` the database is held by another townhall, `500` anything else (logged).
- Copy in `error` strings is lowercase and has no `error:` prefix; the UI adds it.
- CORS allows only the desktop and Vite origins (`cors::ALLOWED_ORIGINS`), methods `GET POST PUT PATCH DELETE`. CORS does not cover websockets or multipart posts: `/api/events` and the upload route must call `cors::origin_allowed(&headers)` and answer `403` when it is false.

## Routes

Agents come only from tasks: the scheduler starts one when a conversation's task is ready, under the task's name (see Scheduler). There is no route that creates an agent (2.13 removed phase 1's `POST /agents`, which now answers `405`); the `/agents` routes list running agents, stop them, send to them and stream their terminals.

`?repo=<name>` is accepted by every `/api/agents/{name}...` route. Without it the name is looked up across every registered repo (ignoring case, as macOS folders do; an agent whose name differs only by case is `404`, never acted on); two repos with that agent is `409` (`agent `x` exists in several repos (a, b): pass ?repo=<name>`). Repo names match ignoring case.

| Method | Path | Body | Response | Errors | Unit |
|--------|------|------|----------|--------|------|
| GET | `/info` | | `{ home, repo, repoKey, userName }` (`userName` from `git config user.name`, or `null`) | | 2.1 |
| GET | `/repos` | | `Repo[]` in the order added | 503 | 2.1 |
| POST | `/repos` | `AddRepo { path, name?, baseBranch? }` | `201 Repo` | 400 not absolute, missing, not a folder, `not the top folder of a git repo` (a subfolder, a plain folder, a folder under `~/.townhall/worktrees/`, or a linked worktree on a `townhall/*` branch), blank or bad name, base branch not found; 409 path registered, name taken ignoring case | 2.1 |
| DELETE | `/repos/{name}` | | the removed `Repo`. Only unregisters: folder, worktrees and branches stay | 404; 409 it has live agents (tmux sessions), or it is the daemon's startup repo | 2.1 |
| GET | `/agents` | | `AgentInfo[]` across every repo | 503 | 2.1 |
| DELETE | `/agents/{name}` | | the name | 400, 404, 409 | phase 1, 2.1 |
| POST | `/agents/{name}/send` | `{ text }` or `{ key }` (2.10), exactly one | the name. `text` is typed as it is (`tmux send-keys -l`, so `Down` or `C-c` stay text), then Enter. `key` (`up`, `down`, `enter`, `escape`, `tab`) is pressed alone, without text or Enter. When the agent is a task's, adds an `agent` `sent` entry to its conversation: the text, or the key as `↑`, `↓`, `⏎`, `esc`, `tab` | 400 (neither or both, an unknown key), 404 (also a dead pane), 409 | phase 1, 2.1, 2.10 |
| GET | `/agents/{name}/output` | | the pane text | 400, 404 (also a dead pane), 409 | phase 1, 2.1 |
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
| GET | `/search?q=&limit=` | | `SearchHit[]`, newest activity first, at most `limit` (default 50) | 400 `search needs 1 to 200 characters` (`q` trimmed), `limit needs to be a whole number from 1 to 200` | 3.3 |
| GET (ws) | `/events` | | `ServerEvent` JSON text frames, see below | 403 origin | 2.2 |
| POST | `/conversations/{id}/mcp` | JSON-RPC 2.0, see MCP | JSON-RPC answer, or `202` with no body for notifications | 404 | 2.3 |
| GET | `/tasks?conversation=&repo=&state=` | | `Task[]`, oldest first | | 2.3 |
| GET | `/tasks/{name}` | | `Task` | 404 | 2.3 |
| POST | `/conversations/{id}/attachments` | multipart, one field `file` per request | `201 Attachment` | 400, 403 origin, 404, 413 | 2.6 |
| GET | `/conversations/{id}/attachments` | | `Attachment[]` | 404 | 2.6 |
| GET | `/conversations/{id}/attachments/{attachment}` | | the bytes with their `content-type` (not the envelope) | 404 | 2.6 |
| GET | `/library/agents` | | `AgentTypeSummary[]` by name, ignoring case | | 2.12 |
| GET | `/library/agents/{name}` | | `AgentType` (ignoring case) | 404 | 2.12 |
| PUT | `/library/agents/{name}` | the TOML text, `content-type: text/plain` or `application/toml` | `201 AgentType` new, `200 AgentType` replaced (ignoring case, the new spelling wins) | 400 wrong content type, not UTF-8, the error list (below), its `name` is not the path's | 2.12 |
| DELETE | `/library/agents/{name}` | | the deleted type's name | 404; 409 it is the last type | 2.12 |
| GET | `/library/workflows` | | workflow definitions | 501 | 2.8c |
| GET, PUT, DELETE | `/library/workflows/{name}` | PUT: TOML text | | 501 | 2.8c |
| POST | `/library/import` | one agent type's `.toml` as the body, like PUT | `201 AgentType` | 400 like PUT; 403 origin; 409 the name exists (replace it with PUT); 501 a workflow (a `[[step]]` table) | 2.12 |
| POST | `/workflows/{name}/runs` | `{ repo, input }` | the run | | 2.8c |
| GET | `/runs/{id}` | | run state | | 2.8c |

Every route above is registered in `crates/townhall-daemon/src/routes/mod.rs` now. A unit replaces the `501` handlers in its own file and keeps the handler names; it adds extractors (`State`, `Path`, `Query`, `JsonBody`) to the signatures as needed. The upload route already carries `DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)` (20 MB plus framing).

### Agent types (2.12, `routes/library.rs`)

An agent type says how a task's agent runs: 2.8a's agent definition (`definitions::AgentDef`) saved as TOML. Every task names one. The bodies are the TOML text, not JSON, kept exactly as written (comments too). Every save and import runs `Store::check_agent_type`: at most `Store::MAX_AGENT_SOURCE` (10 000) characters, since the prompt goes on claude's command line, then 2.8a's `check_agent`. A type that fails is `400` with the error list as `data` and every error on one line, without `error: `, as `error`:

```json
{ "success": false,
  "data": [{ "kind": "agent", "name": "x", "step": null, "field": "model", "message": "\"-p\" must not start with \"-\"", "line": null }],
  "error": "agent \"x\": model \"-p\" must not start with \"-\"" }
```

`line` is set for TOML parse errors. Import checks the origin (`403`) because a `text/plain` post needs no CORS preflight. Deleting a type that queued tasks name is allowed: those tasks fail when they start (see Scheduler). The last type cannot be deleted, so a task always has a type to run.

The seed: `0801_seed_builder` saves `builder` once, on the first open with no agent types; it is never saved again, also not after you delete it:

```toml
# the agent type tasks run as when they name none
name = "builder"
description = "implements a task, tests first"
adapter = "claude"
model = "sonnet"
permission_mode = "acceptEdits"
prompt = """
You implement the task in your brief.
- Write a failing test first, then the code that makes it pass.
- Commit in small steps with conventional commit messages (feat:, fix:, test:, refactor:).
- Stay inside the task: no unrelated changes, refactors or new dependencies.
- When you are blocked or the brief is unclear, stop and ask instead of guessing.
"""
```

No `tools`, so claude's defaults apply.

## Types

All in `townhall_core::model`, mirrored in `web/src/api/types.ts`.

- `Repo { name, path, baseBranch }`. `name`: `[A-Za-z0-9._-]`, starts with a letter or digit, at most 64, unique ignoring case, the folder name by default. `path`: canonical, the top folder of a git work tree, never one of townhall's own worktrees. `baseBranch`: origin's HEAD branch (the local branch when it exists, else `origin/x`), else the current branch, else `HEAD`.
- `AgentInfo { name, repo, kind, branch, worktree, alive, state }`: phase 1's `Agent` plus `repo` and `state`. `AgentState`: `{ kind: "working" }`, `{ kind: "waiting", question }`, `{ kind: "error", exitCode: number | null }`, `{ kind: "ended" }`.
- `AgentKey`: `up`, `down`, `enter`, `escape`, `tab`, the `key` of `POST /agents/{name}/send` (`AgentKey::symbol` is how a `sent` entry shows it).
- `Settings { permissionMode, workWithoutAsking, agentsAtOnce, repoUnclear, sessionPanelOpen, notifyWhen, townhallModel }`, the same names, values and defaults as `web/src/settings/settings.ts` (a type test checks it). `townhallModel` (2.11) has no row on the settings page yet.
- `Conversation { id, sessionId, sessionStarted, title, status: "thinking" | "yourTurn", tasks: string[] (task names, oldest first), linkedRepo: string | null, ended, context: { used, limit } | null, createdAt, updatedAt }`. `title` starts as `new conversation`. `sessionStarted` is false until the first turn went through (that turn uses `--session-id`, later ones `--resume`).
- `ThreadEntry { id, at } & EntryBody`. `id` counts 1, 2, 3 ... per conversation. `EntryBody` is tagged by `kind`:

| kind | fields | shown as |
|------|--------|----------|
| `you` | `text, attachments: string[]` (attachment ids of this conversation) | your message |
| `townhall` | `text` (Markdown) | townhall's reply |
| `repo` | `repo, baseBranch, rule, reason` | `~ repo shop · develop, it has src/search/query.ts` |
| `repoAsk` | `question, candidates: string[], offerAll` | needs-you block from townhall: one button per candidate, plus `both` (2 candidates) or `all` when `offerAll`. A button sends its text as a normal message |
| `tasksCreated` | `tasks: string[]` | task block |
| `needsYou` | `agent, question, choices: string[]` | `◆ <agent> needs you`, the question verbatim, one `send <choice>` per choice (through `/agents/{name}/send`). A choice goes as `text`, except in claude's folder trust dialog (`claude asks whether to trust this worktree`), which reads keys and would take any text as `no, exit`: `yes, trust it` sends the key `up`, then `enter` (200 ms apart), and `no, exit` sends `enter`. That dialog has no free text answer |
| `agent` | `agent, event: "sent" \| "passed" \| "failed" \| "ended", text` | `→` `✓` `×` `○` lines |
| `error` | `message` | townhall's own failure |

- `RepoRule`: `named`, `code`, `only` (2.4's `repo_infer::Rule`, same names), `picked` (an answer to a `repoAsk`), `linked` (the row menu).
- `Task { name, title, prompt, repo, baseBranch, after: string[], state, conversationId, attachments: string[], createdAt, pr: { number, url: string | null, checks: "pending" | "passed" | "failed" | "none" } | null, agent }`. `agent` (2.12) is the stored name of the agent type it runs as, resolved when it was created (`builder` for tasks from before 2.12). `types.ts` has it optional until a web unit reads it (`parseTask` drops it for now). `name` is the agent name and the branch `townhall/<name>` (`Task::branch()`), unique across every conversation and repo ignoring case. `TaskState`: `queued`, `working`, `needsYou`, `inCi`, `inReview`, `merged`, `ended`, `error`.
- `Attachment { id, conversationId, name, mime, size, kind: "image" | "file", createdAt }`. `kind` is `image` for `image/*`. At most 20 MB (`MAX_ATTACHMENT_BYTES`).
- `Step { id, tool, summary, startedAt, endedAt: string | null, result: "running" | "ok" | "error" }` and `ShippedDay { date: "YYYY-MM-DD", fixes, features, lines, agents }`: the Rust types belong to 2.7 (`steps/`, `stats/`); `types.ts` already mirrors them.
- `AgentType { name, description, adapter: "claude" | "shell", model, permissionMode: "default" | "acceptEdits" | "plan" | null, tools: string[], prompt, command, source, updatedAt }` (2.12): the parsed `AgentDef` plus its TOML as written. `AgentTypeSummary { name, description, adapter, model, permissionMode, updatedAt }`: a row of the list. `DefinitionError { kind: "agent" | "workflow", name, step, field, message, line }`: 2.8a's error.
- `CreateTaskInput`, `NewTask`, `NewMessage`, `ConversationPatch`, `AddRepo`, `PutSetting`: request bodies, see the routes and MCP. `NewTask` carries the resolved `agent`.
- `SearchHit { conversationId, title, match: "title" | "message", entryId: number | null, snippet: string | null, updatedAt }` (`townhall_core::search`, mirrored in `web/src/api/search.ts`): one per conversation whose title or one of whose `you` and `townhall` entries holds `q`, ignoring case (every letter, not only ASCII) and runs of whitespace; `%` and `_` are plain characters. A title match wins and has `entryId` and `snippet` `null`; else `entryId` is the newest matching entry and `snippet` that entry on one line, about 80 characters around the first match, cut between words where it can, with `…` where it was cut. `updatedAt` is the conversation's.

## Attachments

Unit 2.6, `routes/attachments/` and `runner/content.rs`.

- Upload: the origin check comes first (`403`), then the conversation (`404`), then the body: exactly one multipart field `file` holding a non-empty file (`400` otherwise), counted while it streams and stopped past 20 MB (`413`, nothing kept).
- `mime`: png, jpeg, gif and webp come from the file's first bytes; anything else from the part's content type, then the extension. An image type the bytes do not back up (an svg, text sent as `image/png`) is never kept, so `kind: "image"` is always a real png, jpeg, gif or webp.
- `name`: the last path part without control or invisible (bidi) characters and without edge dots or spaces, at most 120 bytes with a 1 to 10 letter extension kept, `file` when nothing is left. An image gets the extension of its type (`shot` becomes `shot.png`), so an agent's Read tool shows it as an image.
- Download: the bytes with `content-type`, `content-length`, `content-disposition` (`inline` for those images, `attachment` for the rest, an ASCII `filename` plus `filename*`), `x-content-type-options: nosniff` and `content-security-policy: sandbox`. Only a regular file directly in its attachment folder is served; a symlink or a missing file is `404`.
- Townhall's session: up to 10 images per message, each at most 5 MB of base64, go in as `{"type":"image","source":{"type":"base64","media_type":...,"data":...}}` blocks before the text block. The text ends with `attachments of this message (pass their ids to create_task):` and one line per attachment, `- <id> (<name>, <mime>)`, with `image n above` or the reason an image was only listed.
- Tasks: `Store::attachment` gives the path of the bytes, absolute whenever the home is, for 2.3 to append to an agent's prompt.

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
| `townhallModel` | `sonnet`, `opus`, `haiku`: the model of townhall's own session, `--model=<value>` (2.11) | `sonnet` |

A missing or invalid stored value reads as its default. The store also keeps `desktop.repo` (the desktop app's chosen repo, a path); it is not a setting and not in the API.

## Permission mode

`LaunchOptions` (`townhall_core::agent`) has `prompt, command, permission_mode, session_id, model, allowed_tools, system_prompt, add_dirs`. `ClaudeCodeAdapter` (`agent/claude.rs`) builds this argv:

```
claude [--model=M] [--permission-mode=MODE | --dangerously-skip-permissions]
       [--append-system-prompt=P] [--session-id=ID] [--allowedTools=T1,T2]
       [--add-dir=D ...] [-- PROMPT]
```

- Every option and its value is one argv element (`--name=value`), and the launch command shell-quotes every element, so no value can split or start a new flag. `--allowedTools` is one comma-joined element, so its position does not matter.
- The prompt is the only positional, after the end-of-options `--`. A prompt such as `--dangerously-skip-permissions`, `-p`, `--permission-mode=bypassPermissions` or `--` arrives as the prompt, never as a flag.
- `claude` still runs a subcommand when the first positional equals its name, even after `--` (`claude -- mcp` runs `claude mcp`). A prompt without any whitespace therefore gets a trailing newline; no subcommand name has one.
- `add_dirs` are folders outside the worktree claude may read without a permission prompt (in `default`, `acceptEdits` and `plan` its Read asks for anything outside the working directory and `--add-dir` folders): one `--add-dir=<folder>` each, refused unless absolute and UTF-8, and when it starts with `-` or holds control characters.
- `model`, `session_id` (a UUID) and each `allowed_tools` entry are refused (`InvalidInput`) when they start with `-` or hold control characters; a tool may not hold a comma. The system prompt may hold anything, since it sits inside its own `--append-system-prompt=` element. Blank values add no flag; no prompt means no `--`.
- `permission_mode`: `default`, `acceptEdits`, `plan` and `auto` become `--permission-mode=<mode>`; `bypassPermissions` becomes `--dangerously-skip-permissions`. `claude` 2.1.318 lists `acceptEdits, auto, bypassPermissions, manual, dontAsk, plan` and no longer lists `default`, but still accepts it, so `default` is passed as is for now.
- Checked against the installed `claude` 2.1.318 (empty environment, throwaway `HOME`, an invalid `--session-id` so it stops right after parsing): `-- --no-such-flag` reaches the session-id check while `--no-such-flag` alone is `unknown option`, and `--allowedTools=Read,Bash(git diff:*),Edit` and the `--name=value` forms parse.
- The launch command puts `env -u <name>` for each of `agent::OUTER_SESSION_ENV` in front of `claude` (see Environment below). `ClaudeCodeAdapter::args(&options)` returns the argv after the program unquoted, without that prefix, for a caller that spawns `claude` without a shell (2.2's runner, which removes the same variables itself).
- Townhall's own session (`runner/args.rs`) follows the same rules, spawned without a shell: `claude -p --output-format=stream-json --verbose --input-format=stream-json --permission-prompt-tool=stdio --include-partial-messages (--session-id=ID | --resume=ID) --model=M --permission-mode=default --append-system-prompt=P --strict-mcp-config --mcp-config=JSON --tools=Read,Grep,Glob --allowedTools=T1,T2 --disallowedTools=T1,T2 --setting-sources= --settings={"disableAllHooks":true} --disable-slash-commands [--add-dir=D ...]`. `M` is the `townhallModel` setting, read at the start of every turn. The session id, the MCP config and each folder are refused (`InvalidInput`, the turn fails) when they start with `-` or hold control characters, a folder also unless absolute and UTF-8. The system prompt may hold newlines.

### Townhall's session leaves your own Claude setup out

Unit 2.11. Townhall's session plans tasks; your hooks, CLAUDE.md, skills, slash commands, plugins and extra built-in tools must not steer it. The flags were picked from `claude --help` of `claude` 2.1.318 (nothing else was run), all as one `--name=value` element each (`runner::ISOLATION` plus `--tools`):

| flag | why |
|------|-----|
| `--setting-sources=` (empty) | loads none of the user, project and local settings files: no hooks, `enabledPlugins`, permission rules or `env` from them, and no CLAUDE.md, which claude loads only for an enabled source (the Agent SDK runs claude the same way when its `settingSources` is empty). Managed (policy) settings still apply, as they should |
| `--settings={"disableAllHooks":true}` | hooks stay off even if some source still had one (a plugin, a future default) |
| `--disable-slash-commands` | "disable all skills and commands" |
| `--tools=Read,Grep,Glob` | the whole built-in tool set: no Bash, Edit, Write, WebFetch, Task and the like exist in the session. MCP tools are not built in, so townhall's own stay |
| `--strict-mcp-config --mcp-config=...` (as before) | townhall's MCP server only, none of yours |

`--allowedTools` (townhall's MCP tools plus `READ_ONLY_TOOLS` = Read, Grep, Glob) and `--disallowedTools` stay as they were; `LS` left the list, since `--tools` names the built-in set.

Auth keeps working: it comes from your login (the macOS keychain or `~/.claude/.credentials.json`, with the account in `~/.claude.json`) and from `ANTHROPIC_*` / `CLAUDE_*` variables, which pass through (see Environment). None of these flags reads or skips them. Not chosen: `--bare`, which never reads OAuth or the keychain (only `ANTHROPIC_API_KEY` or an `apiKeyHelper`), so a subscription login would stop working; `--safe-mode`, a troubleshooting mode that also disables MCP servers, which could take townhall's own with it; `--restricted`, which overlaps the flags above but also confines and gates writes in ways townhall does not need. One limit: a provider or `apiKeyHelper` set only in your `~/.claude/settings.json` (not in the environment) is no longer read by townhall's session.

`permission_mode` always comes from `Context::settings()`, never from a request body or a tool call. A task's agent type can only narrow it (`AgentDef::launch_options`: the stricter of the two, `plan` < `default` < `acceptEdits` < `auto` < skip all), and no type can hold `bypassPermissions`. The binary is `Context::claude_bin()` (`TOWNHALL_CLAUDE_BIN`, default `claude`); tests point it at a fake script.

## Websocket: `/api/events`

One socket per client. Each text frame is one `ServerEvent`, tagged by `type`:

| type | fields | published by |
|------|--------|--------------|
| `conversationUpdated` | `conversation` | every conversation write, each appended entry, each created task |
| `entryAppended` | `conversationId, entry` | `Store::append_entry`, and `Store::note_task_created` when it adds a task to the turn's `tasksCreated` entry (same `id`) |
| `taskUpdated` | `task` | `Store::create_task`, `Store::update_task` |
| `agentUpdated` | `agent: AgentInfo` | the agent watcher (2.5) when a state changes |
| `resync` | | the socket itself, when its bus receiver lagged (`RecvError::Lagged`): refetch everything |

Store writes publish on `Context::bus()` (`townhall_core::events::EventBus`, a tokio broadcast of capacity 1024) while they hold the connection, so events arrive in write order. An append publishes `entryAppended`, then `conversationUpdated`; a created task publishes `taskUpdated`, then `conversationUpdated`. The socket handler (2.2) subscribes with `ctx.bus().subscribe()` and forwards each event as `serde_json::to_string(&event)`. A client puts entries in order by `id`, and an `entryAppended` whose `id` it already has replaces that entry.

## MCP: `POST /api/conversations/{id}/mcp`

JSON-RPC 2.0 over plain HTTP JSON (no SSE), like Hive's `src/main/plan/mcp.ts` and `rpc.ts`: `initialize` (answers `protocolVersion` (echo the client's, else `2025-06-18`), `capabilities: { tools: {} }`, `serverInfo: { name: "townhall", version }`, `instructions: mcp::INSTRUCTIONS`), `ping`, `tools/list`, `tools/call`, batches, and notifications (no `id`) answered with `202` and no body. A tool error comes back as a result with `isError: true` and a text content, so Claude can fix the call. Results are `{ content: [{ type: "text", text: <JSON> }] }`. The body goes through `JsonBody`, so a browser cannot post it without a CORS preflight.

The names, schemas and config are code in `townhall_core::mcp`: `SERVER_NAME` (`townhall`), `tools()`, `allowed_tools()` (`mcp__townhall__create_task`, `mcp__townhall__set_title`, `mcp__townhall__task_status`, `mcp__townhall__list_agent_types`, `mcp__townhall__read_agent_type`, `mcp__townhall__save_agent_type`), `endpoint(base_url, id)` and `config_json(base_url, id)`, which 2.2 passes as `--mcp-config`:

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
                     "description": "Attachment ids of this conversation the agent needs." },
    "agent": { "type": "string", "pattern": "^[A-Za-z0-9_-]+$", "minLength": 1, "maxLength": 64,
               "description": "The agent type the task runs as (list_agent_types). Default: builder when it exists, else the first type by name." }
  },
  "required": ["name", "title", "prompt"],
  "additionalProperties": false
}
```

`repo` defaults to the conversation's `linkedRepo`, else the repo of its latest `repo` entry; none of those is a tool error that lists the repos. `baseBranch` defaults to the repo's. `agent` (2.12) matches a saved type ignoring case and is stored with the type's spelling; left out it is `builder` when that exists, else the first type by name; an unknown type is a tool error that lists the types. 2.3 resolves both into a `NewTask`, calls `Store::create_task`, then `Store::note_task_created`. No tool takes a permission mode.

One `tasksCreated` entry per turn: a turn is everything after the conversation's latest `you` entry (the whole thread before the first). The turn's first task appends a `tasksCreated` entry; each later task of that turn is added to it in place (same `id` and `at`), and the entry is sent again as `entryAppended`. Calls are serialized, so tasks created at the same time still share the entry. Each refusal (`name` taken by a task, an agent or a branch `townhall/<name>`, unknown `repo`, `baseBranch`, `after` task or attachment, over a schema limit, an unknown field) is a result with `isError: true` and a text that says what to change. A notification runs nothing; an empty batch is `-32600`, an unknown method `-32601`, `tools/call` without `params.name` `-32602`.

`set_title` (result the `Conversation`):

```json
{ "type": "object", "properties": { "title": { "type": "string", "minLength": 1, "maxLength": 80 } },
  "required": ["title"], "additionalProperties": false }
```

`task_status` (2.11, `mcp/status.rs`): what this conversation's tasks are doing and did. Townhall's system prompt tells the session to call it whenever it is asked what a task did, instead of guessing.

```json
{ "type": "object", "properties": { "name": { "type": "string", "pattern": "^[A-Za-z0-9_-]+$", "minLength": 1, "maxLength": 64 } },
  "additionalProperties": false }
```

- Without `name`: `{ "tasks": [TaskStatus] }`, every task of this conversation, oldest first. `TaskStatus { name, title, repo, branch, state, after, agent, entries }` (`agent`: its agent type), `entries` the task's last 3 (`mcp::RECENT_ENTRIES`) `agent` and `needsYou` `ThreadEntry`s, oldest first.
- With `name` (ignoring case): that task's `TaskStatus` plus `steps` (2.7's `steps_for_agent` for its worktree `<home>/worktrees/<repo-key>/<name>`, the last 20, `mcp::MAX_STEPS`; `[]` without a worktree or transcript) and `lastMessage`: when the task is `ended` or `merged`, what its agent's last finished turn said (the transcript tail reader of agent states, newest transcript since the worktree was made, or any once the folder is gone), at most 2000 characters (`mcp::MAX_MESSAGE_CHARS`, the last one `…` when cut); otherwise, or without such a message, `null`.
- A task of another conversation (``task `x` belongs to another conversation: ...``), an unknown name or a blank one, and an unknown field are tool errors that list this conversation's tasks where it helps.

Agent types (2.12, `mcp/library.rs` and `mcp/ask.rs`):

| tool | input | result |
|------|-------|--------|
| `list_agent_types` | `{}` | `{ "types": [AgentTypeSummary], "default": "builder" }`, `default` the type `create_task` uses without `agent` (`null` with no types) |
| `read_agent_type` | `{ name }` | the `AgentType`, ignoring case. Unknown: a tool error that lists the types |
| `save_agent_type` | `{ source, confirmed? }` (`source` at most 10 000 characters) | `{ status: "saved", name, replaced }` or `{ status: "asked", name, replaces, next }`. A type that fails the checks is a tool error with one `error: agent ...` line per problem |

`save_agent_type` saves at once only a NEW name while the `workWithoutAsking` setting is on. Anything else, every replace of an existing type included, is not saved: townhall appends a `townhall` entry (Markdown: `save the new agent type `x`?` or `replace the saved agent type `x` with this one?`, the TOML in a `toml` code fence longer than any backtick run in it, then `answer yes to save it, or say what to change.`), records the ask in `agent_type_asks`, and answers `asked` with `next` telling the session to end its turn and wait. Once you said yes, the session calls it again with `confirmed: true`; that saves only when this conversation's ask for that name holds exactly this `source` and a `you` entry came after the ask's entry, else it is a tool error that says why (not asked, a different TOML, no answer yet). A save closes the ask. The entry is a plain `townhall` entry because the web knows no definition block yet (3.7). Townhall's system prompt (`runner/prompt.rs`) says every task names an agent type, that it can design new types with you (personality, model, tools) and that a type's permission mode can only be stricter than the setting.

Workflow tools (`list_workflows`, `save_workflow`) wait for 2.8c, which also adds `run_workflow` in `mcp/runs.rs`. `tools()` and `allowed_tools()` pick them up.

## Scheduler

`scheduler::start(ctx)` runs a pass on every `taskUpdated` and every 2 s (`scheduler::TICK`); tests call `scheduler::run_once(ctx)`. A pass:

- starts queued tasks oldest first, each once every `after` task is `ended` or `merged` (an `error` one holds it) and while fewer than `agentsAtOnce` tasks are `working` or `needsYou` with a live session (an interactive claude stays alive after its work, so an `ended` task frees its slot; agents that are no task's do not count).
- Start: reads the task's agent type as saved now (2.12, `scheduler/agent_type.rs`), writes the task's brief, `# <title>`, the prompt and, with attachments, `Files attached to this task:` and one `- <path>` each, to `<home>/tasks/<name>/brief.md` (never the repo or the worktree), then `agent::start` in the task repo's workspace, name = the task name, base = the commit `baseBranch` is at, with the type's `AgentDef::launch_options(<permissionMode setting>)`: a `claude` type gets `ClaudeCodeAdapter::new(ctx.claude_bin())` with the type's `--model`, `--allowedTools` and `--append-system-prompt`, its mode only narrowing the setting, a new session id and `add_dirs` = the brief's folder plus, with attachments, the conversation's `files` folder; a `shell` type runs its `command` in the worktree (`ShellAdapter`, no session id). Then `working`. A task whose type was deleted after it was created goes to `error` with ``could not start: agent type `x` was deleted after the task was created: create the task again with one of: ...``. A prompt of any size fits, since tmux takes the command as one argument of at most about 16 k. The agent runs:

  ```
  env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID -u CLAUDE_CODE_ENTRYPOINT -u CLAUDE_CODE_SSE_PORT -u CLAUDE_EFFORT claude [--model=<model>] --permission-mode=<mode> [--append-system-prompt=<prompt>] --session-id=<uuid> [--allowedTools=<tools>] --add-dir=<home>/tasks/<name> [--add-dir=<home>/conversations/<id>/files] -- 'read your task brief at <home>/tasks/<name>/brief.md and do it. it is the full task.'
  ```

  (`--dangerously-skip-permissions` instead of `--permission-mode` for `skip all`). The prompt's own text only ever reaches claude through the brief.
- sets `error` with an `agent` `failed` entry (`could not start: <reason>`, at most 300 characters; a failed command reads ``<its stderr> (`<command>` exited with <code>)``, so the cut drops the long command line, never the reason) when the start fails, or when another agent has the task's name (in another repo, or in another case).
- adopts a queued task whose own agent (its exact name, in its repo) exists already (a start cut short by a restart) as `working`, never starting a second agent; a `working` task is never touched.
- marks an `ended` task `merged`, with an `agent` `passed` entry (`merged into <base>`), when `townhall/<name>` is reachable from its base branch (`git for-each-ref --merged=<base>`) and its tip is not the commit it started from (a branch with no commits of its own is not merged). Read only: it never merges or deletes.

## Agent states

Unit 2.5, `agent/state.rs` and `agent/watch.rs`.

- `state::detect(ws, agent)` reads Claude's transcripts from `state::claude_projects()` (`$TOWNHALL_CLAUDE_PROJECTS`, else `~/.claude/projects`); `state::detect_in(projects, ws, agent)` takes that folder as an argument (`None`: the screen only), so tests pass a temp folder.
- No session is `ended`. A dead pane (sessions keep it with `remain-on-exit`) is `ended` when it exited 0, else `error` with the exit code (a signal as 128 plus its number). A live pane is `waiting` when the transcript has a pending `AskUserQuestion` or a turn that ended on a question, when the screen shows a selection prompt (read even while the transcript says busy: claude 2.1.318 writes a message with parallel tool calls only once every call has a result, so a call waiting for permission leaves the transcript on another call's result; the screen is captured again only after the pane printed), when the screen shows claude's folder trust dialog (every new worktree: `Yes, I trust this folder` above `No, exit`, numbered or not), asked as `claude asks whether to trust this worktree` (the watcher answers it for townhall's own worktrees, see below), or, for a program without a transcript, when its last line asks. A live Claude whose turn ended without a question `state::IDLE_SETTLE_MS` (10 s) ago or longer is `ended`: interactive Claude never exits on its own, and a short pause between tool calls must not count. Anything else live is `working`. When tmux cannot be asked, alive is `working` and dead is `ended`.
- `state::choices(question)`: what can be typed back, the numbers of the question's numbered options, else the two words of a `y/n` marker, else nothing. The trust question's are `yes, trust it` and `no, exit`, but the dialog takes keys, not text: its cursor starts on `No, exit`, so `{ "key": "up" }` then `{ "key": "enter" }` trusts the worktree, while any text (sent with its Enter) picks `No, exit` and claude exits. The watcher puts them in the `needsYou` entry's `choices`.
- The watcher (`agent::watch::start`) polls every agent every 1.5 s (`watch::INTERVAL`). When a state changes it publishes `agentUpdated`, moves the agent's task (`working`, `needsYou`, `error`, `ended`; never a `queued`, `merged`, `inCi` or `inReview` one) and appends a `needsYou` or `agent` entry. An exit counts once seen on two polls in a row.
- Claude's folder trust dialog is answered for you when the agent's worktree is below `<home>/worktrees/` (`agent/watch/trust.rs`): the watcher presses `up`, then `enter` (200 ms apart), and a task's conversation gets an `agent` `sent` entry `trusted the worktree`; that poll's `waiting` is not acted on. Once per session (tmux pane id and start time), and never when the dialog is already the state acted on (so a restart does not answer again): a dialog still there on the next poll stays `waiting`, with its `needsYou` entry. No other prompt, and no agent outside townhall's worktrees, is ever answered.
- A dead pane answers like a missing session: `POST /agents/{name}/send` and `GET /agents/{name}/output` are `404`, and the stream sends `error: ...` and closes.

## Environment

| variable | what | default |
|----------|------|---------|
| `TOWNHALL_HOME` | townhall's folder, made absolute | `~/.townhall` |
| `TOWNHALL_REPO`, `TOWNHALL_BIND` | the daemon's startup repo and address (`townhall-daemon --repo --bind`) | `.`, `127.0.0.1:7777` |
| `TOWNHALL_CLAUDE_BIN` | the `claude` program; tests point it at a fake script | `claude` |
| `TOWNHALL_CLAUDE_PROJECTS` | where Claude's transcripts are read from, for steps (2.7) and agent states (2.5); tests point it at a temp folder | `~/.claude/projects` |
| `TOWNHALL_TMUX_SOCKET` | a private tmux server for every tmux call townhall makes (`tmux -L <name>`): letters, digits, `.`, `_` and `-`, anything else is refused. Every test harness sets it once per test process (`townhall-test-<pid>-<nanos>`, `tests/common/tmux_server.rs`) and kills that server at exit, so tests never share the default server, or its stale environment, with real agents | the default server |

Every new session also gets `-e PWD=<worktree>`, so a pane never inherits the `PWD` of whoever started the tmux server.

No claude townhall starts sees the variables a running Claude Code sets for its children (`agent::OUTER_SESSION_ENV`: `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_CODE_SSE_PORT`, `CLAUDE_EFFORT`), so a daemon started from inside Claude Code does not make its agents part of that session (`CLAUDE_EFFORT=xhigh` changed how they ran). Townhall's own session removes them from the process it spawns (`runner/process.rs`). A tmux agent's command starts with `env -u <each>` (`ClaudeCodeAdapter::launch_command`, `agent/claude.rs`), since tmux starts it with the tmux server's environment, which can hold them even when the daemon has none. Every other `CLAUDE_*` and `ANTHROPIC_*` variable passes through: it can carry the user's auth or provider setup.

## Storage: `~/.townhall`

```
~/.townhall/                                  (or $TOWNHALL_HOME made absolute, mode 0700)
  townhall.db                                 every repo, setting, conversation, entry and task
  townhall.db-wal, townhall.db-shm            SQLite's WAL files
  townhall.lock                               held while a townhall owns the database (holds its pid)
  conversations/<id>/files/<attachment>/<name>   attachment bytes
  tasks/<name>/brief.md                       a task's whole prompt, written when its agent starts
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

`0201_task_launches` (2.3): `task_launches (name pk collate nocase references tasks (name) on delete cascade, start_commit, session_id, launched_at)`: what each task's agent started from. `start_commit` tells `merged` from a branch that never got a commit; `session_id` is the agent's `--session-id` (`null` for an agent adopted after a restart).

2.8b range, agent types (2.12, `store/unit_2_8b.rs`): `0800_agents`: `agents (name pk collate nocase, description, source, updated_at)`, `source` the TOML as written. `0801_seed_builder`: saves `builder` when `agents` is empty (runs once). `0802_agent_type_asks`: `agent_type_asks (conversation_id references conversations on delete cascade, name collate nocase, source, entry_id, asked_at)`, primary key `(conversation_id, name)`: the types townhall asked you to confirm. `0803_task_agent`: `tasks.agent text not null default 'builder'`.

`0501_agent_states` (2.5): `agent_states (repo, agent, state, updated_at)`, primary key `(repo, agent)`, both `collate nocase`, `state` the `AgentState` as JSON: the state the watcher last acted on, so a restarted daemon repeats neither entries nor events. Detection never reads it. `Store::{seen_agent_states, see_agent_state, forget_agent_state}` (`store/unit_2_5.rs`) read and write it.

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
| tasks | `create_task(conversation_id, NewTask)`, `tasks(conversation_id)`, `all_tasks()`, `find_task(name)`, `update_task(name, change)`; 2.3: `note_task_created(conversation_id, name)`, `record_launch(TaskLaunch)`, `task_launch(name)`, `task_launches()` (`scheduler::TaskLaunch`) |
| attachments | `save_attachment(conversation_id, name, mime, bytes)`, `attachments(conversation_id)`, `attachment(conversation_id, id)` (record and file path) |
| search | `search(SearchQuery)` (3.3, `store/search.rs`, read only): `SearchQuery::parse(q, limit)` checks the text and limit |
| agent types | 2.12, `store/unit_2_8b.rs`: `Store::check_agent_type(source)` (the error list), `agent_types()`, `agent_type(name)`, `save_agent_type(source)` (`(AgentType, replaced)`, `InvalidInput` when it fails the checks), `delete_agent_type(name)` (`Missing`, `Conflict` for the last), `ask_agent_type`, `agent_type_ask`, `forget_agent_type_ask` |

`change` closures take a copy and return the new value; they must not call the store. `update_conversation` keeps `id`, `createdAt` and `tasks` and sets `updatedAt`; `update_task` keeps `name`, `conversationId` and `createdAt`. `create_task` checks the name (`InvalidName`), that it is free ignoring case (`Conflict`), that every `after` task and attachment exists (`InvalidInput`), and dedups both lists.

Shared code around the store:

| item | what |
|------|------|
| `townhall_core::Context` | `startup()`, `store()`, `registry()`, `bus()`, `base_url()`, `claude_bin()`, `settings()`, `ensure_startup_repo()`, `startup_repo_name()`, `repos()`, `repo(name)`, `workspace(&repo)`. Built by `AppState`; core units get it from `start(ctx)` |
| `townhall_core::repos::RepoRegistry` | `list`, `get`, `add(&AddRepo)`, `ensure(path)`, `remove`; `resolve_path`, `inspect`, `names::*` |
| `townhall_core::fleet` | `list(ctx)` every `AgentInfo`, `holders(ctx, name)`, `locate(ctx, name, repo)`, `unregister(ctx, name)` |
| `townhall_core::agent` | `start`, `stop`, `list`, `send`, `send_key`, `output`, `OUTER_SESSION_ENV`, `LaunchOptions`, `ClaudeCodeAdapter::new(ctx.claude_bin())`, `ClaudeCodeAdapter::args`, `state::detect(ws, agent)`, `state::detect_in(projects, ws, agent)`, `state::choices(question)`, `state::IDLE_SETTLE_MS` |
| `townhall_core::mcp` | names, `tools()`, `allowed_tools()`, `config_json()` |
| `townhall_daemon` | `AppState` (`new`, `with_options`, `open`, `with_store`, `ctx()`), `start_background(&state)`, `extract::JsonBody`, `error::ApiError::NotImplemented`, `cors::origin_allowed` |

`start_background(&state)` (daemon `main.rs` and the desktop app's in-process daemon) registers the startup repo, then calls `runner::start(ctx)`, `scheduler::start(ctx)` and `agent::watch::start(ctx)`. Each returns `Option<JoinHandle<()>>`, `None` until its unit lands (the scheduler runs since 2.3); `Background::stop()` aborts them when the desktop app restarts its daemon.

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
