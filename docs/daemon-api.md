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
| GET | `/info` | | `{ home, repo, repoKey, userName, version }` (`userName` from `git config user.name`, or `null`; `version` is the build's version, `0.2.0-alpha.1+d812d0d` in a dev build) | | 2.1 |
| GET | `/repos` | | `Repo[]` in the order added | 503 | 2.1 |
| POST | `/repos` | `AddRepo { path, name?, baseBranch?, cloneDir? }` | `201 Repo` | 400 not absolute, missing, not a folder, `not the top folder of a git repo` (a subfolder, a bare repo, a folder under `~/.townhall/worktrees/`, or a linked worktree on a `townhall/*` branch), blank or bad name, base branch not found, `is not a git url`, `could not clone` (authentication failed, repository not found, or git's own message), relative `cloneDir`; 409 path registered, name taken ignoring case, clone target folder already exists | 2.1 |
| PATCH | `/repos/{name}` | `PatchRepo { name?, baseBranch?, path?, moveFolder? }` | `Repo` | 400 nothing to change, blank or bad name, base branch not found, `path` not absolute, not a folder, not the top folder of a git repo or not the same git repo, `moveFolder` without `path`, target folder's parent missing; 404 no such repo; 409 name or path taken, `moveFolder` target exists, the repo has worktrees, or (for a new path) it is the startup repo or has running agents | |

`PATCH` changes only the keys sent. `name` also renames the repo in its tasks, conversations and agent states. `baseBranch` must exist in the repo. `path` alone points the repo at a new place of the same git repo (same root commits; when the old folder is gone only the registered base branch must exist there). With `moveFolder` the folder is moved to `path`, which must not exist yet, and the registry follows. Accounts are set with `PUT /repos/{name}/accounts`.

`path` is a folder or a git link, told apart by its shape. A folder that is a git repo is added as it is. A folder that is not inside any git repo is turned into one first: `git init -b <baseBranch or main>`, then one commit of everything in it (an empty commit for an empty folder; a `townhall <townhall@localhost>` author when git has no identity). Townhall never does this to `/` or your home folder. A link (`https://`, `ssh://`, `git@host:owner/repo`, with or without `.git` or a trailing `/`) is cloned with your own git credentials and without prompts. Without `cloneDir` and with an empty `clonesDir` setting it goes into `~/.townhall/repos/<repo>` (`-2`, `-3` ... when that name holds another repo); with either set it goes into `<folder>/<repo>` (created when missing), an existing clone of the same link there is reused and any other existing folder is a 409, never overwritten. An earlier clone of the same link is reused, and `name` defaults to the repo's name made unique. A refused request leaves the folder untouched.
| DELETE | `/repos/{name}?deleteClone=` | | the removed `Repo`. Only unregisters: folder, worktrees and branches stay. With `deleteClone=true` it also deletes the folder, but only a clone townhall made directly in `~/.townhall/repos`, and only when every check passes first (a refusal changes nothing): not a clone townhall made (a folder of yours, `400 ... is never deleted`); `409` uncommitted changes, commits that are on no remote, linked worktrees left, or a task of the repo that is not `merged` or `ended` | 404; 409 it has live agents (tmux sessions), or it is the daemon's startup repo; the refusals above | 2.1, repo ui polish |
| PUT | `/repos/{name}/accounts` | `PutRepoAccounts { githubAccount?, gitlabAccount? }`: a login picks that registered account (a login of the `forgeAccounts` setting on that forge), `null` clears the pick, a missing key leaves it | the updated `Repo` | 400 no such account is registered for that forge, an unknown key; 404 | multi forge accounts |
| GET | `/forge-accounts/detected` | | `{ forge, login, host, registered }[]`: every account `gh` and `glab` are logged in with, from `gh auth status --json hosts` (older gh: the text of `gh auth status`) and the text of `glab auth status`, run with the programs `TOWNHALL_GH_BIN` and `TOWNHALL_GLAB_BIN` name (default `gh`, `glab`) and the same environment as `open_pr`. `registered` is true when the `forgeAccounts` setting has that forge, host and login (ignoring case). A program that is not installed, or has no login, adds nothing (an empty list, not an error). Logins a registered account could not have (a GitHub managed user with an underscore) are left out. Never reads or returns a token | the list | 400 a program did not answer within 15 seconds | auto detect forge accounts |
| GET | `/marketplaces` | | `Marketplace[]` in the order added, `townhall-official` first | 503 | plugins 1.3 |
| POST | `/marketplaces` | `{ source, name?, account? }` | `201 Marketplace` | 400 `source` is not a git url, `owner/repo` or absolute folder, a url with a user name or token in it, a folder or clone with no `marketplace.json` or an invalid one, bad name, `account` not registered for the host, `account` with a folder, `could not clone` (as for repos); 409 the name is taken ignoring case, or it is `townhall-official` | plugins 1.3 |
| DELETE | `/marketplaces/{name}` | | the removed `Marketplace` (with `plugins: []`) | 404; 409 it is `townhall-official` (`is built in: it cannot be removed`) or it still has installed plugins | plugins 1.3 |
| POST | `/marketplaces/{name}/refresh` | | the refreshed `Marketplace` | 400 `could not fetch` or an index that does not parse (the stored marketplace keeps its last good state and gets `error`); 404 | plugins 1.3 |
| GET | `/plugins` | | `{ available: AvailablePlugin[], installed: Plugin[] }` | 503 | plugins 1.4 |
| GET | `/plugins/preview?plugin=name@marketplace` | | `PluginPreview`: what an install would approve, nothing is kept | 400 no `plugin`, not `name@marketplace`, a manifest or file that breaks the rules, a symlink out of the plugin, a submodule, `needs plugin api N`, `could not clone`, `repository not found or not readable as <login>`; 404 no such marketplace or plugin | plugins 1.4 |
| POST | `/plugins` | `{ plugin, approve }` | `201 Plugin`, not enabled | 400 `approve` is not the hash of the permission list as it is now (also missing or empty), the refusals of `preview`; 404; 409 already installed | plugins 1.4 |
| PATCH | `/plugins/{id}` | `{ enabled?, pinned? }` | `Plugin` | 400 neither key, a non boolean, an unknown key; 404 | plugins 1.4 |
| POST | `/plugins/{id}/update?approve=` | | `PluginUpdate` | 400 `approve` is not the hash of the new permission list, the refusals of `preview`; 404 | plugins 1.4 |
| DELETE | `/plugins/{id}?purge=` | | the removed `Plugin` (`enabled: false`) | 404 | plugins 1.4 |
| GET | `/plugins/contributions` | | `Contributions`: starters, themes, the active theme and settings groups of enabled plugins | 503 | plugins 1.7 |
| PUT | `/plugins/theme` | `{ plugin: string \| null }` (the key is required; `null` clears) | `Contributions` | 400 a missing or non string `plugin`, an unknown key; 404 no such plugin; 409 the plugin is disabled or ships no valid theme | plugins 1.7 |
| PUT | `/plugins/{id}/settings/{key}` | `{ value }` | `Contributions` | 400 the value does not fit the field, a missing `value`, an unknown key; 404 the plugin is not enabled or has no such setting | plugins 1.7 |
| DELETE | `/plugins/{id}/settings/{key}` | | `Contributions` with the field back at its default | 404 as above | plugins 1.7 |
| GET | `/agents` | | `AgentInfo[]` across every repo | 503 | 2.1 |
| DELETE | `/agents/{name}` | | the name | 400, 404, 409 | phase 1, 2.1 |
| POST | `/agents/{name}/send` | `{ text }` or `{ key }` (2.10), exactly one | the name. `text` is typed as it is (`tmux send-keys -l`, so `Down` or `C-c` stay text), then Enter. `key` (`up`, `down`, `enter`, `escape`, `tab`) is pressed alone, without text or Enter. When the agent is a task's, adds an `agent` `sent` entry to its conversation: the text, or the key as `↑`, `↓`, `⏎`, `esc`, `tab` | 400 (neither or both, an unknown key), 404 (also a dead pane), 409 | phase 1, 2.1, 2.10 |
| POST | `/agents/{name}/allow` | `{ tool }` | adds `tool` (trimmed, checked like a `tools` entry) to the `tools` list of the agent type of the task named `name`, keeping the rest of its TOML as written, and adds an `agent` `allowed` entry to the conversation. `{ agentType, tool, added }`: `added` is false, with no entry, when the type lists it already. Only the web ask calls it, after the user picks `always allow for <type>`. It applies from the next launch of the type: a running agent keeps the list it started with | 400 (an invalid tool), 404 (no such task or type) | this change |
| GET | `/agents/{name}/output` | | the pane text | 400, 404 (also a dead pane), 409 | phase 1, 2.1 |
| GET (ws) | `/agents/{name}/stream` | | text frames with the pane, `error: ...` then close when it ends | 400, 404, 409 before the upgrade | phase 1, 2.1 |
| GET | `/agents/{name}/steps` | | `Step[]` | 404 | 2.7 |
| GET | `/stats?repo=` | | `{ days: ShippedDay[] }`: last 730 days, days with activity only, summed across repos unless `repo` | 404 repo | 2.7 |
| GET | `/usage` | | `ClaudeUsage`: Claude plan usage, never the token. Always `200`; trouble is `error`, next to the last numbers read | | usage meter |
| GET | `/system` | | `SystemStats { cpuPercent, memoryUsedBytes, memoryTotalBytes, diskUsedBytes, diskTotalBytes }`: whole machine cpu percent (0 to 100, since the previous read, from a sampler kept in state), RAM bytes, and the bytes of the volume that holds `~/.townhall`. Read only | 500 the machine's stats or disks could not be read (logged) | key bar stats |
| GET | `/settings` | | `Settings` | 503 | 2.1 (was 2.0) |
| PUT | `/settings/{key}` | `{ value }` | `Settings` after the change | 400 unknown key or invalid value | 2.1 (was 2.0) |
| GET | `/permissions` | | `{ decisions, suggestions }` of adviser-answered permission prompts (Permission prompts, below) | 503 | |
| POST | `/permissions/suggestions/accept` | `{ agentType, tool }` | `{ agentType, tool, added }`: the tool is saved on the type's `tools` like `POST /agents/{name}/allow` does (`added: false` when it was there already) | 400 a tool that does not validate; 404 unknown agent type | |
| GET | `/profile` | | `{ text }`: the user's profile, `""` when none | 503 | 2.14 |
| PUT | `/profile` | `{ text }` (at most 20 000 characters, counted as Unicode chars; the settings textarea counts UTF-16 units, so it stops a little earlier for emoji) | `{ text }` as stored; replaces the whole profile | 400 too long, not a string, unknown field | 2.14 |
| DELETE | `/profile` | | `{ text: "" }`: resets the profile to empty, also when it was empty | | 2.14 |
| GET | `/conversations` | | `Conversation[]`, newest activity first | | 2.2 |
| POST | `/conversations` | `NewMessage { text, attachments? }`, or `multipart/form-data` with a `text` field and any number of `file` fields (at most 5 files, 20 MB each; 403 foreign origin, 413 over 20 MB), saved as the first message's attachments | `201 ConversationDetail`: created, your message appended, the first turn started | 400 | 2.2 |
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
| POST | `/tasks/{name}/mcp` | JSON-RPC 2.0, see Task agent MCP | JSON-RPC answer, or `202` with no body for notifications | 404 | 2.3 |
| POST | `/conversations/{id}/attachments` | multipart, one field `file` per request | `201 Attachment` | 400, 403 origin, 404, 413 | 2.6 |
| GET | `/conversations/{id}/attachments` | | `Attachment[]` | 404 | 2.6 |
| GET | `/conversations/{id}/attachments/{attachment}` | | the bytes with their `content-type` (not the envelope) | 404 | 2.6 |
| GET | `/tasks/{name}/artifacts` | | `Artifact[]` of the task, oldest first | 404 | 2.9 |
| GET | `/tasks/{name}/artifacts/{id}` | | the bytes with the stored `content-type` (not the envelope); `inline` for a raster image or text, a download otherwise; `nosniff` and `sandbox; default-src 'none'` | 400 bad id, 404 | 2.9 |
| GET | `/library/agents` | | `AgentTypeSummary[]` by name, ignoring case | | 2.12 |
| GET | `/library/agents/usage` | | `AgentUsage[]` = `{ agent, runs, tokens }` per agent type, by name. Every conversation turn that ran claude counts as one run of `default`, with its input plus output tokens (cache tokens left out, like a task's) | | 2.12 |
| GET | `/library/agents/{name}/runs` | | `TaskRun[]` = `{ kind, conversationId, task, repo, state, model, tokens, createdAt, endedAt }`: the tasks of that type (name matched ignoring case), newest first, at most 50. `default` also lists its conversation turns: `kind` is `turn`, `task` is the conversation's title, `repo` its linked repo (`""` when none), `state` is `done`, `failed` or `stopped`, `model` is null, and `conversationId` is the conversation to open. A task has `kind` `task` and the `conversationId` it was made in. `200 []` when there is none, also for a deleted type | | 2.12 |
| GET | `/library/agents/{name}` | | `AgentType` (ignoring case), with `origin` (`user` or `plugin`), `plugin` (the owning plugin id or `null`) and `plugins` (the plugin ids the type opts into). Types of enabled plugins are listed here as read-only `<plugin>-<name>` | 404 | 2.12 |
| PUT | `/library/agents/{name}` | the TOML text, `content-type: text/plain` or `application/toml` | `201 AgentType` new, `200 AgentType` replaced (ignoring case, the new spelling wins) | 400 wrong content type, not UTF-8, the error list (below), its `name` is not the path's; 409 `owned by plugin <id>` for the name of a plugin's type | 2.12 |
| DELETE | `/library/agents/{name}` | | the deleted type's name | 404; 409 it is the last type for tasks, or `default`, or `owned by plugin <id>` | 2.12 |
| GET | `/library/workflows` | | workflow definitions | 501 | 2.8c |
| GET, PUT, DELETE | `/library/workflows/{name}` | PUT: TOML text | | 501 | 2.8c |
| POST | `/library/import` | one agent type's `.toml` as the body, like PUT | `201 AgentType` | 400 like PUT; 403 origin; 409 the name exists (replace it with PUT); 501 a workflow (a `[[step]]` table) | 2.12 |
| POST | `/workflows/{name}/runs` | `{ repo, input }` | the run | | 2.8c |
| GET | `/runs/{id}` | | `Run` = `{ id, workflow, conversationId, repo, baseBranch, state: "running" \| "needsYou" \| "finished" \| "failed", input, steps: RunStep[] }`. `steps` has every step of the run's saved definition, in its order, each as `RunStep` = `{ id, agent, state, round, task, verdict, note }` for the newest round of that step. `state` is the `run_steps` state (`starting`, `started`, `done`, `failed`, `skipped`, `fixing`, `redone`), or `pending` with `round` 0 and null `task`, `verdict` and `note` when the step has not begun. `note` holds why a step was skipped | 404 unknown run; 500 its saved definition cannot be read | 2.8c |
| GET | `/runs?conversation=` | | `Run[]` of that conversation, newest first (`[]` when it has none). The conversation view shows the first one in its session strip and loads it again on every task or thread change in that conversation and every 5 seconds | 400 without `conversation` | 2.8c |

Every route above is registered in `crates/townhall-daemon/src/routes/mod.rs` now. A unit replaces the `501` handlers in its own file and keeps the handler names; it adds extractors (`State`, `Path`, `Query`, `JsonBody`) to the signatures as needed. The upload route already carries `DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)` (20 MB plus framing).

### Agent types (2.12, `routes/library.rs`)

An agent type says how a task's agent runs: 2.8a's agent definition (`definitions::AgentDef`) saved as TOML. Every task names one. The bodies are the TOML text, not JSON, kept exactly as written (comments too). Every save and import runs `Store::check_agent_type`: at most `Store::MAX_AGENT_SOURCE` (10 000) characters, since the prompt goes on claude's command line, then 2.8a's `check_agent`. A type that fails is `400` with the error list as `data` and every error on one line, without `error: `, as `error`:

```json
{ "success": false,
  "data": [{ "kind": "agent", "name": "x", "step": null, "field": "model", "message": "\"-p\" must not start with \"-\"", "line": null }],
  "error": "agent \"x\": model \"-p\" must not start with \"-\"" }
```

`line` is set for TOML parse errors. Import checks the origin (`403`) because a `text/plain` post needs no CORS preflight. Deleting a type that queued tasks name is allowed: those tasks fail when they start (see Scheduler). The last type for tasks cannot be deleted, so a task always has a type to run, and neither can `default`, the conversation agent type (below): both are `409`.

`default` (migration `0806_seed_default`, saved when no type of that name exists) is the agent type townhall's own conversation session runs as. It holds the session's tools and its personality prompt (the rules that were hardcoded in `runner/prompt.rs`), and `model = "opus"`: the type is the only place the conversation's model is set (migration `0807_default_model` adds that line to an existing `default` that has none). A blank `model`, or a type that is missing or unusable, runs on the fallback `opus` (`FALLBACK_MODEL` in `runner/persona.rs`). Edit it like any type (`PUT /library/agents/default`). On every turn `runner/persona.rs` reads it: a `model` and `prompt` that are not blank, a non-empty `tools` list and the `permission_mode` apply; the mode is the type's own, `default` when it sets none, and `bypassPermissions` reads as `default` because the planner never skips permissions (`AgentDef::conversation_mode`), and entries that write or run things (`Bash`, `Edit`, `MultiEdit`, `Write`, `NotebookEdit`) are dropped from `tools`. Townhall still appends the repo of the turn, the registered repos and the profile after the prompt. A type that is missing, no longer parses or uses the `shell` adapter leaves the built-in model, mode, tools and prompt in place. It is never the type tasks default to (`create_task`, `list_agent_types.default`), is left out of the types a task may hand back to and out of the unknown-type error of `create_task`, and the agents page labels it the conversation default.

The seed: `0801_seed_builder` saves `builder` once, on the first open with no agent types; it is never saved again, also not after you delete it:

```toml
# the agent type tasks run as when they name none
name = "builder"
description = "implements a task, tests first"
adapter = "claude"
model = "sonnet"
effort = "medium"
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

## Marketplaces (plugins 1.3, `routes/marketplaces.rs`, `plugins::market`)

Design: `docs/plugins.md` section 8. `Marketplace { name, source, account: string | null, enabled, official, headSha: string | null, fetchedAt: string | null, error: string | null, plugins: { name, description, version, category }[] }`, the `marketplaces` table plus the plugins its index lists. `source` is `{ source: "github", repo }` (only the built in one), `{ source: "url", url }` or `{ source: "directory", path }`.

- `source` of `POST`: a GitHub or GitLab link, any git url, `owner/repo` (GitHub shorthand), or an absolute folder (`~` allowed), told apart like `repos::source`. A url is cloned into `~/.townhall/plugins/marketplaces/<name>` (checked in a temporary folder first, so a refused add leaves nothing), with `repos::auth_env` and no prompts. A folder is read in place and never copied, synced or deleted; its `headSha` is `HEAD` when it is a git repo, else `null`. The index is `.townhall-plugin/marketplace.json`, else `.claude-plugin/marketplace.json`, parsed by `plugins::marketplace` (a symlink or a file over 1 MB is refused). `name` defaults to the index's `name`; both are checked as repo names.
- `townhall-official` is compiled in (`plugins::OFFICIAL`: name, `WilliamThogersen/townhall-plugins`, the sha of the index snapshot) with an `include_str!` snapshot of its index, so `GET` lists its plugins with no network and no `git`. It is saved as a row on first use (`official: true`, `headSha` the pinned sha, `fetchedAt: null`). Its name is reserved, ignoring case, whether given as `name` or by an index. Once `refresh` has cloned it, its listed plugins come from the clone.
- `refresh` of a clone: `git fetch origin HEAD`, the new commit's index is read and must parse, then the clone is checked out at it (a force pushed history is accepted). A missing clone is cloned again. A folder is read again. A failure leaves the last state and sets `error`; the next success clears it.
- Accounts (plugins 8.3): for an `https` remote on a GitHub or GitLab host, the login is `account` of the request (stored on the row), else the first account of the `forgeAccounts` setting for that forge and host, else none. Its token comes from `gh auth token --user <login> --hostname <host>` or `glab config get token --host <host>` when each `git` child is spawned, and goes to that child only as `GIT_CONFIG_COUNT=1`, `GIT_CONFIG_KEY_0=http.extraheader` and `GIT_CONFIG_VALUE_0=Authorization: Basic <base64(x-access-token:<token>)>` (`PRIVATE-TOKEN: <token>` on GitLab). Never in argv, a file or a log. `ssh`, `http` and unknown hosts get no token; the old `githubAccount` setting is not used. `account` names a login that is not registered for the host: `400`.
- `TOWNHALL_GIT_BIN` names the `git` program these routes run (default `git`), the way `TOWNHALL_GH_BIN` does for `gh`; tests point it at a fake.

## Plugins (plugins 1.4, `routes/plugins.rs`, `plugins::install`, `plugins::tree`)

Design: `docs/plugins.md` sections 2.3, 8.4, 8.5, 10.1. `{id}` is `name@marketplace`, ignoring case.

- `Plugin { id, marketplace, name, version: string | null, sha, enabled, pinned, permissions: string[], permissionsHash, installedAt, updatedAt, error: string | null }`: a row of `plugins`. `permissions` is what the user approved (`plugins.approved`), `permissionsHash` its hash.
- `AvailablePlugin { id, marketplace, name, description, version, category, installed }`: every entry of every enabled marketplace's index (the built in one included); a marketplace whose index cannot be read adds nothing here and shows its `error` on `GET /marketplaces`.
- `PluginPreview { plugin, marketplace, name, version, sha, permissions, permissionsHash }`.
- `PluginUpdate { outcome: "unchanged" | "updated" | "pinned" | "needsApproval", sha: string | null, added: string[], permissionsHash: string | null, plugin: Plugin }`. `sha`, `added` and `permissionsHash` describe the new tree (`null`, empty and `null` for `unchanged` and `pinned`).
- The permission list is one sorted list of strings built from the manifest and checked against the files: `api:<scope>`, `hook:<event>`, `prompt:<kind>`, `env:<NAME>`, `ui:<slot>`, `claude.api:<scope>`, `claude.hooks:<Event>`, `claude.hooks.agent-type:<type>`, `claude.mcp:<server>:<tool>` (`*` for all tools), `claude.mcp.read-only:<server>:<tool>`, `claude.planner`, and what runs or connects: `runs.hook:<event>:<command>`, `runs.mcp:<server>:<command and args>`, `connects.mcp:<server>:<url>`. The hash is `sha256:` and the SHA-256 of the items joined by `\n`, so the order of declaration does not matter. A plain Claude Code plugin (no `.townhall-plugin/plugin.json`) gets every hook event and MCP server found in `hooks/hooks.json` and `.mcp.json` (the latter with `*`), on top of the entry's `townhall.permissions` when it has them.
- Source to sha: a `./path` entry is the marketplace's own `HEAD` (the folder is cloned from the marketplace's clone or folder, which has to be a git repo); `github`, `url` and `git-subdir` entries use the entry's `sha`, else its `ref` (a tag, else a branch), else the remote's default branch. The tree is cloned with `git -c core.hooksPath=/dev/null clone --no-recurse-submodules --no-checkout` over `repos::remote_git` (the marketplace's `account`, else the first registered account for the host, token only through `GIT_CONFIG_*` env), checked out detached, and refused with `400` when it holds a git submodule. Every later `git` call of install also runs with `core.hooksPath=/dev/null` and without the user's global and system git config. Nothing from the plugin runs: no script, no build, no git hook.
- Validation (`plugins::tree`): the manifest (64 KB), every contributed file (exists, a regular file, at most 1 MB, resolves inside the plugin), the whole tree (at most 50 MB, any symlink that leads outside or nowhere is refused), `apiVersion` not above the daemon's (`needs plugin api 2, this townhall serves 1`), and the declaration against use: a hook file with an event, or an `.mcp.json` with a server, that `permissions` does not list is a `400`.
- `POST /plugins` clones into `~/.townhall/plugins/cache/<marketplace>/<plugin>/<sha>/` (no `.git`), creates `~/.townhall/plugins/data/<marketplace>/<plugin>/`, and stores the row with `enabled: false`. A refused install leaves no tree and no row. `preview` does the same work in a staging folder and removes it.
- `PATCH` changes only the keys sent. Enabling starts nothing.
- `update` re-resolves the entry's source with a fresh clone and compares. A pinned plugin (`pinned: true`) is `pinned` and nothing is fetched. The same sha and the same list is `unchanged` (a pending `error` is cleared). Else the new tree is built beside the old one, and when it adds nothing to the approved list (the same list, or less) the plugin switches to it (`updated`, `enabled` and `pinned` stay, the old tree is removed). When it adds something and there is no `approve`, the plugin stays on the old sha and keeps running, `error` becomes `update to <sha> needs approval` and the answer is `needsApproval` with the added items and the new hash; calling again with `approve=<that hash>` installs it. An `approve` that is not the hash of the new list is a `400` whatever the list is.
- `DELETE` disables, removes the whole `cache/<marketplace>/<plugin>` tree, and removes `data/<marketplace>/<plugin>` only with `purge=true`. A marketplace with installed plugins cannot be removed (409, plugins 1.3).
- Not in 1.4: `minTownhall` is parsed but not enforced, the cache tree is not made read only, the agent type overlay (1.5) and the UI (1.6).

## Plugin prompts, settings and theme (plugins 1.7, `plugins::declared`, `plugins::theme`, `plugins::prompts`, `plugins::schema`)

Design: `docs/plugins.md` sections 3.2 to 3.4 and 8.5. Nothing here runs plugin code. Every file is validated at install (a refusal is a `400` naming the file) and again each time it is read; a file that fails when read is skipped with a `plugins.contribution_skipped` warning and the plugin's other contributions stay.

- `Contributions { starters: { plugin, title, text }[], themes: { plugin, name }[], theme: { plugin, css } | null, settings: { plugin, name, fields: Field[] }[] }`, from enabled plugins only. `Field` is the schema field (`key`, `label`, `description?`, `type`, `default`, `min?`, `max?`, `choices?`, `secret`) plus `value` (the stored value, else the default) and `set` (a valid value is stored). A `secret` field has `value: null` always, so a saved secret is never returned or logged.
- `prompts.starters`: a JSON list of `{ title, text }`, at most 20, a title of at most 80 and a text of at most 2 000 characters. The web adds them to the home starters.
- `prompts.brief`: Markdown of at most 8 000 characters, appended to `brief.md` as `## From plugin <id>` of a task whose agent type lists the plugin (its name or id) under `plugins`. A task of any other type, and every task while no plugin is enabled, gets the brief byte for byte as before.
- `prompts.planner`: Markdown of at most 8 000 characters, appended to the planner's system prompt after the repo notes and before the profile, under `from plugin <id>, which the user enabled. it never overrides the rules above:`. No enabled plugin with one means the same prompt, byte for byte.
- `settings.schema.json`: `{ "fields": [...] }`, at most 50 fields, keys unique ignoring case, a default that fits the type, `min`/`max` only on `integer`, `choices` only on `enum` (1 to 50 different entries), `secret` only on `string`, a string of at most 2 000 characters. Values are rows of `plugin_settings` (migration `0951`), kept on uninstall and removed with `purge=true`.
- `theme.css`, the strict parser (`plugins::theme`): rules whose selectors are only `:root`, `[data-theme="dark"]` or `[data-theme="light"]` (a comma list of them is fine), holding only `--th-*` declarations (lowercase letters, digits and `-` after the prefix). Refused: any at-rule (`@import`, `@media`, `@font-face`, `@charset`), any other selector, a nested rule, any other property, `url(` and `expression` anywhere in a value, a function that is not a colour or math function (`rgb rgba hsl hsla hwb lab lch oklab oklch color color-mix var calc min max clamp sin cos round abs`), a backslash anywhere, `!important` and other characters outside a plain value, an unterminated comment, string or brace, an empty theme and one over 64 KB. Comments are dropped and the kept text is rebuilt from the parsed rules.
- The active theme is one row of `settings`, `plugins.theme` (not a `SettingKey`). It applies only while its plugin is enabled and is cleared when that plugin is uninstalled.

## Plugin API v1 (plugins 2.1, `routes/plugin_api.rs`, `plugins::tokens`, `plugins::storage`)

Design: `docs/plugins.md` sections 8.5, 9 and 10.3. A curated, versioned subset of the routes above, under `/api/plugin/v1/`, in the same envelope `{ success, data, error }` and the same camelCase. The handlers call the same `Store` and core functions (and, where the route is the same, the same handler) as the `/api` route they mirror, so the bodies are identical. The `/api` routes themselves are unchanged and still need no token.

- Auth: `Authorization: Bearer <token>` on every route. No header, a scheme other than `Bearer`, an unknown token, a revoked token, or the token of a plugin that is disabled or uninstalled is `401`. A token without the scope the route needs is `403` (`this token has no \`<scope>\` scope`). The `401` comes first. A browser origin sends no token and CORS does not allow the `Authorization` header, so a page of another origin cannot use a plugin's scopes.
- Tokens: 32 bytes from `/dev/urandom`, written as 64 hex characters. Only `sha256(token)` is stored, in `plugin_tokens (token_hash pk, plugin_id, task, created_at, revoked_at)` (migration `0952`); the clear text is returned once by `plugins::tokens::issue` and never read back. A plugin token (`task` null) is created when `PATCH /api/plugins/{id}` turns `enabled` from false to true. Disabling and uninstalling revoke every token of the plugin, task tokens too. A task token (`issue(store, plugin, Some(task))`, `revoke_task`) is supported by the store; the scheduler does not create them until phase 3.
- Scopes of a caller are the `api:<scope>` items of the plugin's approved permission list. A task token keeps at most `tasks:read`, `conversations:read` and `storage` of them (plus `tasks:create` and `conversations:write` only through `permissions.claude.api`, below), and sees only its own task and the conversation of that task: another task or conversation is `403`, lists hold only its own rows, and the routes that span everything (`/stats`, `/search`) are `403`. `settings:read` is never given to a task token.

| Method | Path | Scope | Answer | Errors |
|--------|------|-------|--------|--------|
| GET | `/info` | any | `{ apiVersion: 1, townhall, plugin, scopes }`: the version of the daemon, the plugin id and the scopes this token has | 401 |
| GET | `/tasks` | `tasks:read` | as `GET /api/tasks`, with the same `conversation`, `repo` and `state` filters | 400 as there; 401; 403 |
| GET | `/tasks/{name}` | `tasks:read` | as `GET /api/tasks/{name}` | 401; 403 another task for a task token; 404 |
| GET | `/tasks/{name}/artifacts` | `tasks:read` | `Artifact[]` as `GET /api/tasks/{name}/artifacts` (no download route) | 401; 403; 404 |
| GET | `/agents` | `tasks:read` | `AgentInfo[]` as `GET /api/agents`; a task token gets the agent named like its task | 401; 403 |
| GET | `/stats` | `tasks:read` | `{ days }` as `GET /api/stats` | 401; 403 a task token |
| GET | `/conversations` | `conversations:read` | `Conversation[]` as `GET /api/conversations`; a task token gets its own | 401; 403 |
| GET | `/conversations/{id}` | `conversations:read` | `ConversationDetail` as `GET /api/conversations/{id}` | 401; 403 another conversation for a task token; 404 |
| GET | `/search?q=&limit=` | `conversations:read` | `SearchHit[]` as `GET /api/search` | 400 as there; 401; 403 a task token |
| GET | `/settings` | `settings:read` | townhall's own settings only: `agentsAtOnce`, `repoUnclear`, `sessionPanelOpen`, `notifyWhen`, `minFreeDiskGb`, `learnAboutMe`, `reflectAfterTasks`. Never `githubAccount`, `forgeAccounts` or `clonesDir`, never the profile | 401; 403 |
| GET | `/storage/{key}` | `storage` | `{ key, value }` of the plugin's own row | 400 bad key; 401; 403; 404 |
| PUT | `/storage/{key}` | `storage` | body `{ value: string }`; answers `{ key }` | 400 bad key or body; 401; 403; 413 value over 64 KB, or the plugin's rows would pass 5 MB in all |
| DELETE | `/storage/{key}` | `storage` | `{ key }` | 400; 401; 403; 404 no such row |

- Storage: rows of `plugin_storage (plugin_id, key, value, updated_at)` (migration `0952`), keyed by the plugin of the token, so a plugin never sees another plugin's rows and a task token shares its plugin's. A key is 1 to 128 characters of letters, digits, `.`, `_` and `-` (not `.` or `..`). A value is text of at most 65 536 bytes; a plugin's values total at most 5 242 880 bytes, and replacing a value counts only the new one. The rows are kept on uninstall and removed with `purge=true`.
- Versioning: the version is in the path. New routes and optional fields stay in `v1`.
- Not in 2.1 or 2.2: `profile:read`, `events` and the hooks (2.3). The plugin token's clear text is not handed to anything yet: delivery to a hook or MCP server environment arrives with them.

### Plugin API v1, writes and the library (plugins 2.2)

| Method | Path | Scope | Body | Answer | Errors |
|--------|------|-------|------|--------|--------|
| POST | `/conversations/{id}/tasks` | `tasks:create` | the `create_task` input, camelCase: `name`, `title`, `prompt`, `repo`, `baseBranch`, `after`, `attachments`, `agent`, `prBase`, `doneWhen`, `check`, `maxRetries` | `201` the `Task` | 400 the planner's checks (name, repo, base branch, agent type, `after`, `check`, `requires`, a taken name) or an unknown field; 401; 403; 404 conversation |
| POST | `/conversations/{id}/messages` | `conversations:write` | `{ text }` | `200` the new `plugin` `ThreadEntry` | 400 blank or over 100 000 characters, or an unknown field; 401; 403; 404; 409 a turn is running |
| GET | `/library/agents` | `library:read` | | `AgentTypeSummary[]` as `GET /api/library/agents` | 401; 403 |
| GET | `/library/agents/{name}` | `library:read` | | `AgentType` as `GET /api/library/agents/{name}` | 401; 403; 404 |
| PUT | `/library/agents/{name}` | `library:write` | `{ conversation, source, confirmed? }`, `source` the whole type as TOML | `200` `{ status: "asked", name, replaces, next }` or `{ status: "saved", name, replaced }` | 400 the TOML's errors, a `name` that is not the path's, a `confirmed` save before the user answered, or a different TOML than the one asked about; 401; 403; 404 conversation |

- `POST .../tasks` is `create_task` (`mcp::create_task_as_plugin` runs the planner's own code, so the checks cannot drift). No route of the plugin API takes a permission mode: `permissionMode` and `permission_mode` are unknown fields (`400`), and the task gets the saved setting narrowed by its agent type when the scheduler starts it. After the task is created a `plugin` entry `created by plugin <id>: task \`<name>\`` is written next to the `tasksCreated` entry, so the thread shows who made it.
- `POST .../messages` writes a `plugin` entry (author `plugin <id>`, never `you`, so a plugin cannot post as the user) and starts a planner turn like a message does. The planner reads `message from plugin <id> (automatic, the user did not write this):` before the text, as it does for task events, and repo inference is skipped as for an event. A `plugin` entry does not answer a `repoAsk` or an agent type ask.
- `PUT /library/agents/{name}` is `save_agent_type` (`mcp::save_as_plugin`). The ask needs a conversation to show the TOML in, so the body names one. Without `confirmed` townhall writes the TOML and the question into that conversation as a `townhall` entry and answers `asked`; nothing is saved. With `confirmed: true` it saves only that exact TOML, and only after a `you` entry was written after the ask; a `plugin` entry never counts, so only the user can say yes. Saved types are the same as the planner's, whatever they set. `DELETE` of a type is not part of the API.
- A task token never has `tasks:create` or `conversations:write` from `permissions.api`. It has them only when the manifest lists them under `permissions.claude.api` (the approved item `claude.api:<scope>`), and then only for the conversation of its task (another is `403`). It never has `library:write`.

## Types

All in `townhall_core::model`, mirrored in `web/src/api/types.ts`.

- `Repo { name, path, baseBranch, githubAccount: string | null, gitlabAccount: string | null }`. `githubAccount` and `gitlabAccount` (migration 0004, `repos.github_account` and `repos.gitlab_account`) are the login of the registered account the repo's tasks use on that forge; `null` means the first registered account for the origin's host, and for GitHub, with none registered for it, the old `githubAccount` setting (see `open_pr`). `name`: `[A-Za-z0-9._-]`, starts with a letter or digit, at most 64, unique ignoring case, the folder name by default. `path`: canonical, the top folder of a git work tree, never one of townhall's own worktrees. `baseBranch`: origin's HEAD branch (the local branch when it exists, else `origin/x`), else the current branch, else `HEAD`.
- `AgentInfo { name, repo, kind, branch, worktree, alive, state }`: phase 1's `Agent` plus `repo` and `state`. `AgentState`: `{ kind: "working" }`, `{ kind: "waiting", question }`, `{ kind: "error", exitCode: number | null }`, `{ kind: "ended" }`.
- `AgentKey`: `up`, `down`, `enter`, `escape`, `tab`, the `key` of `POST /agents/{name}/send` (`AgentKey::symbol` is how a `sent` entry shows it).
- `Settings { agentsAtOnce, repoUnclear, sessionPanelOpen, notifyWhen, minFreeDiskGb, learnAboutMe, githubAccount, forgeAccounts, reflectAfterTasks, clonesDir, adviserPermissions, permissionTimeoutSecs }`, the same names, values and defaults as `web/src/settings/settings.ts` (a type test checks it). `githubAccount` and `reflectAfterTasks` have no row on the settings page yet; `forgeAccounts` has the forge accounts group.
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
| `agent` | `agent, event: "sent" \| "passed" \| "failed" \| "ended" \| "allowed", text` | `→` `✓` `×` `○` `+` lines. `allowed` has the text `<tool> to <agent type>` |
| `event` | `text` | `◇` line: a task event townhall's own session was woken with, not a message from you (see below) |
| `handoff` | `task, from, to, reason` | `↩ <task>: <from> handed back to <to>, <reason>` |
| `plugin` | `plugin, text` | `◇ plugin <id>: <text>`: a message or a note from an enabled plugin through the plugin API (`plugin` is the plugin id, `name@marketplace`). Never shown as you, and never counts as your answer to an ask |
| `error` | `message` | townhall's own failure |

- `RepoRule`: `named`, `code`, `only` (2.4's `repo_infer::Rule`, same names), `picked` (an answer to a `repoAsk`), `linked` (the row menu).
- `Task { name, title, prompt, repo, baseBranch, after: string[], state, conversationId, attachments: string[], createdAt, pr: { number, url: string | null, checks: "pending" | "passed" | "failed" | "none" } | null, agent, blocked: string | null, prBase: string | null, doneWhen: string | null, check: string | null, maxRetries: number, verify: { phase: "verifying" | "retrying" | "passed" | "failed", retries: number, output: string | null } | null }`. `doneWhen`, `check`, `maxRetries` and `verify` are the verification loop (Verification below); a task created without them reads `null`, `null`, `2`, `null`, and one stored before migration 0206 reads the same. `prBase` is the branch the task's pull request targets when that is not its `baseBranch` (`null` when unset: `open_pr` then targets `baseBranch`; `Task::pr_target()` is the one that is used). `blocked` is why a queued task whose `after` tasks finished does not start (Required steps under Scheduler), else `null`. `agent` (2.12) is the stored name of the agent type it runs as, resolved when it was created (`builder` for tasks from before 2.12). `types.ts` has it optional until a web unit reads it (`parseTask` drops it for now). `name` is the agent name and the branch `townhall/<name>` (`Task::branch()`), unique across every conversation and repo ignoring case. `TaskState`: `queued`, `working`, `needsYou`, `inCi`, `inReview`, `merged`, `ended`, `error`.
- `Attachment { id, conversationId, name, mime, size, kind: "image" | "file", createdAt }`. `kind` is `image` for `image/*`. At most 20 MB (`MAX_ATTACHMENT_BYTES`).
- `TaskRun { task, repo, state, model: string | null, tokens, createdAt, endedAt: string | null }` and `AgentUsage { agent, runs, tokens }`: per-task numbers for comparing cheap and expensive runs of an agent type (the Agents page detail view lists the runs). Whenever a claude task's agent changes state, townhall reads the task's newest claude transcript and stores `tokens` = input + output tokens (cache tokens excluded) and `model` = the last model the transcript reports (`<synthetic>` and empty names are skipped), so it stays right when the type's `model` is edited later. `model` is `null` when unknown: tasks from before migration `0805_task_model`, `shell` tasks (no transcript), and agents that have not changed state yet. Cost here means tokens: there is no dollar cost, because townhall has no price table.
- `Step { id, tool, summary, startedAt, endedAt: string | null, result: "running" | "ok" | "error" }` and `ShippedDay { date: "YYYY-MM-DD", fixes, features, lines, agents }`: the Rust types belong to 2.7 (`steps/`, `stats/`); `types.ts` already mirrors them.
- `AgentType { name, description, adapter: "claude" | "shell", model, permissionMode: "default" | "acceptEdits" | "plan" | "bypassPermissions" | null, tools: string[], prompt, command, canHandBackTo: string[] | null, requires: string[], env: { [name: string]: string }, source, updatedAt }` (2.12): the parsed `AgentDef` plus its TOML as written. `env` is the optional TOML table `[env]` (`NAME = "value"`, put it after the top-level keys): environment variables the launched process gets on top of the daemon's, `{}` when the table is left out, so types saved before it existed load unchanged. A name that is empty or holds `=` is a definition error on `env` that names the variable, never its value. Townhall gives no name a meaning and hardcodes none; values are never logged, and a failed start shows each as `NAME=<hidden>`. Auto-compaction can be tuned this way, for example with `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` (the name is present in the installed claude binary; townhall does not interpret it): set it under `[env]` in the type's TOML and every task of that type runs with it. The agents detail view lists the names, not the values (the TOML under it is shown as saved). `AgentTypeSummary { name, description, adapter, model, permissionMode, updatedAt }`: a row of the list. `canHandBackTo` is the TOML key `can_hand_back_to`: the agent types this type may hand a task back to; `null` (the key is left out) is any type, `[]` is none. Names are checked like type names, with no repeats ignoring case. `requires` is the TOML key `requires = ["tester", "reviewer"]`: the agent types a task of this type needs in its `after` chain (see Required steps under Scheduler); `[]` (the default, or the key left out) requires nothing, and its names are checked like `canHandBackTo`'s. Only `tester` and `reviewer` can be required (they are the types with verdict words): any other name is a definition error on `requires`, ``error: agent "x": requires: `coder` cannot be required: it has no verdict words, only tester and reviewer can be``, and a stored type that has one anyway is not loaded. `DefinitionError { kind: "agent" | "workflow", name, step, field, message, line }`: 2.8a's error.
- `ClaudeUsage { session, weekly: UsageWindow | null, updatedAt, error: string | null }` and `UsageWindow { usedPercent: 0..100, resetsAt: unix ms | null, ratePerHour: points per hour | null }`: `session` is the five hour window, `weekly` the seven day one. The daemon reads Claude Code's OAuth token (macOS Keychain, else `~/.claude/.credentials.json`), calls `api.anthropic.com/api/oauth/usage` through `curl` and polls every 5 minutes, 15 after a 429. Polling starts with the first `GET /usage`. A result under a minute old is reused. On a failure `error` is set and the windows and `updatedAt` stay at the last good reading, which is also kept in `<home>/usage.json` and shown after a restart with `error: "Not refreshed yet"`.
- `CreateTaskInput`, `NewTask`, `NewMessage`, `ConversationPatch`, `AddRepo`, `PutSetting`: request bodies, see the routes and MCP. `NewTask` carries the resolved `agent`.
- `SearchHit { conversationId, title, match: "title" | "message", entryId: number | null, snippet: string | null, updatedAt }` (`townhall_core::search`, mirrored in `web/src/api/search.ts`): one per conversation whose title or one of whose `you` and `townhall` entries holds `q`, ignoring case (every letter, not only ASCII) and runs of whitespace; `%` and `_` are plain characters. A title match wins and has `entryId` and `snippet` `null`; else `entryId` is the newest matching entry and `snippet` that entry on one line, about 80 characters around the first match, cut between words where it can, with `…` where it was cut. `updatedAt` is the conversation's.

## Attachments

Unit 2.6, `routes/attachments/` and `runner/content.rs`.

- Upload: the origin check comes first (`403`), then the conversation (`404`), then the body: exactly one multipart field `file` holding a non-empty file (`400` otherwise), counted while it streams and stopped past 20 MB (`413`, nothing kept).
- `mime`: png, jpeg, gif and webp come from the file's first bytes; anything else from the part's content type, then the extension. An image type the bytes do not back up (an svg, text sent as `image/png`) is never kept, so `kind: "image"` is always a real png, jpeg, gif or webp.
- `name`: the last path part without control or invisible (bidi) characters and without edge dots or spaces, at most 120 bytes with a 1 to 10 letter extension kept, `file` when nothing is left. An image gets the extension of its type (`shot` becomes `shot.png`), so an agent's Read tool shows it as an image.
- Download: the bytes with `content-type`, `content-length`, `content-disposition` (`inline` for those images, `attachment` for the rest, an ASCII `filename` plus `filename*`), `x-content-type-options: nosniff` and `content-security-policy: sandbox`. Only a regular file directly in its attachment folder is served; a symlink or a missing file is `404`.
- Townhall's session: up to 10 images per message, each at most 5 MB of base64, go in as `{"type":"image","source":{"type":"base64","media_type":...,"data":...}}` blocks before the text block. The text ends with `attachments of this message (pass their ids to create_task):` and one line per attachment, `- <id> (<name>, <mime>, ...)`, ending in `image n above` for a sent image, `file at <path>` (the stored file, which the session can read) for any other file, or the reason an image was only listed.
- Tasks: `Store::attachment` gives the path of the bytes, absolute whenever the home is, for 2.3 to append to an agent's prompt.

## Settings keys

One row per key in the `settings` table, the value as JSON. `PUT /api/settings/{key}` with `{ "value": ... }` is the only way to change one. Permissions are not settings: each agent type sets its own `permission_mode`. A row left over from the removed `permissionMode` and `workWithoutAsking` settings is ignored when the settings are read. The browser-settings import (a separate web unit) sends each key the settings screen stored in `localStorage` (`th-settings`) once, then clears it.

| key | values | default |
|-----|--------|---------|
| `agentsAtOnce` | integer 1 to 50 | `6` |
| `repoUnclear` | `ask`, `guess`: what townhall does when a message points at several repos (names two, or its code fits two equally): ask, or run in the first candidate. A message that points at no repo is not unclear: it keeps the conversation's repo, else runs with no repo (the session passes `repo` to `create_task`), and nothing is asked or noted | `ask` |
| `sessionPanelOpen` | boolean | `true` |
| `notifyWhen` | `needsYou`, `failed`, `everything` | `needsYou` |
| `minFreeDiskGb` | integer 0 to 100000: a task does not start with less free space on the data volume; `0` is off | `5` |
| `learnAboutMe` | boolean: while on, the profile text (below) is added to townhall's own session prompt. Nothing learns into the profile automatically yet | `false` |
| `githubAccount` | the old single account: a GitHub login, or empty for none. Still read, and still the account `open_pr` uses for a GitHub repo that has no registered account of its own (see `forgeAccounts`) | `WilliamThogersen` on a new database (migration 0003), empty when the row is missing or invalid |
| `forgeAccounts` | a list of at most 50 `ForgeAccount { forge: "github" \| "gitlab", login, host, label }`. A value may leave out `host` (`github.com` or `gitlab.com` by forge) and `label` (the login); the stored and returned form has all four, `host` lowercase. A login is `[A-Za-z0-9._-]` (GitHub: at most 39, no `.` or `_`), a host a name with an optional port, no scheme. The same forge, host and login (ignoring case) twice is a 400. No tokens are stored: they are fetched when a child is spawned | `[]` |
| `clonesDir` | string: the folder repos added by link are cloned into, `~` or an absolute path; empty means `~/.townhall/repos`. Only clones in `~/.townhall/repos` can be deleted with their registration | `""` |
| `reflectAfterTasks` | boolean: while on, a task that ended is handed on once to the `reflector` agent type, which records lessons in the repo's memory files (Reflect step, below) | `false` |

| `adviserPermissions` | boolean: while on, a Claude permission prompt of a task's agent is put to the adviser instead of you (Permission prompts, below) | `true` |
| `permissionTimeoutSecs` | integer 5 to 600: how long the adviser has for one prompt before it comes to you | `60` |

A missing or invalid stored value reads as its default. The store also keeps `desktop.repo` (the desktop app's chosen repo, a path); it is not a setting and not in the API.

## Permission prompts (`permissions/`, `agent/watch/permission.rs`)

With `adviserPermissions` on, the watcher does not turn a task agent's Claude permission prompt (`Do you want ...?` with a `Yes` and a `No` option) into a needs-you entry at once. It runs `permissions::decide` in the background and holds the prompt for at most `permissionTimeoutSecs`:

1. The command comes from the prompt and from the agent's screen. A prompt whose command cannot be read comes to you.
2. The hard deny list, `permissions/deny.rs`, is checked first and the adviser is never asked: `rm` or deletes outside the worktree, a force push (`--force`, `-f`, `--force-with-lease`, `+ref`, `--mirror`) to `main`, `master` or the task's base branch, anything naming secrets, credentials, `~/.ssh`, keychains or token variables, dropping or deleting databases and data, and any path outside the task's worktree. These come to you as needs-you, with the reason.
3. Otherwise the adviser type is asked, read-only, with the command, the agent type (prompt, tools, permission mode), the task brief, the worktree and the last 6 steps. Its first line is `ALLOW: why`, `DENY: one line for the agent` or `ASK USER: why`; anything else is ask-user.
4. Allow presses the `Yes` option (`1`, never `don't ask again`). Deny presses the `No` option, then types the reason as one line and Enter. Ask-user, a timeout, an adviser that fails, or an agent type in `plan` mode (it is never granted a command) leave the prompt as a needs-you entry whose `advice` is the reasoning. A prompt that went away meanwhile is left alone.
5. Every decision is a row of `permission_decisions` (migration `1100`): task, agent type, command, pattern, verdict (`allow`, `deny`, `ask-user`, `hard-deny`, `timeout`) and reason.

`GET /permissions` answers `{ decisions: PermissionDecision[], suggestions: PermissionSuggestion[] }`: the last 50 decisions, newest first, and each `{ agentType, tool, approvals }` whose pattern the adviser allowed at least 3 times for that agent type and that the type's `tools` do not list yet. A suggestion is never applied by itself: the settings page offers it, and saving it is the normal agent type save. A `needsYou` entry has an optional `advice` string, absent when there is none.

## Reflect step

Off by default (`reflectAfterTasks`, `scheduler/reflect.rs`). While it is on, each scheduler pass hands every `ended` task on to the agent type `reflector` once: the same hand back an agent makes (`Store::hand_on`, a `handoff` entry in the thread, counted in `MAX_HANDOFFS`), so the task is queued again as `reflector` in its own worktree and branch, and its brief says what the task was and how to see its changes (`git diff <start commit>..HEAD`, plus its commits). The reflector reads that and adds at most five short, deduplicated bullets under a `Lessons` heading in `CLAUDE.md` (or in `docs/lessons.md`, linked from `CLAUDE.md`, when `CLAUDE.md` is missing or long) and commits them on the task's branch. It never pushes.

- The shipped type is `REFLECTOR_SOURCE`: model `haiku`, permission mode `default`, no hand backs, and tools that name only `Edit` and `Write` for those two files, read-only git commands, and `git add` of those two files and `git commit -m`. Townhall saves it when the step first needs it and no `reflector` is saved; a saved one, edited or not, is never replaced (delete it while the setting is on and it comes back).
- A task is reflected on once: never when its type is `reflector`, never when any earlier hand back went to `reflector`, never after `MAX_HANDOFFS` hand backs, and only while its worktree still exists (the reaper keeps an ended task's agent for a grace period first).
- Agents run with `--setting-sources=`, so claude does not load a repo's `CLAUDE.md` for them by itself. A task's brief has to tell its agent to read it for the lessons to reach it.

## Disk use

Every task runs in its own worktree, and a Rust repo builds a target folder of several GB in each. That `target/` goes with the worktree when its agent is retired (`git worktree remove --force` in `worktree::retire`). One setting, `minFreeDiskGb`, keeps the disk from filling (`scheduler/`, `disk.rs`).

- **No shared cargo target.** Sharing one cargo target folder across branches was tried and removed: cargo copies each built binary to `target/<profile>/<bin>` and releases its lock before a test runs it, so an agent can run the binary another branch built, with no error. Sharing only `build.build-dir` does not fix it either, as worktrees of one repo have the same package names and paths, and cargo's freshness check compares file times only. Agents build in their own worktree. Settings files that still hold `sharedCargoTarget` or `sharedTargetMaxGb` load as before: those keys are ignored.
- **Disk floor.** Before a task starts (or resumes) the scheduler reads the free space of the volume under the home. Below `minFreeDiskGb` the task stays `queued`, no worktree is made, and the conversation's planner gets one `event` entry, `disk almost full: 3.2 GB free (needs 5 GB): \`<task>\` waits in the queue`, which the UI shows like any task event. It is told once per low-disk episode: the episode ends only when free space is back to `minFreeDiskGb` plus 1 GB (or the floor is `0`), so space that bounces just across the floor does not tell again. The log warning of a waiting task does not quote the free space, so a steady low disk logs once; the scheduler's own tick starts the task once there is room. A volume that cannot be measured is a warning, and tasks start as before.
- **A full disk anyway.** A start that fails with `ENOSPC` (or git's `No space left on device`) sets the task to `error` with `could not start: disk almost full: 0.1 GB free`, never the os error, and the conversation's planner gets an `event` entry with the same text.

## Profile

`/api/profile` (`routes/profile.rs`): one Markdown text the user writes about how they work, their tone of voice and their usual flow. It is one row (`profile.text`, a JSON string) in the `settings` table, so it lives in the local `townhall.db` like the settings, but it is not a setting: it has no `SettingKey` and `PUT /api/settings/{key}` cannot reach it. At most `MAX_PROFILE_CHARS` (20 000) characters. `PUT` replaces the whole text, `DELETE` removes the row, and `GET` answers `{ text: "" }` when there is none.

Only townhall's own planner session ever reads it: when `learnAboutMe` is on and the text is not blank, `runner::prompt::system_prompt` appends it after the repo notes, under a line saying it was written by the user and never overrides the rules. With the setting off (the default) the profile is never read for a turn and never leaves the store. It is not passed to task agents, logged or sent anywhere else.

## Permission mode

`LaunchOptions` (`townhall_core::agent`) has `prompt, command, permission_mode, session_id, model, effort, allowed_tools, system_prompt, add_dirs, mcp_config, mcp_tools, env`. `env` (from the agent type's `[env]`) becomes one shell-quoted `NAME=value` word after the `env -u ...` words for `claude`, and `export 'NAME=value'; <command>` in front of a `shell` type's command; an empty name or one holding `=` is refused (`InvalidInput`). `ClaudeCodeAdapter` (`agent/claude.rs`) builds this argv:

```
claude [--model=M] [--effort=E] [--permission-mode=MODE | --dangerously-skip-permissions]
       [--append-system-prompt=P] [--session-id=ID] [--allowedTools=T1,T2]
       [--add-dir=D ...] [--mcp-config=JSON] [-- PROMPT]
```

- Every option and its value is one argv element (`--name=value`), and the launch command shell-quotes every element, so no value can split or start a new flag. `--allowedTools` is one comma-joined element, so its position does not matter.
- `mcp_config` is the JSON of the one MCP server an agent gets, after the isolation flags (`--strict-mcp-config` stays: no other server loads). `mcp_tools` are its tools, allowed without asking: they join `--allowedTools` (and the tool notice) when the type lists tools, and are the only `--allowedTools` entry when it lists none, so they never turn a type without a tool list into one (no `--tools=`). Both are checked like `allowed_tools`.
- The prompt is the only positional, after the end-of-options `--`. A prompt such as `--dangerously-skip-permissions`, `-p`, `--permission-mode=bypassPermissions` or `--` arrives as the prompt, never as a flag.
- `claude` still runs a subcommand when the first positional equals its name, even after `--` (`claude -- mcp` runs `claude mcp`). A prompt without any whitespace therefore gets a trailing newline; no subcommand name has one.
- `add_dirs` are folders outside the worktree claude may read without a permission prompt (in `default`, `acceptEdits` and `plan` its Read asks for anything outside the working directory and `--add-dir` folders): one `--add-dir=<folder>` each, refused unless absolute and UTF-8, and when it starts with `-` or holds control characters.
- `model`, `session_id` (a UUID) and each `allowed_tools` entry are refused (`InvalidInput`) when they start with `-` or hold control characters; a tool may not hold a comma. The system prompt may hold anything, since it sits inside its own `--append-system-prompt=` element. Blank values add no flag; no prompt means no `--`.
- `effort` (agent type TOML `effort`): one of `low`, `medium`, `high`, `xhigh`, `max`, the values `claude --effort` lists; anything else is refused when the type is checked. It becomes `--effort=<level>` right after `--model`, and unset adds no flag, so claude keeps its own default. The agent type list and `read_agent_type` show it as `effort` (`null` when unset).
- `permission_mode`: `default`, `acceptEdits`, `plan` and `auto` become `--permission-mode=<mode>`; `bypassPermissions` becomes `--dangerously-skip-permissions`. `claude` 2.1.318 lists `acceptEdits, auto, bypassPermissions, manual, dontAsk, plan` and no longer lists `default`, but still accepts it, so `default` is passed as is for now.
- Checked against the installed `claude` 2.1.318 (empty environment, throwaway `HOME`, an invalid `--session-id` so it stops right after parsing): `-- --no-such-flag` reaches the session-id check while `--no-such-flag` alone is `unknown option`, and `--allowedTools=Read,Bash(git diff:*),Edit` and the `--name=value` forms parse.
- The launch command puts `env -u <name>` for each of `agent::OUTER_SESSION_ENV` in front of `claude` (see Environment below). `ClaudeCodeAdapter::args(&options)` returns the argv after the program unquoted, without that prefix, for a caller that spawns `claude` without a shell (2.2's runner, which removes the same variables itself).
- Townhall's own session (`runner/args.rs`) follows the same rules, spawned without a shell: `claude -p --output-format=stream-json --verbose --input-format=stream-json --permission-prompt-tool=stdio --include-partial-messages (--session-id=ID | --resume=ID) --model=M --permission-mode=default --append-system-prompt=P --strict-mcp-config --mcp-config=JSON --tools=Read,Grep,Glob,WebFetch,WebSearch --allowedTools=T1,T2 --disallowedTools=T1,T2 --setting-sources= --settings={"disableAllHooks":true} --disable-slash-commands [--add-dir=D ...]`. `M` is the `model` of the type `default`, read at the start of every turn (`opus` when it has none). The session id, the MCP config and each folder are refused (`InvalidInput`, the turn fails) when they start with `-` or hold control characters, a folder also unless absolute and UTF-8. The system prompt may hold newlines.

### Claude-side plugin facts (phase 3.1 spike)

Claude version: the docs above record `claude` 2.1.318; this spike could not run `claude --version` (its shell was limited and refused `ls`, `cat` and `claude`), so the version is the one already recorded, not re-read. Nothing here was verified by a run. Each answer says "from docs" (Claude Code's own plugin files under `~/.claude/plugins/marketplaces/claude-plugins-official`, `plugin-dev` skills and `README.md`) or "unverified". The first build task of 3.2 and 3.3 runs the checks below with a throwaway `HOME` and a fake or real `claude` and changes the word.

| question | answer | status |
|----------|--------|--------|
| Do `--settings=<file>` hooks run with `--setting-sources=` empty? | Expected yes: `--settings` is its own source, not one of `user,project,local`, and the docs above already rely on `disableAllHooks` in `--settings` working with an empty `--setting-sources=`. Not shown for adding hooks | unverified |
| Does `--mcp-config=<path>` take a file? | Claude's `--mcp-config` takes "JSON files or strings"; `plain()` in `agent/claude.rs` accepts a path | unverified (from `claude --help` wording as quoted in this repo's docs) |
| Tool name of a renamed server key | Plugin-loaded servers are `mcp__plugin_<plugin>_<server>__<tool>` (plugin-dev `mcp-integration` skill). For a key given in `--mcp-config` the name is `mcp__<key>__<tool>`; with key `plugin_<plugin>_<server>` both give the same string, so section 6.3 holds either way | from docs (plugin form), unverified (`--mcp-config` form) |
| Does a `PreToolUse` allow beat `--disallowedTools` or a type's deny rules? | Not stated in the installed docs. `plugin-dev` hook-development lists `permissionDecision: allow\|deny\|ask` only. Claude Code's documented order is deny rules first, then hooks, so deny should win, but that is from memory | unverified |
| `${CLAUDE_PLUGIN_ROOT}` | Used in `command` of 20+ installed `hooks.json` and in `command` and `args` of `.mcp.json` (`imessage`, `discord`, `plugin-dev` `stdio-server.json`). Claude substitutes it for plugins it loads and also exports `CLAUDE_PLUGIN_ROOT` to hook processes (`i-have-adhd` reads `process.env.CLAUDE_PLUGIN_ROOT`). A `--settings` file is not a plugin, so Claude will not substitute it: townhall must | from docs and files on disk |
| `marketplace.json` `source` forms | In `claude-plugins-official`: a relative string, `{source:"url", url, sha}`, `{source:"git-subdir", url, path, ref, sha}` (README and 20+ entries); `strict: false` entries describe skills in the entry because the repo has no `plugin.json` (README, "Skill-bundle plugins"). `{source:"github"}` is in no installed marketplace, so it is unverified | from files on disk; `github` unverified |

Plugin hooks run in parallel with each other and with the user's (plugin-dev, "run in parallel"). Fixtures for 3.2 and 3.3 are in `crates/townhall-core/tests/fixtures/plugins/claude/`.

### Townhall's session leaves your own Claude setup out

Unit 2.11. Townhall's session plans tasks; your hooks, CLAUDE.md, skills, slash commands, plugins and extra built-in tools must not steer it. The flags were picked from `claude --help` of `claude` 2.1.318 (nothing else was run), all as one `--name=value` element each (`runner::ISOLATION` plus `--tools`):

| flag | why |
|------|-----|
| `--setting-sources=` (empty) | loads none of the user, project and local settings files: no hooks, `enabledPlugins`, permission rules or `env` from them, and no CLAUDE.md, which claude loads only for an enabled source (the Agent SDK runs claude the same way when its `settingSources` is empty). Managed (policy) settings still apply, as they should |
| `--settings={"disableAllHooks":true}` | hooks stay off even if some source still had one (a plugin, a future default) |
| `--disable-slash-commands` | "disable all skills and commands" |
| `--tools=Read,Grep,Glob,WebFetch,WebSearch` | the whole built-in tool set: read code and open urls or search the web, but no Bash, Edit, Write, Task and the like exist in the session. MCP tools are not built in, so townhall's own stay |
| `--strict-mcp-config --mcp-config=...` (as before) | townhall's MCP server only, none of yours |

`--allowedTools` (townhall's MCP tools plus `READ_ONLY_TOOLS` = Read, Grep, Glob, WebFetch, WebSearch) and `--disallowedTools` stay as they were; `LS` left the list, since `--tools` names the built-in set.

Auth keeps working: it comes from your login (the macOS keychain or `~/.claude/.credentials.json`, with the account in `~/.claude.json`) and from `ANTHROPIC_*` / `CLAUDE_*` variables, which pass through (see Environment). None of these flags reads or skips them. Not chosen: `--bare`, which never reads OAuth or the keychain (only `ANTHROPIC_API_KEY` or an `apiKeyHelper`), so a subscription login would stop working; `--safe-mode`, a troubleshooting mode that also disables MCP servers, which could take townhall's own with it; `--restricted`, which overlaps the flags above but also confines and gates writes in ways townhall does not need. One limit: a provider or `apiKeyHelper` set only in your `~/.claude/settings.json` (not in the environment) is no longer read by townhall's session.

`permission_mode` always comes from the task's agent type (`AgentDef::launch_options`), never from a request body or a tool call: a type that sets none runs in `acceptEdits`, and `bypassPermissions` (`skip all`) only when the type holds it. A type holding it is only ever saved by `save_agent_type` after the user confirmed its TOML. The binary is `Context::claude_bin()` (`TOWNHALL_CLAUDE_BIN`, default `claude`); tests point it at a fake script.

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
    "prBase": { "type": "string", "minLength": 1,
                "description": "Existing branch of the repo the task's pull request (open_pr) targets. Default: baseBranch. Set it when the task starts from a branch that is not where the work must land, for example a shipper that starts from the reviewed branch and opens its pull request into main." },
    "doneWhen": { "type": "string", "maxLength": 500,
                  "description": "What finished means, in plain words. Put in the agent's brief and shown to people. Optional." },
    "check": { "type": "string", "maxLength": 2000,
               "description": "Shell command run in the task's worktree when its agent ends; exit 0 means done. A failure runs the task again with its output in the brief, up to maxRetries times, then the task needs the user. Optional." },
    "maxRetries": { "type": "integer", "minimum": 0, "maximum": MAX_HANDOFFS,
                    "description": "How many times a failing check relaunches the task. Default 2." },
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

`repo` defaults to the conversation's `linkedRepo`, else the repo of its latest `repo` entry; none of those is a tool error that lists the repos. `baseBranch` defaults to the repo's. `prBase` (migration 0205, `tasks.pr_base`) is optional and stored on the task, shown by `task_status` as `prTarget` (the branch `open_pr` will use: `prBase`, else `baseBranch`) and by the web task view; it is never used to start the task or by the gate, which keep reading `baseBranch`. It must be a plain branch name: it is checked with `git check-ref-format --branch` (its printed name must equal the input), then looked up with `git show-ref --verify --quiet`, so revision syntax is never evaluated. A blank value, a name that starts with `-` or `@`, holds whitespace, control characters, `..`, `@{`, `^`, `~`, `:` or ends in `.lock`, `HEAD`, a 40 or 64 hex digit commit id and a branch that does not exist are tool errors, so it is never a sha. When the repo has an `origin` remote the branch must exist as `refs/remotes/origin/<name>` (the local remote-tracking ref, no fetch) and the refusal is `branch is not on origin: push it first`; with no `origin` a local branch (`refs/heads/<name>`) is accepted. Any existing branch is accepted, including a release or protected one: a pull request is only a request, and protecting a branch is the forge's job. `open_pr` answers with `base`, the branch it targeted, so the planner and the user can see it. `agent` (2.12) matches a saved type ignoring case and is stored with the type's spelling; left out it is `builder` when that exists, else the first type by name; an unknown type is a tool error that lists the types. 2.3 resolves both into a `NewTask`, calls `Store::create_task`, then `Store::note_task_created`. No tool takes a permission mode.

**Verification** (`doneWhen`, `check`, `maxRetries`; migration 0206 `tasks.done_when`, `check_cmd`, `max_retries`, `verify`; all optional and additive, so a client that sends none sees no change). A task with a `check` is not done when its agent ends:

1. The agent watcher (`agent/watch/sync.rs`) leaves the task `working` and sets `verify` `{ phase: "verifying", retries, output }`, so dependants do not start and the reaper keeps the worktree. A crash or a question is not held back.
2. The scheduler (`scheduler/verify.rs`) runs `sh -c <check>` in the task's worktree on its own tokio task (one at a time per task, at most 600 s, otherwise it failed), once per pass while the task is `verifying`. Exit 0: the task is `ended`, `verify` `passed`, and an `agent` `ended` entry says so.
3. Anything else is a failure, with the tail (4000 characters) of stdout then stderr and `(exit code N)` as its output. `Store::check_failed` is one write. With `retries < maxRetries` it records a hand back of the task to its own agent type with reason `the check `<cmd>` failed (retry n/max). Its output: ...`, queues the task again and sets `verify` `{ phase: "retrying", retries: n, output }`: the next pass resumes it in the same worktree and branch as a new agent run (new session), and its brief now holds the output like any hand back. When the new agent ends the check runs again.
4. With no retries left (or `MAX_HANDOFFS` hand backs used) the task is `needsYou` with `verify` `{ phase: "failed", retries, output }`, an `agent` `failed` entry with the output, and the conversation's planner gets one event line. It is left alone from then on: the user decides.

`maxRetries` is at most `MAX_HANDOFFS` because each retry is a hand back. The brief gets a `Done when` section from `doneWhen` and one line naming the check. The web shows `verifying`, `retry n/max` and the final result (`web/src/tasks/verify.ts`).

**Retry backoff.** A failed check with retries left is not requeued at once: `scheduler/verify.rs` waits `Pacing.check_backoff` (60 s) for the first retry, twice that for the second, and so on up to `Pacing.check_backoff_max` (600 s), then calls `Store::check_failed`. The task stays `verifying` while it waits, so a check that polls external state (a deploy, a CI run) is read again after that state had time to change. The last failure, which sends the task to `needsYou`, does not wait. The wait holds only a weak context, so a restart can reopen the store meanwhile; after a restart the check simply runs again.

**Waiting for CI** (`scheduler/ci.rs`, `repos/pr_checks.rs`). Sessions cannot outlive their agent, so a lander that opened a PR with `open_pr` and ends while the checks run would end with "checks pending". Townhall waits for it:

1. A task that is `ended` (its check, if any, passed), is not a `reflector` and has a `pr` whose `checks` is `pending` has its PR's checks read once per `Pacing.ci_poll` (30 s). `gh pr checks <n> --json name,state,bucket` runs in the task's worktree as the repo's registered forge account (`Accounts::for_repo` and `pick`, with the token set for that one child, exactly like `open_pr`). On GitLab, `glab ci get --branch <branch> --output json` is read and its pipeline status is the one check `pipeline`. A repo with no `origin` or a task without a worktree is not read.
2. Every check is done (none pending): the task stays `ended` and its `pr.checks` becomes `passed`, `failed` (a `fail` or `cancel` bucket) or `none` (no checks), so it is not read again. A task with no `pr` is never read.
3. Some check is pending (`bucket` `pending`: queued or in progress): the task becomes `inCi`, with an `agent` `ended` entry `ended with pending checks: waiting for CI on PR #n (name: STATE, ...)`. `task_status` shows `state` `inCi` and the entry; the web shows `in ci`. The watcher leaves it alone, and the reaper keeps its worktree (it is not `ended`; once its session is gone only its build artifacts are deleted).
4. While `inCi` the checks are read every `Pacing.ci_poll`. When none is pending any more, `pr.checks` is set and `Store::hand_after_ci` hands the task back to its own agent type, in the same worktree and branch, as a new agent run with reason `the checks of PR #n finished: build: FAILURE, lint: SUCCESS. CI finished: merge if green, fix if red`. The brief shows it like any hand back. One relaunch per CI completion.
5. Bounds, each ending in `needsYou` with `blocked` set to the reason (the states included), a `needsYou` entry and one planner event: the task was already relaunched `Pacing.ci_waits` (3) times and ends with pending checks again; it waited `Pacing.ci_timeout` (60 min) in `inCi` for one run; or `MAX_HANDOFFS` hand backs are used. A `needsYou` task with `blocked` set is parked: neither the watcher nor a removed agent changes it, and the user decides.

**Merging on green** (`scheduler/land.rs`, `merges_on_green` of the agent type). A task whose agent type merges on green has its PR merged by the daemon instead of a new agent run: the type's `merges_on_green` (TOML key, `Option<bool>`) when set, else true only for the type named `lander`. A lander should therefore end right after pushing and opening the PR with CI pending; it does not poll or merge. Each read of such a task (`ended` with pending checks, or `inCi`) is judged by `land::decide`:

- Some check pending: wait (steps 3 to 5 above, the bounds included).
- No checks, or a failed or cancelled one: relaunch as in step 4.
- All green: `gh pr view <n> --json headRefOid,mergeable` and `gh api repos/{owner}/{repo}/compare/<base>...<head> --jq .behind_by` (base is `pr_base`, else `baseBranch`) are read. `CONFLICTING` or behind by more than 0: relaunch, the reason saying the PR conflicts with or is behind the base. `UNKNOWN`: wait for the next read. Otherwise `gh pr merge <n> --merge --match-head-commit <head>` runs as the repo's forge account, the merge commit is read with `gh pr view <n> --json mergeCommit`, and the task becomes `merged` with `pr.mergeSha` set. The conversation gets a `passed` entry and the planner hears `task <name> merged PR #n into <base>: merged, sha <sha>`. No new agent run starts.

GitHub only: on GitLab nothing is merged by the daemon and the old relaunch after CI applies. GitHub auto-merge is not used (the base branch has no required checks).

The wait is kept in the daemon's memory (`Pacing.ci_timeout` starts again after a restart); the count of waits is the task's hand backs whose reason starts with `the checks of PR #`. Tests replace `Pacing` and the sleeper with `Context::set_pacing` and `Context::set_sleeper`.

One `tasksCreated` entry per turn: a turn is everything after the conversation's latest `you` entry (the whole thread before the first). The turn's first task appends a `tasksCreated` entry; each later task of that turn is added to it in place (same `id` and `at`), and the entry is sent again as `entryAppended`. Calls are serialized, so tasks created at the same time still share the entry. Each refusal (`name` taken by a task, an agent or a branch `townhall/<name>`, unknown `repo`, `baseBranch`, `prBase`, `after` task or attachment, over a schema limit, an unknown field) is a result with `isError: true` and a text that says what to change. A notification runs nothing; an empty batch is `-32600`, an unknown method `-32601`, `tools/call` without `params.name` `-32602`.

`set_title` (result the `Conversation`):

```json
{ "type": "object", "properties": { "title": { "type": "string", "minLength": 1, "maxLength": 80 } },
  "required": ["title"], "additionalProperties": false }
```

Task events (`runner/notify.rs`): when a task of a conversation starts to need you, fails or ends, the agent watcher (`agent/watch/event.rs`) calls `runner::notify` with one line (`task <name> needs you, its agent asks: ...`, `task <name> failed: ...`, `task <name> ended (...). its agent's last message: ...`). Going back to work, `sent` and `passed` lines send nothing. Events of one conversation gather for `ContextOptions::notify_delay` (3 s) so a burst makes one turn, wait while a turn runs (also one you stopped), then are appended as `event` entries and start one planner turn whose user message is `townhall event (automatic, the user did not write this):` plus the lines. The turn uses the linked or last repo (no inference, no `repoAsk`), sets `thinking` like a message but leaves `ended` alone, and an ended conversation is not woken. The planner's reply is a normal `townhall` entry and wakes nothing: only a new task change does. Undelivered events are lost on a restart (the `agent` and `needsYou` entries stay). An `event` entry also ends a turn for `tasksCreated`: tasks the woken planner creates make a new entry.

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

`save_agent_type` never saves at once. Every save, new or replacing, and so every type that sets `permission_mode = "bypassPermissions"`, is not saved: townhall appends a `townhall` entry (Markdown: `save the new agent type `x`?` or `replace the saved agent type `x` with this one?`, the TOML in a `toml` code fence longer than any backtick run in it, then `answer yes to save it, or say what to change.`), records the ask in `agent_type_asks`, and answers `asked` with `next` telling the session to end its turn and wait. Once you said yes, the session calls it again with `confirmed: true`; that saves only when this conversation's ask for that name holds exactly this `source` and a `you` entry came after the ask's entry, else it is a tool error that says why (not asked, a different TOML, no answer yet). A save closes the ask. The entry is a plain `townhall` entry because the web knows no definition block yet (3.7). Townhall's system prompt (`runner/prompt.rs`) says every task names an agent type, that it can design new types with you (personality, model, tools) and that a type's permission mode (`default`, `acceptEdits`, `plan` or `bypassPermissions`, `acceptEdits` when left out) is the mode its agents run in, and that `bypassPermissions` is skip all.

`conversation_insights` (`mcp/insights.rs`, analysis in `townhall_core::insights`): the recurring patterns across every conversation, not only the caller's, counted without a model. Read-only: no table, no mutation. Only townhall's own session gets it (`allowed_tools()`), never a task's agent (`HandBackTools` neither lists nor runs it).

| tool | input | result |
|------|-------|--------|
| `conversation_insights` | `{ since?, top? }`: `since` a duration like `7d` or an RFC 3339 time (default `30d`), `top` patterns (default 10, clamped to 1..30) | `{ since, counts, patterns: [Pattern], truncated }` |

- It reads `Store::conversations`, `entries` per conversation, `all_tasks`, `handoffs` per task and `task_ended_times`, then calls the pure `insights::analyze(&InsightData, now_ms, &InsightsArgs)`, which is unit-tested on fixture data. Entries, hand backs and tasks older than `since` are left out.
- `counts`: `conversations`, `tasks`, `user_messages`, `needs_you`, `agent_failed`, `agent_sent`, `handoffs`, `errors` and `tasks_by_agent` (agent type, then task state, then number).
- `Pattern { kind, pattern, count, first_seen, last_seen, samples, conversations, tasks, fingerprint, suggested_automation }`. Only patterns seen at least twice (`MIN_COUNT`), ranked by count, then most recent `last_seen`, then kind and pattern. `kind` is one of: `user_request` (a `you` entry that is not a reply, normalized: lowercased, redacted, words with digits or paths dropped), `needs_you` (the question), `agent_failed` and `manual_step` (the text of an `agent` entry with event `failed` or `sent`, minus the agent's own name), `handoff` (`from -> to`, plus `: reason` when there is one), `task_chain` (the agent types of a task and its first `after` parents, oldest first, then each type it was handed to; at least 2 stages, at most 8) and `townhall_error` (`error` entries).
- `fingerprint` is 16 hex chars from the same FNV hash as `log_summary` (`logs::fingerprint`) over the kind and the normalized pattern, so it does not change between builds and the planner can cite it in a task brief. `suggested_automation` is a rule-based hint per kind (new agent type or planner prompt, ask-less default, prompt or tool change, auto-chain after X).
- Caps: `samples` at most 3 per pattern, each redacted, on one line and cut to 160 chars (`pattern` too); `conversations` and `tasks` at most 5 ids each; the whole result at most 24 KB (`MAX_OUTPUT_BYTES`): the lowest ranked patterns are dropped first and `truncated` is `true` (also when `top` cut some). All text goes through `redact` first.
- The system prompt tells the planner to start from it when asked what to automate, citing each pattern's fingerprint and `suggested_automation`.

`search_history` and `read_conversation` (`mcp/history.rs`, query in `store/unit_history.rs`): full-text search over old conversations and a compact read of one, so the planner and agents need not carry them in context. Read-only. Both the conversation's endpoint and a task's endpoint list and run them, `allowed_tools()` and `task_allowed_tools()` grant them, so every agent type gets them. `1001_default_history_tools` appends them to the seeded `default` tools and adds one line to its prompt (as in `runner/prompt.rs`): before planning work that may have been done or rejected before, call `search_history`.

| tool | input | result |
|------|-------|--------|
| `search_history` | `{ query, repo?, since?, kind?, limit? }`: `query` plain words (stemmed, all must match) or FTS5 syntax; `repo` a conversation linked to it or with a task in it; `since` a duration like `7d` or an RFC 3339 time (default all time); `kind` a list of `message`, `task`, `agent_report`, `needs_you`; `limit` default 10, clamped to 1..50 | `{ hits: [{ kind, conversation, title, task?, agent?, state?, branch?, pr?, prUrl?, at, snippet }] }`, best first |
| `read_conversation` | `{ id, offset?, limit? }`: `offset` messages to skip (default 0), `limit` default 30, clamped to 1..100 | `{ id, title, repo?, created, updated, total, offset, messages: [{ at, by: "you" or "planner", text }], tasks: [{ name, title, agent, state, branch, pr?, prUrl?, lastMessage? }] }` |

- `1000_history_index` creates the FTS5 table `history_fts` (tokenizer `porter unicode61`; `text` indexed, `kind`, `conversation_id`, `src_id`, `task` and `at` stored unindexed), backfills it and keeps it in sync with insert, update and delete triggers on `messages`, `tasks` and `events` (a conversation delete cascades to them). Indexed: `message` (`you` and `townhall` messages), `task` (title, a newline, then the brief), `needs_you` (the `needsYou` entry's question) and `agent_report` (the `event` entries `task <name> ended (...). its agent's last message: ...` and `task <name> failed: ...` that wake the planner; the agent's last message is not stored anywhere else in the database). An event dropped before delivery (daemon restart, ended conversation) has no report to find.
- Plain words (no `" ( ) : * ^ { }`, no leading `-` or `+`, no `AND`, `OR`, `NOT`, `NEAR`) are quoted one by one, so `fix-flaky-timing-tests` works. Anything else goes to FTS5 as written; a syntax error is a tool error that says to put words with punctuation in double quotes.
- Ranked by `bm25`, then newest. `snippet` is the match in `[brackets]` on one line, about 14 words. Task fields come from the task the hit belongs to, when it still exists. Titles, snippets and messages go through `redact`.
- `read_conversation` only shows `you` and `townhall` entries, each cut to 600 characters, and per task the last `agent_report` line's last message cut the same way. It never returns raw tool output or steps.

`history_graph` and `agent_stats` (`mcp/graph.rs`, store in `store/unit_graph.rs`): a graph over townhall's own history, so the planner can ask how past work went and who touched what without loading old chats. Read-only, listed and allowed like `search_history` (both endpoints, `allowed_tools()` and `task_allowed_tools()`). `1051_default_graph_tools` appends them to the seeded `default` tools and adds one line to its prompt (as in `runner/prompt.rs`): before touching an area that has a history, call `history_graph`; when asked how agents perform, call `agent_stats`.

| tool | input | result |
|------|-------|--------|
| `history_graph` | `{ node, kind?, depth?, since?, limit? }`: `node` a file path (a suffix such as `insights/mod.rs` or a glob such as `src/store/*` works), task, conversation id, branch `townhall/<task>`, pr `<repo>#<number>`, agent type, outcome or log fingerprint; `kind` one of `file`, `task`, `conversation`, `branch`, `pr`, `agent`, `outcome`, `fingerprint` (default: guessed in that order); `depth` 1 (default) or 2; `since` a duration like `7d` or an RFC 3339 time (default all time); `limit` default 10, clamped to 1..50 | `{ node: { kind, id }, matched, tasks: [{ task, title, conversation, conversationTitle, agent, outcome?, branch, pr?, at, retries?, handBacks?, files?, fingerprints? }] }`, newest first. `matched` counts before `limit`. `files` (up to 10) and `fingerprints` (up to 5) come with `depth` 2 and always for a task node. An unknown node is a tool error |
| `agent_stats` | `{ since?, agent? }`: `since` default `30d`, by task creation; `agent` one type, ignoring case | `{ since, agents: [{ agent, tasks, outcomes: { <outcome>: n }, avgRetries, handBacks, failures: [{ reason, count }], failsOn: [{ area, count }] }] }`. `tasks`, `outcomes` and `avgRetries` count the tasks now under that type; `handBacks` is how many times it handed a task back; `failures` the top 3 reasons of its failed and needs-user tasks, classified from stored data: `API error ...`, `connection lost / went to sleep`, `check failed: <first line of the check output>`, `permission denied`, `tool missing`, `ci failed`, `max retries`, `needs user: <question>`, else the `task <name> failed: ...` text (all redacted and normalised like log fingerprints), `unknown` when nothing was stored; `failsOn` the top 5 folders its failed or needs-user tasks changed |

Outcomes and files are reconciled with the base branch by the scheduler (`scheduler/graph.rs`, `1052_graph_landing`), with no network calls: a settled task counts as `merged` when a merge commit on the base (or `origin/<base>`) names its branch (`Merge pull request #N from .../townhall/<task>`, `Merge townhall/<task>`; text after ` into ` is ignored) or its PR number, when its last `ended` message says `merged` with a PR that landed, or when it is in the `after` of a merged task and changed files. A merged task whose branch is gone and has no files gets them from `git diff <merge>^1 <merge>`. The scan reruns only when the base head moved or a task was newly recorded, so it is idempotent and also fixes existing data on the first pass after startup. A task keeps `merged` through later `ended` or `failed` updates until it is queued again.

- `1050_history_graph` creates `graph_edges` (`src_kind, src, rel, dst_kind, dst, at`; kinds `conversation`, `task`, `agent`, `branch`, `pr`, `file`, `outcome`, `fingerprint`) and `graph_recorded`. Edges: conversation `has` task, task `ran_as` agent type (one per type that ran it, so a hand back adds one), task `on` branch, branch `opened` pr, task `ended_as` outcome, task `touched` file, task `hit` fingerprint. Deleting a task deletes its edges.
- The structural edges and the outcome are rewritten in the same transaction that writes the task. Outcomes: `merged`, `failed` (state `error`), `api-error` (a `failed` task whose last `task <name> failed:` report says `API error`), `needs-user`, `ended`, `in-review` (in CI or review); a queued or working task has none.
- Files and fingerprints are recorded by the scheduler pass (`scheduler/graph.rs`) once a task is `ended`, `merged` or `error`, at most 20 tasks per pass. Files come from `git diff --name-only <start commit>..townhall/<task>` (`<base>...townhall/<task>` when the launch is unknown), so a merged branch keeps its files; a branch that no longer exists records no files. The fingerprints are those of the task's warnings and errors in `~/.townhall/logs/` (as `log_summary`). The same pass backfills every older task not yet in `graph_recorded`. A task that runs again (state `queued` or `working`) is recorded again when it ends.
- Titles and reasons go through `redact`.

`ask_adviser` (`mcp/adviser.rs`, run in `runner/advice.rs`): one non-interactive `claude -p` turn of the agent type `adviser`, seeded by `0810_seed_adviser` (model `claude-fable-5-2`, moved to the alias `fable` by `0813_adviser_model_alias` unless you edited the line, effort `high`, permission mode `plan`, tools `Read Grep Glob WebSearch WebFetch`) and appended to the seeded `default` tools by `0811_default_ask_adviser`. Both the conversation's endpoint and a task's endpoint list it, and `task_allowed_tools()` and `allowed_tools()` grant it.

| tool | input | result |
|------|-------|--------|
| `ask_adviser` | `{ question, context? }` (`question` at most 20 000 characters, `context` at most 50 000) | `{ answer, needsUser, model }`. `needsUser` is true when the answer starts with `NEEDS THE USER`. A failure is a tool error starting `the adviser could not answer:` |

- It runs in the task's worktree, or in the conversation's repo, with the saved type's model, effort, tools and prompt (read each call, so an edit applies at once). Tools that write, run commands or reach townhall (`Bash`, `Edit`, `MultiEdit`, `Write`, `NotebookEdit`, `mcp__*`) are dropped from the type's list and disallowed, a mode that accepts edits runs as `plan`, no MCP server is passed and the session is not saved (`--no-session-persistence`). Any tool request outside the list is denied.
- It stops after `ADVICE_TIMEOUT` (600 s) with `the adviser did not answer within 600 seconds`. With no type named `adviser` it answers `no adviser type: ...` without running anything. The adviser type can be deleted like `builder`; a task running as the adviser cannot ask itself.
- Every call is a row of `adviser_calls` (question, answer, state `done` or `failed`, model, tokens, times), counted as a run of the adviser in `/api/library/agents/usage`, `.../{name}/usage` and `.../{name}/runs` (kind `consult`, `task` is the task name, or the conversation title for the conversation agent). The call is also a step in the caller's own transcript.
- A task's agent is told in its system prompt (`scheduler/agent_type.rs`) to ask the adviser before it stops to ask the user, and to ask the user only when `needsUser` is true. Like `hand_back`, the tool reaches a task only when its type may hand back to some type, since both come from the task's MCP server. A task with no `agent` never defaults to the adviser.

Workflow tools (`list_workflows`, `save_workflow`) wait for 2.8c, which also adds `run_workflow` in `mcp/runs.rs`. `tools()` and `allowed_tools()` pick them up.

## Scheduler

`scheduler::start(ctx)` runs a pass on every `taskUpdated` and every 2 s (`scheduler::TICK`); tests call `scheduler::run_once(ctx)`. A pass:

- starts queued tasks oldest first, each once every `after` task is `ended` or `merged` (an `error` one holds it) and while fewer than `agentsAtOnce` tasks are `working` or `needsYou` with a live session (an interactive claude stays alive after its work, so an `ended` task frees its slot; agents that are no task's do not count).
- Start: reads the task's agent type as saved now (2.12, `scheduler/agent_type.rs`), writes the task's brief, `# <title>`, the prompt and, with attachments, `Files attached to this task:` and one `- <path>` each, to `<home>/tasks/<name>/brief.md` (never the repo or the worktree), then `agent::start` in the task repo's workspace, name = the task name, base = the commit `baseBranch` is at, with the type's `AgentDef::launch_options()`: a `claude` type gets `ClaudeCodeAdapter::new(ctx.claude_bin())` with the type's `--model`, `--allowedTools` and `--append-system-prompt`, its own mode (`acceptEdits` when it sets none), a new session id and `add_dirs` = the brief's folder plus, with attachments, the conversation's `files` folder; a `shell` type runs its `command` in the worktree (`ShellAdapter`, no session id). Then `working`. A task whose type was deleted after it was created goes to `error` with ``could not start: agent type `x` was deleted after the task was created: create the task again with one of: ...``. A prompt of any size fits, since tmux takes the command as one argument of at most about 16 k. The agent runs:

  ```
  env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID -u CLAUDE_CODE_ENTRYPOINT -u CLAUDE_CODE_SSE_PORT -u CLAUDE_EFFORT claude [--model=<model>] --permission-mode=<mode> [--append-system-prompt=<prompt>] --session-id=<uuid> [--allowedTools=<tools>] --add-dir=<home>/tasks/<name> [--add-dir=<home>/conversations/<id>/files] -- 'read your task brief at <home>/tasks/<name>/brief.md and do it. it is the full task.'
  ```

  (`--dangerously-skip-permissions` instead of `--permission-mode` for `skip all`). The prompt's own text only ever reaches claude through the brief.
- sets `error` with an `agent` `failed` entry (`could not start: <reason>`, at most 300 characters; a failed command reads ``<its stderr> (`<command>` exited with <code>)``, so the cut drops the long command line, never the reason) when the start fails, or when another agent has the task's name (in another repo, or in another case).
- adopts a queued task whose own agent (its exact name, in its repo) exists already (a start cut short by a restart) as `working`, never starting a second agent; a `working` task is never touched.
- marks an `ended` task `merged`, with an `agent` `passed` entry (`merged into <base>`), when `townhall/<name>` is reachable from its base branch (`git for-each-ref --merged=<base>`) and its tip is not the commit it started from (a branch with no commits of its own is not merged). Read only: it never merges or deletes.

### Required steps

An agent type with `requires` cannot skip the steps it names (`scheduler/requires.rs`, `gate.rs`, `report.rs`):

- `create_task` refuses a task of that type unless its `after` chain, followed transitively and ignoring case, holds a task whose `agent` is each required type. The error names the missing types. `hand_back` applies the same check to the target type against the task's own `after` chain and refuses, leaving the task running, when a required type is missing.
- A queued task whose `after` tasks finished still does not start until, for each required type, the newest task of that type in the chain ended (`ended` or `merged`) with a report that clears it. The report is the final message of the Claude session recorded for that task (`task_launches.session_id`), read from `<projects>/<worktree folder>/<session id>.jsonl` and nothing else: a `report` artifact is never read, because `add_artifact` on `/api/tasks/{name}/mcp` has no auth and any local agent could file one for another task, and the newest transcript of the folder is never read either, because any local agent can write one. A task with no recorded session blocks with `<task> (<type>) has no recorded session to read its report from`; a recorded session whose transcript is not there blocks with `<task> (<type>) session transcript was not found: its report cannot be read`.
- Report lines: the whole final message is scanned, so `VERDICT:` and `COMMIT:` may come first (the relay truncates long messages) or last. A line counts only when it is exactly `VERDICT: <WORD>` or `COMMIT: <sha>` from the first column, with optional trailing whitespace, and is not inside a fenced code block or a blockquote (`>`); any other line is ignored. The sha is the full 40 hex digits, lower or upper case; a short sha or a name is read as no commit and blocks, so a branch or tag named like a short sha cannot stand in for a commit. A fence opens with three or more backticks or tildes and closes only with the same character, at least as many, and nothing else on the line (a `~~~` inside a backtick fence is content); one never closed runs to the end of the message. A line right after a `>` line, with no blank line between, is quoted too. Exactly one distinct verdict word and one distinct commit are allowed: identical repeats are fine, different ones block with `conflicting VERDICT lines: FAIL, PASS` (or `COMMIT`). The word must be in the required type's vocabulary: `tester` PASS or FAIL, `reviewer` APPROVE or REJECT (one table in `report.rs`; no other type has a vocabulary, so a coder writing PASS clears nothing); a word outside it blocks. If a type that cannot be required is somehow read by the gate, its tasks stay blocked with ``agent type `x`: `coder` cannot be required: ...``. `APPROVE` and `PASS` clear. No verdict, no commit or no report blocks.
- `REJECT` or `FAIL` is final, not a block: the gated task ends in `error` with a `failed` agent entry and one planner event, `task <name> failed, it will not start: <required task> (<type>) reported FAIL; create a new task after the fix`, and `scheduler::Pass.failed` names it. The planner creates a new task after the fix. The rejection reason stays in the task's `blocked` (shown by `task_status`), which marks the `error` as the gate's. The planner is told and the entry is written first, then the task's session is killed: a kill error is only logged and never aborts the scheduler pass or loses the entry or the note. The watcher leaves an `error` task that has a `blocked` reason as it is, so a handed-back task never moves on to `ended` or `needsYou` and no task `after` it starts. Still running, no report yet, a malformed or conflicting report and a stale report stay non-terminal blocks. A REJECT or FAIL wins over a block of another required type.
- Stale rule: the commit is compared with the head the task will start from, resolved once to a sha before the gate. A task with handoffs (handed back) must have its own branch `townhall/<name>`: that tip is the head, it never falls back to the base, and any git error (a missing branch included) fails closed, so the task keeps waiting with a warning. Any other task starts from its `baseBranch` (any rev: a branch, `origin/main`, `refs/heads/main`, a sha) after `worktree::fresh_base` (the fetch, which may pick `origin/<base>` when the remote is ahead), so the gate checks the commit the worktree is made from. `Gate::Open` carries that sha and the start (`agent::start`) and the resume use exactly it and never look the base up again: a base that moves after the check cannot start unchecked code, and a resume whose worktree is not at the checked sha, or whose `HEAD` (`git symbolic-ref HEAD`) is not the task's own branch `refs/heads/townhall/<name>` (another branch, or a detached `HEAD`), is refused: the task ends in `error` with `the worktree of `<task>` is at <sha>, not the checked commit <sha>` or `... is on <ref>, not its own branch <ref>: check out <ref>`. For a handed-back task the order is: kill the old session, gate (reports, clean tree), check the sha, then the branch, then spawn. A report is fresh only when it names that head exactly. Any later commit makes it stale until a new report is made, a merge of the base branch included: a merge can carry changes of its own (an evil merge, a conflict resolution with edits) and checking each one is not worth the risk. An unknown commit blocks with `which is not a commit of this repo`.
- A blocked task stays `queued` with `Task.blocked` set to the reasons, one line, joined by `; `; `task_status` and the web task list show it, and `scheduler::Pass.blocked` names it. The reason never holds the head's sha (a stale report names the sha it covers and the head by branch name), so a base branch that moves does not change it. The planner gets one `event` entry when the reason first appears or changes. A handed-back task whose worktree has uncommitted changes (`git status --porcelain` not empty) blocks too, with `` `<task>` has uncommitted changes in its worktree that no COMMIT covers: commit or discard them``, because no report can cover them. The scheduler clears `blocked` once the task may start, and `fail()` (an ordinary start error such as a missing base branch, a clash or a refused resume) clears it too, so an ordinary error on a held task never looks like a gate rejection; only the gate's own rejection (`gate::reject`) keeps `blocked`, which is what the watcher's `error` exemption reads. The gate runs for a task with no agent yet and for a handed-back task before its new agent resumes in the kept worktree; a queued task whose agent is already running (adopted after a restart) and was not handed back is never gated or killed.

## Task agent MCP: `POST /api/tasks/{name}/mcp`

Same JSON-RPC handling as the conversation endpoint (`mcp::handle_task`), for the agent of one task, which gets `hand_back`, `open_pr` and the proof tools `add_artifact` and `list_artifacts` (below). A `claude` task whose type can hand back to at least one other type starts with `--mcp-config=` (`mcp::task_config_json(base_url, name)`), `mcp::task_allowed_tools()` (`mcp__townhall__hand_back`, `mcp__townhall__open_pr`, `mcp__townhall__add_artifact`, `mcp__townhall__list_artifacts`) and, in its system prompt, ``you can hand back to: <types>``. A `shell` type, or a type with `can_hand_back_to = []`, gets none of it.

The route accepts bodies up to about 30 MB before a tool's own cap applies (`open_pr` and `hand_back` share it).

`open_pr { title, body? }`: pushes `townhall/<name>` to `origin` and opens a pull request with `gh pr create` (GitHub) or a merge request with `glab mr create` (GitLab) into the task's base branch (`repos::publish::publish`). It runs as the repo's account (below). The host comes from the origin URL (https, `ssh://` and scp forms); a host that is neither `github.com`/`gitlab.com` nor contains `github` or `gitlab` needs `git config townhall.forge github|gitlab` in the repo, else it fails before pushing. A repo with no `origin` answers state `noRemote` (nothing pushed, `pr` unchanged); a local-folder origin, a missing or logged-out CLI and a failed push are errors with the command to run. On success it answers state `opened` with `url`, `number`, `base` (the target branch; `noRemote` carries `base` too) and `account` (`{ login, host }` of the account it ran as, `null` when none was enforced), sets the task's `pr` (`checks` `pending`) and publishes `taskUpdated`; an already open request is returned as is. The programs are `gh` and `glab`, or `TOWNHALL_GH_BIN` and `TOWNHALL_GLAB_BIN`. The target is the task's `prBase` when it has one, else its `baseBranch` (`Task::pr_target()`), so a gated shipper that starts from the reviewed branch (`baseBranch` = that branch, so the gate compares the reports' COMMIT with its tip) is created with `prBase` = the repo's base branch and opens its pull request into that, not into the branch it starts from. A task without `prBase` behaves as before.

Which account `open_pr` runs as (`repos::accounts::Accounts::pick`), for the forge and host of the origin: the repo's own pick (`Repo.githubAccount` or `gitlabAccount`; a pick that is not registered for that host is an error saying so, before any command runs), else the first registered account for that forge and host, else for GitHub the `githubAccount` setting, else none (the CLI's own active login, as before). With an account, townhall fetches its token when it spawns the child and never stores it: GitHub `gh auth token --user <login> --hostname <host>` becomes `GH_TOKEN` (`GH_ENTERPRISE_TOKEN` on a host other than `github.com`) plus `GH_HOST`, GitLab `glab config get token --host <host>` becomes `GITLAB_TOKEN` plus `GITLAB_HOST`. The variables go to `git push`, `gh pr create` and `glab mr create` only; `gh auth switch` is never run, so the CLI's own active account is not changed. A CLI with no login for the account is an error naming the account and the `auth login --hostname <host>` command, and nothing is pushed. On GitHub the token's user must be the account's login (else an error), and it becomes the worktree's git author and committer (`<id>+<login>@users.noreply.github.com`). Later `gh` calls by the agent run as the account with `GH_TOKEN="$(gh auth token --user <login> --hostname <host>)"`; `hand_back`'s instructions say so.

`hand_back { to, reason, summary? }`: `to` is a saved agent type (never the task's own, and one of `can_hand_back_to` when the type sets it), `reason` is required, `summary` is what the agent did. Refusals are texts that name the types it can hand back to. It writes once (`Store::hand_back`): a `task_handoffs` row (`reason`, and `summary` plus the branch's commit subjects since its first start), the task `queued` again with `agent` = `to`, and a `handoff` entry in its conversation. Only a `working` or `needsYou` task can be handed back, and at most `MAX_HANDOFFS` (5) times per task, then `Conflict`.

### Proof artifacts (2.9)

Proof a task's agents attach outside the repo: a row in `artifacts` (migration 0900) and the bytes in `<home>/artifacts/<conversation id>/<task>/<artifact id>-<name>`. Append only: there is no tool or route to replace or delete one.

`add_artifact { task, name, kind, content, encoding?, author_role, caption? }`: `kind` is `screenshot`, `log`, `note` or `report`; `encoding` is `text` (default) or `base64`; `author_role` is `coder`, `tester`, `reviewer` or `other`. An agent adds only to its own task (`task` must be it). It returns `{ id, name, kind, mime, size, url }`. The type is decided from the kind and the bytes, never from the agent: a `screenshot` must be a png, jpeg, gif or webp by its first bytes (the name gets that extension); the other kinds must be UTF-8 text, stored as `text/plain` (`log`) or `text/markdown` (`note`, `report`), so nothing is ever served as HTML. The name goes through `files::naming::file_name` (folders, invisible characters and edge dots dropped). Limits: `MAX_ARTIFACT_BYTES` (5 MB) per artifact, `MAX_ARTIFACTS_PER_TASK` (50), a caption of at most `MAX_CAPTION_CHARS` (300); `/tasks/{name}/mcp` takes a body up to that size as base64.

`list_artifacts { task }`: `{ task, artifacts: [{ id, name, kind, author_role, caption, created, url }] }`, oldest first, for the agent's own task or any other task of the same conversation. A task of another conversation answers as if it did not exist (`no task named ... in this conversation`). `url` is `<base_url>/api/tasks/<task>/artifacts/<id>`.

`task_status` lists each task's `artifacts` (`name`, `kind`, `authorRole`, `url`) its `blocked` reason (`null` when nothing holds it back) and its `prTarget`, the branch its pull request will target.

The next scheduler pass resumes it: the brief gains a `This task was handed back` section with each hand back's `Why` and `What was done so far`, and the old session is killed first (`scheduler::launch::stop_old_session`, before the gate reads the worktree, so the agent that handed it back cannot commit or edit meanwhile; a kill error keeps the task queued with a warning and is retried next pass), then the gate and the checks below run, then `agent::restart` runs the target type in the same worktree and branch. The commit its merge is told from stays the first one; the task takes a slot like a start.

## Agent states

Unit 2.5, `agent/state.rs` and `agent/watch.rs`.

- `state::detect(ws, agent)` reads Claude's transcripts from `state::claude_projects()` (`$TOWNHALL_CLAUDE_PROJECTS`, else `~/.claude/projects`); `state::detect_in(projects, ws, agent)` takes that folder as an argument (`None`: the screen only), so tests pass a temp folder.
- No session is `ended`. A dead pane (sessions keep it with `remain-on-exit`) is `ended` when it exited 0, else `error` with the exit code (a signal as 128 plus its number). A live pane is `waiting` when the transcript has a pending `AskUserQuestion` or a turn that ended on a question, when the screen shows a selection prompt (read even while the transcript says busy: claude 2.1.318 writes a message with parallel tool calls only once every call has a result, so a call waiting for permission leaves the transcript on another call's result; the screen is captured again only after the pane printed), when the screen shows claude's folder trust dialog (every new worktree: `Yes, I trust this folder` above `No, exit`, numbered or not), asked as `claude asks whether to trust this worktree` (the watcher answers it for townhall's own worktrees, see below), or, for a program without a transcript, when its last line asks. A live Claude whose turn ended without a question `state::IDLE_SETTLE_MS` (10 s) ago or longer is `ended`: interactive Claude never exits on its own, and a short pause between tool calls must not count. Anything else live is `working`. When tmux cannot be asked, alive is `working` and dead is `ended`.
- `state::choices(question)`: what can be typed back, the numbers of the question's numbered options, else the two words of a `y/n` marker, else nothing. The trust question's are `yes, trust it` and `no, exit`, but the dialog takes keys, not text: its cursor starts on `No, exit`, so `{ "key": "up" }` then `{ "key": "enter" }` trusts the worktree, while any text (sent with its Enter) picks `No, exit` and claude exits. The watcher puts them in the `needsYou` entry's `choices`.
- The watcher (`agent::watch::start`) polls every agent every 1.5 s (`watch::INTERVAL`). When a state changes it publishes `agentUpdated`, moves the agent's task (`working`, `needsYou`, `error`, `ended`; never a `queued`, `merged`, `inCi` or `inReview` one, nor an `error` one the gate rejected (it has a `blocked` reason, so that `error` is terminal); an `error` set by a crash still follows its agent when it restarts) and appends a `needsYou` or `agent` entry. An exit counts once seen on two polls in a row.
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
- Import: when `~/.townhall/config.json` exists at open, its `repo` is registered (when it is still a git repo) and saved as `desktop.repo` (existing rows are never overridden), and the file is renamed to `config.json.imported` (`-2`, `-3` ... when taken). Never deleted. Nothing else is read from it: a `settings` object is ignored. A corrupt file is left alone with a warning. The desktop app imports the older `<app config dir>/settings.json` into `desktop.repo` once, when the store has none.

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
| 0900 to 0949 | 2.9 artifacts | `store/unit_2_9.rs` |
| 0950 to 0999 | plugins (0950 marketplaces and plugins, 0951 plugin_settings, 0952 plugin_storage and plugin_tokens) | `store/unit_plugins.rs` |
| 1000 to 1049 | history search (1000 history_index, 1001 default_history_tools) | `store/unit_history.rs` |
| 0300 to 0499, 1050 and up | free | ask the orchestrator |

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
| `tasks` | `name` pk `collate nocase`, `conversation_id`, `title`, `prompt`, `repo`, `base_branch`, `after` (JSON names), `state`, `attachments` (JSON ids), `pr_number`, `pr_url`, `pr_checks`, `created_at`, `agent`, `pr_base` (migration 0205, the next free number of the tasks range above 0204), `blocked` (migration 0204; 0203 is `0203_task_ended_at`, and a database whose ledger records 203 under another name, such as the unreleased `0203_task_blocked`, is refused at startup with an error naming both, since its `ended_at` column was never added) | indexed by conversation and by state |
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
| 2.8a definitions | `crates/townhall-core/src/definitions/`, its `pub mod` line | `LaunchOptions` fields (owned by 2.1, already there), `model::PermissionMode` |
| 2.8b library | `store/unit_2_8b.rs` and `store/migrations/080x to 084x_*.sql`, `crates/townhall-daemon/src/routes/library.rs`, `crates/townhall-core/src/mcp/library.rs` | `Store::call` for its tables, `definitions::*` (2.8a), `JsonBody` |
| 2.8c runs | `store/unit_2_8c.rs` and `store/migrations/085x to 089x_*.sql`, `crates/townhall-daemon/src/routes/runs.rs`, `crates/townhall-core/src/mcp/runs.rs` | `Store::{call, create_task, update_task, append_entry}`, 2.3's scheduler, 2.7's transcript reader |

## Open items

- The storage plan asks about backups (`VACUUM INTO`) and per-repo settings; neither is built.
