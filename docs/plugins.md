# Plugins and marketplaces (design)

Status: proposal, nothing of this is built. It is the basis for later build tasks, so it names real files, functions and routes as they are on `main` today (`09c2e8c`). Where it says "verify", the claim comes from memory of Claude Code's plugin docs and not from a file or a run in this repo; the first build task that depends on it checks it against the installed `claude` first.

## 1. Goals and non-goals

Townhall as it is today is the base. A plugin lets a person shape it to their needs, in four ways:

1. Declarative contributions, no code: agent types, prompts, plugin settings, a theme.
2. Townhall hooks: shell commands run on townhall's own events, which can change what happens.
3. Claude-side contributions: Claude Code hooks and MCP servers that the claude sessions townhall launches get while the plugin is enabled.
4. UI: sandboxed iframes in fixed slots, talking to townhall through a small `postMessage` API, plus a versioned plugin API in the daemon that hooks, MCP servers and UI call.

Plugins are JSON manifests plus git repos. An official marketplace is built in, and a user can add their own, private ones included.

Non-goals for the first versions:

- No plugin code runs inside the daemon process. No dynamic library loading, no WASM, no embedded JS runtime. Plugins are separate processes (shell commands, MCP servers) or sandboxed iframes.
- No change to townhall's own planner isolation. Today's launch flags stay the default; plugins add to them only where a user opted in (section 7).
- No payments, ratings, search ranking or review process for marketplaces.
- No OS-level sandbox for hooks in the first versions (section 10 says what that means).

### What exists today (grounding)

- `README.md` goals already say "custom behaviour lives in plugins, not in core". Nothing else in the repo is called a plugin: `git log -i --grep=plugin` finds only `48e3d51` and `e459b52`, which are about keeping the user's own Claude plugins out of townhall's sessions (`--setting-sources=`). No plugin system was added or removed before; there is nothing to revive or migrate.
- Townhall keeps no config files: state is in `~/.townhall/townhall.db` (one owner, `store::Store`), content in folders (`docs/daemon-api.md`, Storage). This design keeps that rule.
- Agent types are TOML (`definitions::AgentDef`, `deny_unknown_fields`, keys `name description adapter model effort permission_mode tools prompt command can_hand_back_to requires [avatar] [env]`), saved in the `agents` table by `routes/library.rs` and `mcp/library.rs`, read at launch by `scheduler/agent_type.rs::launcher`.
- Every claude townhall starts is isolated: `agent/claude.rs` `AGENT_ISOLATION` = `--setting-sources=`, `--settings={"disableAllHooks":true,"autoMemoryEnabled":false}` (`ISOLATED_SETTINGS`), `--strict-mcp-config`. A task agent gets an MCP config only when its type may hand back (`mcp::task_config_json`, `mcp::task_allowed_tools`). Townhall's own session (`runner/args.rs`) adds `--tools=Read,Grep,Glob`, `--allowedTools`, `--disallowedTools=Bash,Edit,MultiEdit,Write,NotebookEdit`, `--disable-slash-commands`. Note: `docs/daemon-api.md` (Permission mode) still shows `--settings={"disableAllHooks":true}`; the code also sets `autoMemoryEnabled:false`. That doc line is stale and not touched here.
- The daemon API has no authentication. It is bound to `127.0.0.1`, and CORS plus `cors::origin_allowed` only keep other browser origins out (`crates/townhall-daemon/src/cors.rs`, `ALLOWED_ORIGINS`). Any local process can call it. This matters for the trust model (section 10).

## 2. Format

### 2.1 Layout and names

A plugin is identified as `<plugin>@<marketplace>`, like Claude Code. Both parts are `[A-Za-z0-9._-]`, start with a letter or digit, at most 64 characters, unique ignoring case (same rules as repo names, `repos::names`).

A marketplace is a git repo with a `marketplace.json` at one of, in order:

1. `.townhall-plugin/marketplace.json`
2. `.claude-plugin/marketplace.json`

A plugin is a folder with, in order of preference:

1. `.townhall-plugin/plugin.json` (townhall plugin: everything below is available)
2. `.claude-plugin/plugin.json` (a plain Claude Code plugin: only the Claude-side parts, section 2.4)
3. neither: the marketplace entry alone describes it (Claude Code's `strict: false` style, verify), which is how a marketplace wraps a repo that has no manifest.

Challenge to the planner's proposal "stay close to Claude Code's format": agreed for marketplace and plugin identity and for the Claude-side components, but townhall-only keys go in a separate `.townhall-plugin/plugin.json`, not in `.claude-plugin/plugin.json`. Reasons: Claude Code may refuse or warn on unknown keys in its own manifest (verify with `claude plugin validate`), and a repo can then be both a Claude Code plugin and a townhall plugin with one set of hooks and MCP files and no duplication. The townhall manifest never repeats what the Claude files already say.

### 2.2 `marketplace.json`

What was checked on disk (real files in `~/.claude/plugins/marketplaces/*/.claude-plugin/marketplace.json`): top-level `$schema` (optional), `name`, `description`, `owner { name, url? }`, `plugins[]` with `name`, `description`, `source` (the string `"./"` in those files), `category`. Townhall reads the same shape and adds nothing at top level:

```json
{
  "name": "acme-plugins",
  "description": "Plugins of the Acme team",
  "owner": { "name": "Acme", "url": "https://acme.example" },
  "plugins": [
    {
      "name": "release-notes",
      "description": "Writes release notes when a task is merged",
      "version": "0.3.0",
      "source": { "source": "github", "repo": "acme/townhall-release-notes", "ref": "v0.3.0", "sha": "3f9c0d1e5b7a4c2d8e6f1a0b9c8d7e6f5a4b3c2d" },
      "category": "workflow",
      "tags": ["release", "docs"]
    },
    {
      "name": "theme-paper",
      "description": "A light paper theme",
      "source": "./plugins/theme-paper"
    }
  ]
}
```

`source` forms (verify against the installed claude, the first is the only one seen on disk):

| form | meaning in townhall |
|------|---------------------|
| `"./path"` | a folder inside the marketplace repo (the repo is cloned anyway) |
| `{ "source": "github", "repo": "owner/name", "ref"?, "sha"? }` | a GitHub repo; cloned over the marketplace's forge account if it has one (section 8.3) |
| `{ "source": "url", "url": "https://...git" \| "git@host:o/r.git", "ref"?, "sha"? }` | any git url, GitLab and self-hosted included |
| `{ "source": "git-subdir", "url", "path", "ref"?, "sha"? }` | a folder of another repo (verify that Claude Code has it; townhall supports it as it is cheap: sparse checkout) |

`sha` (40 hex) wins over `ref`; with neither, the remote's default branch at install time is resolved to a sha and that sha is what is stored. Townhall extension keys on an entry live under one key, `townhall`, so Claude Code ignores them (verify): `"townhall": { "apiVersion": 1, "permissions": {...} }` is how a marketplace wraps a plain Claude Code plugin that has no townhall manifest (section 2.4).

### 2.3 `.townhall-plugin/plugin.json`

```json
{
  "name": "release-notes",
  "version": "0.3.0",
  "description": "Writes release notes when a task is merged",
  "author": { "name": "Acme", "url": "https://acme.example" },
  "homepage": "https://github.com/acme/townhall-release-notes",
  "license": "MIT",
  "apiVersion": 1,
  "minTownhall": "0.5.0",
  "permissions": {
    "api": ["tasks:read", "tasks:create", "storage"],
    "hooks": ["task.merged", "pr.before_open"],
    "claude": {
      "hooks": { "events": ["PostToolUse"], "agentTypes": ["release-writer"] },
      "mcp": { "servers": { "notes": ["search_notes", "add_note"] } },
      "planner": false
    },
    "prompts": ["starters"],
    "env": ["ACME_TOKEN"],
    "ui": ["settings.group", "session.section"]
  },
  "contributes": {
    "agentTypes": ["agent-types/release-writer.toml"],
    "prompts": { "starters": "prompts/starters.json", "brief": "prompts/brief.md" },
    "settings": "settings.schema.json",
    "theme": "theme.css",
    "hooks": "townhall-hooks.json",
    "ui": [
      { "slot": "session.section", "title": "release notes", "entry": "ui/session.html", "height": 160 },
      { "slot": "settings.group", "title": "release notes", "entry": "ui/settings.html" }
    ]
  },
  "claude": {
    "hooks": "hooks/hooks.json",
    "mcp": ".mcp.json"
  }
}
```

Rules:

- `name`, `version` (semver), `description`, `author`, `homepage`, `license` mean what they mean in Claude Code's `plugin.json` (checked on disk: `name`, `version`, `description`, `author { name, email?, url? }`).
- Every path is relative to the plugin root, is canonicalized and must stay inside it (no `..`, no absolute path, a symlink pointing outside is refused at install). Size caps: manifest 64 KB, any one contributed file 1 MB, whole plugin 50 MB.
- `claude.hooks` and `claude.mcp` default to Claude Code's own places (`hooks/hooks.json`, `.mcp.json`; verify), so a repo that is already a Claude Code plugin needs no keys here.
- `apiVersion` is the plugin API version the plugin was written for (section 9). A plugin whose `apiVersion` is higher than the daemon serves cannot be installed (`400`, `needs plugin api 2, this townhall serves 1`).
- Unknown keys are an install error (like `deny_unknown_fields` on `AgentDef`), so a typo in `permissions` can never silently grant or hide something.
- `permissions` is a declaration, checked against use: a hook file that names an event not under `permissions.hooks`, an MCP server not under `permissions.claude.mcp.servers`, or a UI slot not under `permissions.ui` is an install error. The user approves exactly this list (section 10.1).

### 2.4 Mapping to Claude Code's plugin format

| Claude Code | Townhall | Notes |
|-------------|----------|-------|
| `.claude-plugin/marketplace.json` | read as is (fallback to `.townhall-plugin/marketplace.json` first) | same `name`, `owner`, `plugins[]` |
| `.claude-plugin/plugin.json` (`name`, `version`, `description`, `author`) | identity of a plain Claude plugin; the townhall file wins when both exist | |
| `hooks/hooks.json` (`{"hooks":{"<Event>":[{"matcher","hooks":[{"type":"command","command","timeout"}]}]}}`, checked on disk) | Claude hooks, injected into agents (section 6) | `${CLAUDE_PLUGIN_ROOT}` (used by plugins on disk) is replaced by townhall with the plugin's absolute path |
| `.mcp.json` / `mcpServers` | Claude MCP servers, merged into the agent's `--mcp-config` (section 6) | server keys are renamed to avoid clashes |
| `commands/`, `skills/` | not supported in phase 3 | task agents keep claude's own skills and slash commands today (`AGENT_ISOLATION` only removes settings, hooks and MCP); injecting a plugin's skills would need `--plugin-dir` or `--add-dir` and is a separate decision (open question 7) |
| `agents/*.md` (Claude subagents) | not supported | not the same thing as townhall agent types, which are whole sessions; the names collide, so townhall's are in `agent-types/*.toml` |
| `outputStyles`, `lspServers` | ignored | |
| none | `contributes`, `permissions`, townhall hooks, UI, settings, theme | townhall only |

Wrapping a plain Claude Code plugin: a marketplace entry with a `townhall` key (section 2.2) supplies `permissions` for it, and townhall derives `claude.hooks` and `claude.mcp` from the files found. A plain Claude plugin with no `townhall` entry installs with a derived permission list, shown to the user in full (every hook event found, every MCP server and "all of its tools"), and nothing else is offered for it (no townhall hooks, no UI).

## 3. Declarative contributions

No code runs for these. They are parsed by core, validated at install and on every start, and failing validation disables only that contribution, with the reason shown on the plugin's page.

### 3.1 Agent types

`agent-types/<name>.toml`, exactly the existing `AgentDef` format (parsed by `definitions::check_agent`, the same checks as `Store::check_agent_type`, including the 10 000 character limit), with one added key:

```toml
name = "release-writer"
description = "writes release notes from merged tasks"
adapter = "claude"
model = "haiku"
permission_mode = "default"
plugins = ["release-notes"]
prompt = """
You write release notes from the brief.
"""
```

Design decisions:

- Plugin types are an overlay, not rows in `agents`. `Store::agent_types()` and `agent_type(name)` (`store/unit_2_8b.rs`) return user types plus the types of enabled plugins; the one place that launches a task, `scheduler/agent_type.rs::launcher`, already goes through `store.agent_type(&task.agent)`, so a plugin type launches with no change there. `AgentType` gets a new field `origin: "user" | "plugin"` (and `plugin: string | null`); the existing `source` field is the TOML text and stays so.
- The type's name as the user and tasks see it is `<plugin>-<name>` (`release-notes-release-writer`), so it fits `create_task`'s `^[A-Za-z0-9_-]+$` and never collides with a user type. A plugin type can never be named `default`, `builder`, `reflector`, `tester`, `reviewer` or any name a user type has: the first user type wins and the plugin's is skipped with a warning.
- Plugin types are read only: `PUT` and `DELETE /api/library/agents/{name}` answer `409 owned by plugin <id>`. The agents page shows a badge and a `copy to my types` button that saves a user type under a new name (a plain `PUT`).
- The planner sees them in `list_agent_types` like any other. `save_agent_type` still always asks the user first; a plugin cannot save or edit a user type without the API scope `library:write`, and then only through the same ask (section 9).
- `permission_mode = "bypassPermissions"` in a plugin type is refused at install. A plugin may not ship a skip-all agent; the user can copy the type and edit it, which is a conscious act.
- A task whose plugin type disappears (plugin disabled or removed) fails to start with the existing message for a deleted type, naming the plugin.
- `plugins = [...]` in `AgentDef` is the opt-in for Claude-side contributions (section 6). It needs a field in `AgentDef` (`#[serde(default)] pub plugins: Vec<String>`), the TS `AgentType` and the agent type editor; `deny_unknown_fields` means every older build refuses a type that has it, which is the safe direction.

### 3.2 Prompts

`prompts.starters` (JSON list of `{ title, text }`) adds entries to the starters on home (`web/src/views/home/starterPrompts.ts`, `Starters.tsx`). `prompts.brief` (Markdown) is appended to every task's `brief.md` under a heading `From plugin <id>` by `scheduler/brief.rs`, and is shown in the task view. `prompts.planner` (Markdown) is appended to the planner's system prompt (`runner/prompt.rs`) after the repo notes and before the profile, under a line saying it came from a plugin and never overrides the rules, as the profile does. `brief` and `planner` steer agents, so each needs its own permission (`prompts: ["brief", "planner"]`) and is listed on the install prompt. Size cap 8 000 characters each, since the planner prompt rides in `--append-system-prompt`.

### 3.3 Settings

A plugin's own settings, not townhall's. `settings.schema.json` is a flat object of fields `{ key, label, description?, type: "boolean" | "string" | "integer" | "enum", default, min?, max?, choices?, secret? }`. Values are stored per plugin (`plugin_settings`, section 8.5), never in the `settings` table, whose keys are a closed set (`SettingKey`, `PUT /api/settings/{key}` refuses unknown keys). The settings page renders a group per plugin from the schema, with the same row components as `views/settings/SettingRow.tsx`, without any iframe. Hooks get the values in their payload, MCP servers and UI read them through the API (`settings:read` is implied for a plugin's own values). A `secret` value is stored like a value but never returned by `GET /api/plugins/...` to the web UI after saving and never logged.

A plugin cannot change townhall's own settings (`agentsAtOnce`, `notifyWhen`, ...) in the first versions. Open question 5 asks whether to allow suggestions ("apply recommended settings").

### 3.4 Theme

townhall's theme today is the `data-theme` attribute (`dark` or `light`, `web/src/theme/theme.ts`) over CSS custom properties in `docs/style-guide/tokens.css` (`--th-bg`, `--th-surface`, `--th-fg`, `--th-accent`, ...). A theme contribution is one CSS file that may only contain rules whose selectors are `:root`, `[data-theme="dark"]` or `[data-theme="light"]` and whose declarations are custom properties named `--th-*`. The daemon parses it at install; any other selector, `@import`, `url(...)`, `expression`, or a property that is not `--th-*` is an install error. The web app injects the validated text as a `<style>` after the built-in tokens. At most one theme is active (`plugins.theme` choice in the store); `contrast` of the palette is the plugin author's job, the style guide has `check-contrast.mjs` for it.

## 4. Townhall hooks

Shell commands that run on townhall events. Declared in `townhall-hooks.json` (the file `contributes.hooks` names):

```json
{
  "hooks": {
    "task.before_start": [
      { "command": "${TOWNHALL_PLUGIN_ROOT}/bin/check-ticket", "timeout": 10 }
    ],
    "task.merged": [
      { "command": "node ${TOWNHALL_PLUGIN_ROOT}/bin/release.mjs", "timeout": 60, "async": true }
    ]
  }
}
```

Same shape as Claude Code's `hooks.json` on purpose (event key, list of `{ command, timeout }`); no `matcher` in the first version, the hook filters on the payload itself.

### 4.1 Events

Taken from the actual task and conversation lifecycle. "Can change" is what a hook's answer may do (section 4.2); every other event is notify-only and runs without holding anything up.

| event | fires at | payload (besides `event`, `plugin`, `apiVersion`, `settings`) | can change |
|-------|----------|-----------------|------------|
| `task.created` | `create_task` tool (`mcp/task_tools.rs`) after `Store::create_task` | `task`, `conversationId` | block (task is ended with the reason), amend `prompt` (append) |
| `task.before_start` | `scheduler/launch.rs::act`, after `admit` (the gate in `scheduler/gate.rs` is open), before `launch` or `resume` | `task`, `repo`, `agentType`, `resume: bool`, `briefPath` | block (the task stays `queued` with `blocked` = the reason, like a gate hold), append to the brief, add `env` entries for the agent |
| `task.started` | `scheduler/launch.rs::settle`, after `queued` to `working` | `task`, `agent`, `sessionId` | none |
| `task.needs_you` | the agent watcher (`agent/watch/sync.rs`, `agent/watch/event.rs`) when a task goes to `needsYou`, together with `runner::notify` | `task`, `question`, `choices` | none |
| `task.ended` | watcher, task to `ended` (`event.rs`) | `task`, `lastMessage` (the same text the planner gets) | none |
| `task.failed` | watcher task to `error`, `scheduler/launch.rs::fail` ("could not start"), `gate::reject`, `Store::check_failed` with no retries left | `task`, `reason`, `phase: "start" \| "run" \| "gate" \| "check"` | none |
| `task.check` | `scheduler/verify.rs`, before `sh -c <check>` | `task`, `worktree`, `check` | add checks: extra commands run after the task's own, same pass/fail meaning |
| `task.handed_back` | `Store::hand_back` (`store/handoffs.rs`) | `task`, `from`, `to`, `reason` | none |
| `pr.before_open` | `mcp/open_pr.rs`, before `repos::publish::publish` | `task`, `repo`, `title`, `body`, `base` | block (a tool error to the agent with the reason), amend `title`, `body` |
| `pr.opened` | `open_pr` success | `task`, `url`, `number`, `base` | none |
| `task.merged` | `scheduler/merged.rs`, task to `merged` | `task`, `base`, `commits` | none |
| `message.before_turn` | the conversation turn start (`runner/turn.rs` via `routes/conversations.rs` and `runner::notify`) | `conversationId`, `text`, `kind: "you" \| "event"` | add `additionalContext` that is appended to that turn's message under a "from plugin" line; never rewrites or hides the user's text, never blocks |
| `message.reply` | a `townhall` entry appended to the thread | `conversationId`, `entryId`, `text` | none |
| `conversation.created`, `conversation.ended` | `Store::create_conversation`, `POST /conversations/{id}/end` | `conversationId` | none |
| `repo.added` | `RepoRegistry::add` | `repo` | none |
| `daemon.started` | `background::start_background` | `home`, `version` | none |

"Landing" in townhall is `open_pr` then the scheduler noticing the branch is merged, so before and after landing map to `pr.before_open`, `pr.opened`, `task.merged`. Whatever lands later (merge queues, a `land` task type) gets its own event rather than a flag on these.

Implementation note: call sites call one function, `plugins::hooks::fire(ctx, Event) -> Outcome`, in a new `crates/townhall-core/src/plugins/` module. The ones that can change something are awaited (with the timeout); the rest are spawned and forgotten, so a slow hook never delays the scheduler pass or the watcher poll (`watch::INTERVAL` 1.5 s, `scheduler::TICK` 2 s). Events are not persisted: like `runner::notify` today, a hook not yet run when the daemon stops is lost, and the doc of each notify-only event says so.

### 4.2 Protocol

- Run as `sh -c <command>` (the same way `check` runs, `scheduler/verify.rs`), working directory the plugin's data folder, own process group, killed on timeout (default 10 s for events that can change something, 30 s for the rest; at most 120 s).
- stdin: one JSON object, the payload above (all strings redacted with `redact::redact` where they quote logs or tool output).
- Environment: a short allow list, not the daemon's: `PATH`, `HOME`, `LANG`, `LC_*`, `TZ`, plus `TOWNHALL_EVENT`, `TOWNHALL_PLUGIN` (the id), `TOWNHALL_PLUGIN_ROOT`, `TOWNHALL_PLUGIN_DATA`, `TOWNHALL_API_URL`, `TOWNHALL_API_TOKEN` (section 9.2) and each name the plugin declared under `permissions.env` that is set in the daemon's environment. The forge tokens townhall fetches for `open_pr` and anything `ANTHROPIC_*` are never passed.
- Exit 0 with empty stdout: continue. Exit 0 with JSON on stdout: an answer. Exit 2: block, stderr is the reason (Claude Code's convention). Any other exit code, a timeout, or output over 64 KB: the hook failed.
- A failed hook fails open: the event goes on as if the hook had not existed, one `event`-style warning goes to the log (`target: "townhall::plugin"`, `event = "plugin.hook_failed"`), and after 5 failures in a row the plugin's hooks are paused until the user re-enables them (a toast, and a line in the plugin's page). Failing closed would let one broken plugin stop every task; a plugin that really must gate can say so with `"failClosed": true` on a hook of a `task.before_start` or `pr.before_open` event, which the install prompt shows.
- Answers (JSON on stdout), only the keys the event allows, anything else ignored with a warning:

```json
{ "decision": "block", "reason": "ticket ABC-123 is not open" }
{ "decision": "continue", "amend": { "prompt": "Ticket ABC-123: ...", "env": { "TICKET": "ABC-123" } } }
{ "decision": "continue", "checks": ["npm run lint"] }
{ "decision": "continue", "additionalContext": "The user is on call this week." }
```

- Several hooks on one event run one after the other in install order (marketplace order then plugin name), each seeing the amendments of the earlier ones; the first `block` wins and the rest are skipped. A task whose hook blocked shows the plugin's name and reason in the task view (`Task.blocked` for `task.before_start`, an `agent` `failed` entry for `task.created`).
- Amendments are bounded: appended brief text up to 8 000 characters per plugin, at most 20 `env` entries, `env` names and values checked like the type's `[env]` (`LaunchOptions.env`, empty name or `=` refused), values never logged.

## 5. What a plugin may not do through hooks

Hooks cannot change a task's agent type, its permission mode, its `tools`, its repo or its `after`; they cannot answer an agent's question, send keys to a pane or end a task. They can create tasks, post messages and read, only through the API and only with the matching scope (section 9). This keeps the permission model of `docs/daemon-api.md` ("`permission_mode` always comes from the task's agent type, never from a request body or a tool call") true for plugins too.

## 6. Claude-side contributions

Goal: when a plugin is enabled and an agent type opted in, the claude it launches also gets the plugin's Claude Code hooks and MCP servers, without reopening the user's own setup (user settings, CLAUDE.md, user plugins stay out, `--setting-sources=` stays).

### 6.1 Selection: two keys must both be turned

A plugin's Claude-side parts reach a launch only when:

1. the plugin is enabled (installed and the permission list approved), and
2. the agent type lists it in `plugins = [...]` (section 3.1), or the type is one the plugin itself ships (it names its own plugin implicitly).

The default is none: a type without `plugins`, which is every type today, launches with exactly today's argv. Existing tests (`agent/claude_tests.rs`, `runner/args.rs`) must stay green unchanged; a new test asserts the argv is byte-identical when no plugin is selected. There is no `"*"`: a user cannot turn on all plugins for all types by accident, and a plugin cannot add itself to a type it does not ship.

Townhall's planner session is the `default` agent type (`runner/persona.rs`). It gets plugin MCP or hooks only when `default` lists the plugin and the plugin declared `claude.planner = true` (a separate permission, default `false`), and then only tools the plugin marked read only (section 6.3). Reason: the planner's isolation (`READ_ONLY_TOOLS`, `DENIED_TOOLS`) exists so it can plan but never write; a plugin MCP tool that writes would bypass that.

### 6.2 Hooks: `--settings` carries them

Today: `--settings={"disableAllHooks":true,"autoMemoryEnabled":false}` (an inline JSON element in `ISOLATED_SETTINGS`). With plugin hooks selected the element becomes a path to a generated file:

```
--settings=<home>/tasks/<name>/claude-settings.json
```

```json
{
  "autoMemoryEnabled": false,
  "hooks": {
    "PostToolUse": [
      { "matcher": "Edit|Write", "hooks": [ { "type": "command", "command": "/Users/me/.townhall/plugins/cache/acme/release-notes/3f9c0d1/hooks/format.sh", "timeout": 30 } ] }
    ]
  }
}
```

- `disableAllHooks` is left out only in this case; when no plugin hook applies the old inline element stays.
- `--setting-sources=` stays, so the user's and the repo's own hooks stay off; only townhall's file defines hooks. Phase 3.1 could not run `claude` and recorded this as unverified (`docs/daemon-api.md`, Claude-side plugin facts); 3.3 must run it with the installed `claude` in a throwaway `HOME` before relying on it: that `--settings` hooks are honored with an empty `--setting-sources=`.
- The file is written to the task's own folder `<home>/tasks/<name>/` (never the repo or the worktree, same rule as `brief.md`), mode 0600, before `agent::start`, and replaced on `resume`. It is a file and not an inline element for two reasons: the whole command line goes through tmux (about 16 k, `docs/daemon-api.md`, Scheduler) and a plugin with many hooks would overflow it; and it can hold per-task tokens that must not show in `ps` or tmux's command (section 9.2).
- `${CLAUDE_PLUGIN_ROOT}` in a plugin's `command` is replaced by the plugin's absolute path in the cache. This is needed: Claude substitutes it only for plugins it loads itself, and a `--settings` file is not one (from docs, unverified by run). Claude's own plugin hooks also read the variable from the environment, hence the env below. Townhall also sets `CLAUDE_PLUGIN_ROOT` and `TOWNHALL_PLUGIN_DATA` in the agent's env (`LaunchOptions.env`, section 3 of the daemon API: one shell-quoted `NAME=value` word, not secret).
- Only the events the plugin declared under `permissions.claude.hooks.events` are merged; a hook file with another event is an install error. A `PreToolUse` hook can allow or deny a tool call, which can widen what the agent does without asking, so the install prompt calls that event out in words ("can approve or block tool calls"), and whether `--disallowedTools` and a type's own `Edit`/`Bash` limits still win over a hook's allow is unverified (phase 3.1 could not run `claude`; the installed docs do not say). Until 3.3 shows it by a run, `PreToolUse` is not offered, as decision 8 requires.
- Hooks of several plugins are concatenated per event in install order; Claude runs matching hooks in parallel, so townhall does not promise an order.

### 6.3 MCP servers: `--mcp-config` carries them

Today the task agent's `--mcp-config=` is the one `townhall` hand-back server or nothing; `--strict-mcp-config` makes sure no other loads. `LaunchOptions` already has `mcp_config` (one JSON string) and `mcp_tools` (allowed without asking; they are the only `--allowedTools` entry when the type lists none, so they never turn a type without a tool list into one, `docs/daemon-api.md` Permission mode). The merge happens in `scheduler/agent_type.rs::launcher`, the one place that sets both today:

1. Start from townhall's own config (`mcp::task_config_json`) when the type may hand back, else from `{"mcpServers":{}}`.
2. For each selected plugin, for each server in its `.mcp.json`: rename the key to `plugin_<plugin>_<server>` (so tools are `mcp__plugin_<plugin>_<server>__<tool>`, the same form Claude gives plugin-loaded servers (from docs; the `mcp__<key>__<tool>` rule for `--mcp-config` keys is unverified); the key `townhall` is reserved and a plugin that uses it is refused), replace `${CLAUDE_PLUGIN_ROOT}`, and drop every `env` entry the plugin did not declare under `permissions.env`. Both `stdio` servers (claude spawns them as the agent's children in its own tmux pane, with that env) and `http` servers (a URL) are allowed.
3. Write the result to `<home>/tasks/<name>/mcp.json` (0600) and pass `--mcp-config=<path>`; `args.rs` and `claude.rs` already check the value with `plain()`, which accepts a path (only a leading `-` or control characters are refused). Reason: server `env` can hold a per-task API token (section 9.2).
4. `mcp_tools` gets `mcp__plugin_<plugin>_<server>__<tool>` for each tool the manifest approved (`permissions.claude.mcp.servers.<server>`: a list of tool names, or `["*"]` for all). Tools the manifest did not list are not allowed without asking: claude asks in the tmux pane, the task goes to `needsYou`, and the user can answer once or `always allow for <type>` (`POST /agents/{name}/allow`, which then adds that name to the type's `tools`; verify what `allow` does to a type that listed no `tools` before, since a first entry would turn on `--tools=` restrictions for it, and make that route leave such a type's list alone for plugin tools if so).
5. `--strict-mcp-config` stays. Nothing but townhall's server and the selected plugins' servers can load.

For the planner, steps 1 to 4 run in `runner/persona.rs` and `runner/args.rs` (`mcp_config` is already one string and `allowed_tools()` one list) with `claude.planner = true` plugins only and only tools marked `readOnly` in the manifest (`"servers": { "notes": { "tools": ["search_notes"], "readOnly": true } }`); `DENIED_TOOLS` still applies to built-in tools.

### 6.4 Lifecycle and limits

- The set is fixed at launch: enabling, disabling or updating a plugin affects the next launch and the next resume (the files are rewritten on `resume`), never a running claude. The plugin page says so.
- A selected plugin that is broken (files missing, manifest no longer valid) does not fail the task silently: the launch fails with `plugin <id>: <reason>` as `fail_start` does for other start errors, so the user sees it, and a toggle `skip broken plugins` is not offered.
- Tokens are per task and revoked when the task leaves `working`/`needsYou` for a final state (section 9.2).
- A plugin's MCP server writes to its data folder (`TOWNHALL_PLUGIN_DATA`), not to the worktree. Whatever else it does is the agent's permission mode's business, as for any MCP tool.

## 7. UI extensions

### 7.1 Slots (real locations in `web/src`)

| slot | where | component to extend | shape |
|------|-------|---------------------|-------|
| `sidebar.nav` | the nav list under the logo | `shell/Sidebar.tsx` `Nav` | an item (glyph + label) that opens a plugin page |
| `sidebar.foot` | above the settings button, next to `UsageMeter` | `Sidebar` foot (`Shell`'s `sidebarFoot` prop already exists) | iframe, fixed height 24 to 120 px |
| `page` | a full page | new route `/p/:plugin/:page` in `router/match.ts` (`PATHS`, `ROUTES`), rendered in `Shell` | iframe fills `th-main` |
| `session.section` | a section of the conversation's session panel | `views/conversation/session/SessionPanel.tsx` (after `context`, a `<h4 className="th-section">`) | iframe, height from the manifest, `ui.resize` up to 400 px |
| `task.section` | below the open task, next to proof | `SessionPanel.tsx` where `ProofSection` is shown for `openName` | iframe given `task` |
| `composer.action` | the button row of the composer | `composer/ComposerTools.tsx` (the `tools` node between `+ attach` and the help text) | a declarative button (glyph, label), not an iframe; it fires `composer.action` to the plugin (a hook-less message to its UI or a popover iframe) and may insert text with `composer.insert` |
| `settings.group` | a group on the settings page | `views/settings/SettingsPage.tsx` (the list `AgentsGroup ... AboutYouGroup`) | the group generated from the plugin's settings schema; plus an optional iframe under it |
| `agents.detail` | the agent type detail | `views/agents/AgentTypeDetail.tsx` | later |
| `control.panel` | the control center | `views/control/ControlCenterPage.tsx` | later |

Composer actions are buttons, not iframes, because the row is a toolbar and an iframe per button is heavy and awkward; an action that needs a form opens one popover iframe.

A slot is a named, typed place in a host component that renders `<PluginSlot name="session.section" context={{ conversationId }} />`; without enabled plugins it renders nothing and the app is byte-for-byte what it is today. New slots are added by changing the host, never by plugins.

### 7.2 Sandbox

- `<iframe sandbox="allow-scripts" referrerpolicy="no-referrer" allow="">`: no `allow-same-origin` (the frame's origin is opaque, so it cannot read townhall's `localStorage`, cookies or call `/api` with the app's origin), no `allow-forms`, `allow-popups`, `allow-top-navigation`, `allow-modals`.
- Decision (phase 4.1): the frame is served, not `srcdoc`. The plugin's HTML and assets are served by the daemon from the installed plugin folder, `GET /api/plugins/{id}/ui/{path}` (`routes/plugins.rs::ui_file`, resolution in `townhall-core` `plugins/ui.rs`), with `content-security-policy: sandbox allow-scripts; default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'none'; frame-ancestors 'self'`, `x-content-type-options: nosniff`, `cache-control: no-store` and `access-control-allow-origin: *`. `connect-src 'none'` means the frame cannot fetch anything, the daemon included: every call goes through the host. The `sandbox` directive in the header makes the document opaque even when the URL is opened directly in a tab, not only inside the host's `<iframe sandbox>`.
- Which files: only for an enabled plugin, only a file named by a `contributes.ui` `entry` or, when the entry sits in a folder, a file under that folder (`ui/panel.html` serves `ui/**`; a root-level entry serves only itself, so a plugin's manifest, hooks and scripts are never reachable). A path with an empty, `.` or `..` part, a backslash or a control character, any symlink on the way, a non-regular file, a file over 2 MiB or an extension outside a fixed list (html, js, mjs, css, json, svg, png, jpg, gif, webp, woff, woff2, txt) is a 404, the same answer as a missing file.
- Why served and not `srcdoc`: a `srcdoc` frame has no URL of its own, so `script-src 'self'` has nothing to match and the host would have to inline every script (which the plugin's CSP then needs `'unsafe-inline'` for) and rewrite assets; a served page keeps scripts as files under `script-src 'self'`, which per the CSP spec resolves against the response URL's origin even when the document is sandboxed (not yet confirmed in a real browser, see below). The sub-resource loads are cross-origin from an opaque origin, which is why that route alone answers `access-control-allow-origin: *` (needed for module scripts and fonts; plain scripts and styles load without it).
- What is verified and what is not: headers, the file allow list, traversal and symlink refusal and the iframe attributes are covered by tests (`crates/townhall-daemon/tests/plugin_ui.rs`, `web/src/plugins/PluginFrame.test.tsx`), with a fixture in `crates/townhall-daemon/tests/fixtures/ui-plugin`. That the page's script really runs under `script-src 'self'`, that `fetch('/api')` and `localStorage` fail inside it, and that the Tauri webview (WebKit) allows the frame are not tested by anything automated; the fixture's `panel.js` reports what it sees to the host so a manual or e2e run can check. If WebKit refuses the served frame, the fallback is `srcdoc` with the entry HTML fetched by the host and assets inlined.
- Host page CSP: `src-tauri/tauri.conf.json` had `default-src 'self'`, which also governs `frame-src`; `frame-src http://127.0.0.1:7777` is now in both `csp` and `devCsp`. In the browser dev setup the Vite proxy serves `/api/plugins/...` same-origin; the frame is built by `web/src/plugins/PluginFrame.tsx` (`sandbox="allow-scripts"`, `referrerpolicy="no-referrer"`, `allow=""`).

### 7.3 postMessage protocol

All messages are `{ th: 1, id?, type, ... }`. The host checks `event.source === iframe.contentWindow` and ignores everything else; the frame cannot tell who else is listening, so the host never sends anything the plugin is not scoped to read. The host sends with `targetOrigin "*"` (opaque origin) for that reason only.

- Frame to host, first: `{ type: "hello", apiVersion: 1 }`. Host answers `{ type: "init", slot, context, theme: { resolved, tokens }, plugin: { id, version }, scopes }` where `context` is the slot's (`conversationId`, `task`, ...) and `tokens` the resolved `--th-*` values, so the frame can paint itself in the app's colors; `theme` events follow when the theme changes.
- Frame to host requests, answered with `{ type: "response", id, result | error }`:
  - `api.call { method, path, body? }`: forwarded to the plugin API (section 9) with the plugin's token, only paths under `/api/plugin/v1/`, only allowed by the plugin's scopes. The host holds the token; the frame never sees it.
  - `events.subscribe { topics }`: the host forwards `ServerEvent`s (`/api/events`) the plugin's scopes allow (`taskUpdated` needs `tasks:read`, `entryAppended` needs `conversations:read`, ...) as `{ type: "event", ... }`.
  - `ui.resize { height }` (clamped to the slot's limits), `ui.toast { text, tone }` (plain text, 200 characters), `ui.navigate { path }` (townhall routes only, `PATHS`), `composer.insert { text }` (composer slot only; inserts into the draft, never sends), `clipboard.write { text }` (only from a user gesture the host saw in the last second).
- Rate limit: 50 messages per second per frame, then the frame is paused and a notice shown; a frame that throws or never says `hello` within 5 s is replaced by a `plugin <id> failed to load: <why>` line with a retry.
- Decision (phase 4.2): the host is `web/src/plugins/PluginSlot.tsx` (frame, hello timeout, pause and failure notices), `bridge.ts` (the protocol, no React), `scopes.ts` (the route and scope table of section 9.2; a path outside `/api/plugin/v1/`, with a dot or empty segment, or a route or method the plugin API lacks is refused without a request) and `hostSession.ts` (the token, in memory only). The daemon adds `GET /api/plugins/ui` (the `contributes.ui` entries of enabled plugins, `plugin`, `slot`, `title`, `entry`, `height`) and `POST /api/plugins/{id}/host-token`, which issues a plugin token and answers `{ plugin, version, token, scopes }`; it needs an `Origin` header of an allowed web origin (a script or hook without one gets `403`), and the CORS policy now allows the `Authorization` header so the host can call the plugin API from the desktop shell. `events.subscribe` needs the `events` scope and the scope of every topic asked for (`taskUpdated` and `agentUpdated` need `tasks:read`, `entryAppended` and `conversationUpdated` need `conversations:read`); it replaces the frame's earlier subscription. `ui.resize` is clamped to 24 to 400 px, `sidebar.foot` to 24 to 120 px. `composer.insert` and `clipboard.write` are not part of 4.2 and answer an unknown-type error.
- Decision (phase 4.4): the SDK is `web/src/plugins/sdk/townhall-plugin-sdk.js` (about 110 lines, no dependencies, no build step, tested by `web/src/plugins/sdk.test.ts`). It is not served by the daemon: a frame may only load files of its own plugin folder (section 7.2), so the author copies it into the plugin's `ui/` folder and loads it with `<script src="townhall-plugin-sdk.js">`. The protocol, not the SDK, is the contract. See section 14 for its API.

## 8. Marketplaces, install, state

### 8.1 The built-in official marketplace

- Name `townhall-official` (reserved; adding another marketplace with that name is a `409`). Source: `github` repo `WilliamThogersen/townhall-plugins` (the owner of `origin`, `https://github.com/WilliamThogersen/townhall.git`; created, still empty and needs setup, see section 12, item 1).
- It is compiled in as a constant (`plugins::OFFICIAL`: name, source, and a pinned sha of its index), together with a snapshot of its `marketplace.json` (`include_str!`), so a first start and an offline start list its plugins with no network. On first use or on `refresh` townhall fetches the repo and fast-forwards; the index is accepted only if it parses, and plugins are still pinned by sha in the index (section 8.4).
- It cannot be removed, only hidden (`enabled: false` on the marketplace row). Official does not mean consent is skipped: every install shows the permission prompt. Official plugins differ only in having the `official` badge and in being the default result of search.
- First-party plugins that need no code (themes, starter prompts, a few agent types such as the `tester` and `reviewer` types the repo's own workflow uses) go there first.

### 8.2 Custom and private marketplaces

`POST /api/marketplaces { source, name?, account? }` with `source` one of: a GitHub or GitLab link, `owner/repo` (GitHub shorthand), any git url, or an absolute local folder (a git repo or a plain folder with a `marketplace.json`, for authors; not synced). The flow reuses what exists for adding repos (`repos::source` parses links, `repos::clone::clone_or_reuse` clones, `repos::auth_env` gives the login shell's `PATH`, `SSH_AUTH_SOCK`, `GIT_SSH_COMMAND`, no prompts: `GIT_TERMINAL_PROMPT=0`, ssh `BatchMode=yes`), so a private marketplace works over ssh with the user's key loaded, or over https with their credential helper, exactly like a private repo does today (`README.md`, private repos). Errors reuse the `authentication failed: ...` messages.

### 8.3 Using the forge account already registered

The `forgeAccounts` setting and `repos::accounts::Accounts::pick(forge, host)` already know which login to use for a host. For a marketplace (and the plugin repos its entries point at) on a host that has a registered account:

- the account is the marketplace row's `account` login if set, else `Accounts::pick`'s first registered one for that forge and host, else none (the user's own git credentials);
- for https remotes, the token is fetched when the child is spawned (`gh auth token --user <login> --hostname <host>`, `glab config get token --host <host>`, the same as `open_pr`) and given to `git` through the environment, never argv and never a file: `GIT_CONFIG_COUNT=1`, `GIT_CONFIG_KEY_0=http.extraheader`, `GIT_CONFIG_VALUE_0=Authorization: Basic <base64(x-access-token:<token>)>` for GitHub (`PRIVATE-TOKEN: <token>` for GitLab), only for that one `git` process. Nothing is stored and nothing is logged (`redact`);
- ssh remotes ignore the account and use the user's key;
- a plugin in a private marketplace may point at another private repo; it is cloned with the same account, and if that account cannot read it the install says so (`repository not found or not readable as <login>`), it does not retry with other accounts.

`GET /api/forge-accounts/detected` and the accounts settings group stay the only place accounts are managed.

### 8.4 Install, enable, update, pin, uninstall

States per plugin: `available` (listed in a marketplace) → `installed` (files in the cache, permissions approved, not active) → `enabled`.

- Install `POST /api/plugins { plugin: "name@marketplace", approve: <hash of permissions> }`: resolve the entry's source to a sha (the entry's `sha`, else its `ref`, else the default branch), clone into the cache, validate the manifest and every contributed file (section 2.3 rules), compute the permission list and its hash, and refuse unless `approve` equals that hash. `GET /api/plugins/preview?plugin=...` returns the same list so the web can show it first; the hash makes "the user approved exactly this" true even if the repo moved between the two calls. Install runs nothing from the plugin: no `postinstall`, no build, no `git` hooks (`git -c core.hooksPath=/dev/null clone --no-recurse-submodules`, submodules are refused), and a plugin that needs dependencies vendors them.
- Enable and disable `PATCH /api/plugins/{id} { enabled }`. Install does not enable; the install flow offers "install and enable". Enabling starts nothing by itself (hooks run on events, MCP and Claude hooks only apply to later launches, the UI loads when its slot renders).
- Update `POST /api/plugins/{id}/update`: fetch, resolve the entry's now-current sha, compare. Same sha: nothing. New sha: build the new tree beside the old, validate it, compare permission hashes. Unchanged permissions: switch (`enabled` stays) unless the plugin is pinned. Changed (only additions count, removals are fine): the plugin stays on the old sha, keeps running, and shows `update to <sha> needs approval` with the diff of the list; approving installs it. There is no silent permission growth.
- Pinning: the marketplace entry's `sha` or `ref` is the author's pin. The user can also pin (`PATCH ... { pinned: true }`): updates are not offered. Auto update is off for everything by default (open question 4); `check for updates` runs on a manual `refresh` and once at startup, and only reports.
- Uninstall `DELETE /api/plugins/{id}`: disables, removes the cache tree, and keeps the plugin's data folder and settings unless `?purge=true`; a plugin type that tasks named is gone, those tasks fail to start with the message in section 3.1.
- Marketplace `refresh` is `git fetch` + fast-forward of its clone; a force-pushed history is accepted but each plugin stays at its stored sha until updated, which is the point of pinning by sha.

### 8.5 Where things live

On disk, inside the one folder townhall already owns (`Home`, `crates/townhall-core/src/home.rs`: add `PLUGINS_DIR = "plugins"` and `Home::plugins_dir()` next to `repos_dir()`), mode 0700:

```
~/.townhall/plugins/
  marketplaces/<marketplace>/                 the marketplace repo clone
  cache/<marketplace>/<plugin>/<sha>/         the plugin tree at that sha, read only after install
  data/<marketplace>/<plugin>/                the plugin's own writable folder (TOWNHALL_PLUGIN_DATA)
```

Per task: `~/.townhall/tasks/<name>/claude-settings.json` and `mcp.json` (section 6), next to `brief.md`. Nothing is ever written to the user's repo, as today.

State is in `townhall.db`, not in files (README: "Townhall keeps no config files"; one owner of the database). A new migration range, `0950` to `0999` (the daemon API doc lists "0950 and up: free, ask the orchestrator"; add the range to `store::migrate::RANGES` and a `store/unit_plugins.rs`), with:

| table | columns |
|-------|---------|
| `marketplaces` | `name` pk collate nocase, `source` (JSON), `account` (login or null), `enabled`, `official`, `head_sha`, `fetched_at`, `error` |
| `plugins` | `id` pk (`name@marketplace`, collate nocase), `marketplace`, `name`, `version`, `sha`, `source` (JSON as resolved), `enabled`, `pinned`, `permissions_hash`, `approved` (JSON, what the user approved), `installed_at`, `updated_at`, `error` |
| `plugin_settings` | (`plugin_id`, `key`) pk, `value` (JSON), `updated_at` |
| `plugin_storage` | (`plugin_id`, `key`) pk, `value` (text, at most 64 KB each, 5 MB per plugin) |
| `plugin_tokens` | `plugin_id`, `task` (null for the plugin's long lived one), `token_hash`, `created_at`, `revoked_at` |

Enabled state is `plugins.enabled`; the active theme is one `settings`-style row (`plugins.theme`, not a `SettingKey`, like `profile.text`). `EventBus` gets a `pluginsUpdated` `ServerEvent` so the web refetches (like `resync`).

## 9. Plugin API

### 9.1 Why a separate, versioned API

`/api/...` is the UI's contract and changes with the UI (`types.ts` mirrors the Rust types and moves with them; `docs/daemon-api.md` is "phase 2 contract"). Plugins need stability and scoping, so they get a curated subset under `/api/plugin/v1/`, same envelope `{ success, data, error }`, same camelCase, same error codes, plus `401` (no or unknown token) and `403` (token's scopes do not allow it). The handlers call the same `Store` and core functions as the routes they mirror, so behaviour is not duplicated.

Versioning: the version is in the path. Additive changes (new routes, new optional fields, new events) stay in `v1`; a removed or changed meaning is `v2` served beside `v1` while any enabled plugin declares `apiVersion: 1`. `GET /api/plugin/v1/info` returns `{ apiVersion, townhall, plugin, scopes }`. A change to `v1` needs a task that updates this document first.

### 9.2 Auth and scoping

- A token is 32 random bytes, hex. Two kinds: a plugin token (created at enable, valid while enabled) and a task token (created when a task launches with the plugin selected, for its MCP servers and Claude hooks, scoped to that task and valid until the task is `ended`, `merged` or `error`). Only the hash is stored (`plugin_tokens`); the clear text exists in the environment of the process townhall spawned or in a 0600 file in `<home>/tasks/<name>/` (never on a command line, because tmux and `ps` show those), and in the web host's memory for the UI (the frame never holds it, section 7.3).
- Header `Authorization: Bearer <token>`. A request from a browser origin carries no token (the UI goes through the host, which adds it), so a web page of any other origin cannot use a plugin's scopes.
- Scopes, declared in `permissions.api` and approved at install:

| scope | allows |
|-------|--------|
| `tasks:read` | `GET /tasks`, `/tasks/{name}`, `/tasks/{name}/artifacts`, `/agents` (the `AgentInfo` list), `/stats` |
| `tasks:create` | `POST /conversations/{id}/tasks` (the `create_task` input and checks: name, repo, agent type, `after`, `check`), which also writes a `tasksCreated` entry saying `created by plugin <id>`, so it is visible in the thread; the same rules as the planner's tool, and no `permission_mode` |
| `conversations:read` | `GET /conversations`, `/conversations/{id}` (entries, tasks), `/search` |
| `conversations:write` | `POST /conversations/{id}/messages` (entry author shown as `plugin <id>` and answered by the planner like a message; a plugin cannot post as the user) |
| `settings:read` | `GET /settings` (townhall's own, never the profile or forge accounts) |
| `profile:read` | `GET /profile` (only when `learnAboutMe` is on) |
| `library:read` | `GET /library/agents`, `/library/agents/{name}` |
| `library:write` | `PUT /library/agents/{name}` through the same ask the planner uses (`save_agent_type`: the user confirms the TOML first) |
| `storage` | `GET`, `PUT`, `DELETE /storage/{key}` of the plugin's own `plugin_storage` rows; plus its own settings |
| `events` | the `/events` websocket filtered to the other scopes (a task token gets only its task's events) |

- A task token has at most `tasks:read` and `conversations:read` limited to its task and conversation, plus `storage`; it never gets `tasks:create` or `conversations:write` unless the manifest asks for them under `permissions.claude.api`, shown separately on the prompt (an agent that is prompt injected could use them).
- Never exposed to plugins: repos' paths and registration, forge accounts or tokens, `PUT /settings`, `DELETE` of anything, `POST /agents/{name}/send` or `/allow`, the planner's session, and other plugins' storage.

### 9.3 Plugin MCP tools

For plugin authors who want a ready-made bridge, townhall may later ship an MCP server `townhall-api` (a thin stdio wrapper over `/api/plugin/v1/`) that a plugin lists in its own `.mcp.json`. It is not part of the first versions: the MCP servers a plugin ships can already call the API with `TOWNHALL_API_URL` and the task token.

## 10. Security and trust

### 10.1 What the user approves

On install the user sees one list, built from the manifest and checked against the files (section 2.3), grouped by what it can do:

- runs programs on this machine, as you: townhall hooks (events listed) and Claude MCP servers (commands listed);
- changes what agents do: Claude hooks (events listed; `PreToolUse` flagged), MCP servers and tools, prompts into briefs or the planner;
- reads or writes townhall data through the API (scopes), including for agents (`permissions.claude.api`);
- shows UI in these places;
- reads these environment variables.

The approved list is stored with its hash (`plugins.approved`, `permissions_hash`); any addition needs a new approval (section 8.4). Official marketplace plugins get the same prompt.

### 10.2 What runs where

| part | runs as | in | trust boundary |
|------|---------|----|----------------|
| manifest, TOML, CSS, prompts | parsed by the daemon | the daemon | strict parsers with caps; no execution; theme limited to `--th-*` |
| townhall hooks | `sh -c`, the user's account | child of the daemon, own process group | none beyond the env allow list, timeout, output cap and fail-open rules; this is the user's authority |
| Claude hooks | the user's account | child of the agent's `claude` | the agent type's opt-in and the plugin's declared events |
| Claude MCP servers | the user's account | child of the agent's `claude` | opt-in, declared servers, declared env, tool allow list; unlisted tools ask |
| UI | the webview | opaque-origin sandboxed iframe | no same-origin, CSP `connect-src 'none'`, calls only through the host's scoped proxy |

### 10.3 What is not a sandbox (said plainly)

Townhall hooks, Claude hooks and MCP servers are programs running with the user's rights. The permission list is consent and a way to scope API access and Claude's tool permissions, not an OS sandbox: a malicious hook can read `~/.ssh` or call the unauthenticated legacy `/api/*` on `127.0.0.1:7777` directly, ignoring its token's scopes. The daemon's lack of authentication on `/api/*` is a standing fact of today's design (any local process can use it), and plugin tokens do not change it. Two consequences the plan accepts and states to users: install only plugins you would run as a shell script; and the scopes protect against honest mistakes and against a compromised UI frame or agent, not against a hostile hook.

Mitigations that are in scope: an env allow list for hooks (no ambient secrets, no forge tokens), per-event timeouts, process group kills, a circuit breaker, sha pinning, permission growth requiring approval, no code run at install, no auto update by default, install-time path and symlink checks, size caps, log redaction (`redact`) and one log target for everything a plugin does (`townhall::plugin`, with plugin id and event, `docs/logging.md`). A follow-up (open question 3) is token auth on all `/api` mutations, which would make the scopes binding for hooks too, and optionally `sandbox-exec` or a container for hooks.

### 10.4 Containing a bad plugin

- Kill switch per plugin (`PATCH enabled: false`; takes effect on the next event, launch and slot render), and for all: `TOWNHALL_PLUGINS=off` in the environment starts the daemon with every plugin disabled (also a toggle in settings), for the day a plugin breaks startup.
- A plugin never runs during daemon start-up before the store is open, and a plugin that throws, hangs, floods output or exits non-zero five times in a row is paused (section 4.2).
- Running agents keep their launch-time config; disabling a plugin does not kill tmux sessions. Its task token is revoked at once, so its MCP servers and hooks lose API access immediately even in a running agent.
- A removed plugin leaves a line in the thread of any task that named one of its types and the log keeps every event it handled (`plugin.hook_run` with exit code and duration, never the payload).

## 11. Phasing

Each task is one agent task: one crate area, tests first, no UI and Rust together unless said. Order is changed from the planner's proposal in one place, with a reason: the smallest API (tokens, scopes, a few reads, `create task`) moves from phase 4 to phase 2, before hooks. Hooks and MCP servers are much less useful without a way to call back (a hook that cannot read the task or create one is a notifier), the UI needs the same API anyway, and building it once with tokens early makes every later phase use it. Phase 4 then is only the UI.

### Phase 1: manifest, marketplace, install, agent types

- 1.1 `plugins::manifest` and `plugins::marketplace` (pure parsing, no I/O beyond reading given strings). Accept: parses the real `marketplace.json` and `plugin.json` fixtures copied from `~/.claude/plugins` (three marketplaces, four plugins) and a townhall manifest; refuses unknown keys, `..` paths, absolute paths, bad names, oversize input; `source` forms of section 2.2 decode; one test per refusal.
- 1.2 Store: migration `0950`, `RANGES` entry, `Home::plugins_dir`, `Store` functions for `marketplaces` and `plugins`. Accept: round trip, unique ignoring case, `townhall.db` from before opens, a database with the new migration refuses on an older build as today.
- 1.3 Marketplace add, refresh, list, remove over git (`routes/marketplaces.rs`; `GET/POST/DELETE /api/marketplaces`, `POST /api/marketplaces/{name}/refresh`), the built-in `townhall-official` constant with its offline snapshot. Accept: add from a local fixture repo; a second add of the same name is `409`; `townhall-official` cannot be removed or reused; offline start lists the snapshot; a private https remote gets the token via `GIT_CONFIG_*` env and the token is not in any argv (test with a fake `git` script like the fake `claude` and `gh` the tests already use).
- 1.4 Install, enable, disable, update, uninstall with the permission hash (`/api/plugins`, `preview`). Accept: install needs the right hash, a wrong hash is `400`; install never runs a file from the plugin (test with a `postinstall` that would write a marker); update with added permission stays on the old sha; pinned plugin is not updated; uninstall removes the cache and keeps data.
- 1.5 Agent type overlay: `AgentDef.plugins`, `origin`, plugin types in `Store::agent_types()`, `PUT/DELETE` refused for them, the launch of a plugin type. Accept: a task of `<plugin>-<type>` starts through the unchanged `launcher`; a disabled plugin makes it fail with the plugin named; a plugin type named `default` is skipped; types saved before still load.
- 1.6 Web: plugins page (marketplaces, available, installed, install with the permission prompt, enable, update), `PluginsProvider` data (as `web/src/data` resources), agents page badge. Accept: component tests for the prompt and the states; no plugin means no new element outside the page and one nav item.
- 1.7 Declarative: prompts (`starters`, `brief`, `planner`), plugin settings (`plugin_settings`, generated group), theme with the strict CSS parser. Accept: each refusal in section 3.4 has a test; the brief gets the plugin section; no plugin means unchanged `brief.md` and unchanged planner prompt (byte equal).

### Phase 2: plugin API v1 and townhall hooks

- 2.1 Token store and `/api/plugin/v1/info`, `tasks:read`, `conversations:read`, `settings:read`, `storage`. Accept: no token `401`, a scope missing `403`, task token only sees its task, hash only stored, revoked token `401`.
- 2.2 `tasks:create`, `conversations:write`, `library:read`, `library:write` (through the ask). Accept: a created task shows the plugin in the thread; no route takes a permission mode; `library:write` does not save before the user's yes.
- 2.3 `plugins::hooks` runner (spawn, env allow list, stdin payload, timeout, process group kill, output cap, exit-2 block, fail-open, circuit breaker) with the notify-only events `task.started`, `task.needs_you`, `task.ended`, `task.failed`, `task.merged`, `pr.opened`, `message.reply`, `conversation.*`, `repo.added`, `daemon.started`. Accept: a fake hook receives the payload; a hanging hook is killed at the timeout and the scheduler pass is not delayed; five failures pause the plugin; env has no `ANTHROPIC_*`.
- 2.4 Blocking events: `task.created`, `task.before_start`, `task.check`, `task.handed_back`, `pr.before_open`, `message.before_turn`. Accept: a block keeps the task `queued` with `blocked` set and the planner is told once; amend appends to `brief.md` within caps; added checks run and fail the verify loop like the task's own; `failClosed` behaves per section 4.2; unchanged behaviour with no plugin.

### Phase 3: Claude hooks and MCP

- 3.1 Spike and verification (a doc and fixtures, no product code): with the installed `claude`, throwaway `HOME`, check `--settings=<file>` hooks with `--setting-sources=`; `--mcp-config=<path>`; naming of tools for a renamed server key; `PreToolUse` allow versus `--disallowedTools`; `${CLAUDE_PLUGIN_ROOT}` handling. Accept: results recorded in `docs/daemon-api.md` (Permission mode) with the claude version, and section 6 corrected where it was wrong.
- 3.2 MCP merge for task agents in `scheduler/agent_type.rs::launcher` (selection by `plugins`, key renaming, env filtering, `mcp.json` file, `mcp_tools` from the approved list). Accept: no selected plugin gives the identical argv (existing tests untouched); a selected one adds exactly its servers, tools and path; the `townhall` key cannot be used; a server env var not declared is dropped; `--strict-mcp-config` present.
- 3.3 Claude hooks via the generated `claude-settings.json` (`LaunchOptions` gets the settings value, `ISOLATED_SETTINGS` kept when none apply). Accept: with no hooks the argv is identical; with hooks `disableAllHooks` is absent and `autoMemoryEnabled:false` present; declared events only; file mode 0600 and rewritten on resume; tokens never on the command line.
- 3.4 Task tokens in the agent env and revocation on final state. Accept: an MCP server started by a fake claude can call `tasks:read` for its task and nothing else; revoked after `ended`.
- 3.5 Planner opt-in (`default` type `plugins`, `claude.planner`, `readOnly` tools only). Accept: a plugin without `claude.planner` never reaches `runner/args.rs`; a tool not marked read only is absent from `--allowedTools`; `DENIED_TOOLS` unchanged.

### Phase 4: UI slots

- 4.1 Spike: sandboxed iframe serving (CSP, opaque origin, `script-src 'self'`, `frame-src` in `tauri.conf.json`) in the browser and in the Tauri webview. Accept: a fixture plugin page renders and cannot reach `/api` or `localStorage`; the decision (served or `srcdoc`) is written here.
- 4.2 `PluginSlot` host, the postMessage protocol and the scoped proxy (`api.call`, `events.subscribe`, `ui.resize`, `ui.toast`, `ui.navigate`), `GET /api/plugins/{id}/ui/*`. Accept: wrong `event.source` ignored; a path outside `/api/plugin/v1/` refused; a call needing a scope the plugin lacks refused; rate limit pauses the frame; no plugin renders nothing.
- 4.3 Slots one by one, each its own small task: `sidebar.nav` and `page` (route `/p/:plugin/:page`), `session.section` and `task.section` (`SessionPanel.tsx`), `settings.group` iframe part, `sidebar.foot`, then `composer.action` (declarative buttons and `composer.insert`). Accept per slot: renders with a fixture plugin, receives its `context`, disappears when the plugin is disabled, existing tests of that component untouched.
- 4.4 The SDK script and a sample plugin in the official marketplace repo, as the end to end test of phases 1 to 4. Accept: one plugin with a hook, a Claude MCP server, an agent type and a UI section installs from a fixture marketplace and works in a test run with fake `claude`.

## 12. Decisions

The user accepted every recommendation below on 2026-11-09. Each item states the decision and keeps its recommendation.

1. Official marketplace repo. Decided: a new public repo `WilliamThogersen/townhall-plugins` (or an org if one is planned), index pinned by sha in the binary, first content small and declarative (a theme, starter prompts, `tester`/`reviewer` types). Moving it later is a one line change plus a migration of the stored source.
2. Where townhall-only manifest keys live. Decided: the separate `.townhall-plugin/plugin.json` of section 2.1, so one repo can serve Claude Code and townhall. The alternative, a `townhall` key inside `.claude-plugin/plugin.json`, is simpler for authors with one file but depends on Claude Code tolerating unknown keys (to verify).
3. Authentication of the daemon's own `/api`. Today anyone on the machine can call it, so plugin scopes bind only honest callers (section 10.3). Decided: do not block the plugin work on it, but plan a separate task that puts a per-start token on every non-GET `/api` route (the web and the desktop app get it at start), which makes the scopes binding. Decide before phase 2 ships to anyone but you.
4. Auto update. Decided: off for everything in v1 (check and report only), on later as a per-plugin switch that still requires approval for any permission growth.
5. Plugin changes to townhall's own settings. Decided: no in v1; a plugin may ship an `applyRecommended` suggestion the user clicks, later.
6. Failing open or closed for hooks. Decided: open by default with a per-hook `failClosed` that the install prompt shows (section 4.2), because one broken plugin should not stop every task, and a security gate can opt in.
7. Skills and slash commands from plugins into agents. Decided: not in phase 3. Agents today keep claude's own skills (only the planner has `--disable-slash-commands`), so enabling plugin skills means `--plugin-dir` or `--add-dir` and a second isolation question; do it after hooks and MCP prove out, as phase 5 with its own verification task.
8. Claude hooks that can approve tool calls (`PreToolUse`). Decided: allowed, flagged in the prompt, and only after phase 3.1 shows `--disallowedTools` and a type's deny rules still win; otherwise leave `PreToolUse` out of v1.
9. Plugin agent type naming `<plugin>-<name>`. Decided: keep it, since `create_task`'s pattern excludes `:` and `/`; the agents page can show the plugin and the short name separately.
10. Planner access to plugin MCP tools. Decided: off by default and read only tools only (section 6.1); it keeps the planner's "can plan, never write" rule intact. Say no to it entirely if you would rather the planner never changes with plugins.
11. A permission mode for a plugin type. Decided: refuse `bypassPermissions` (section 3.1); the user can copy the type and edit it.
12. OS sandbox for hooks and MCP servers (macOS `sandbox-exec`, Linux bubblewrap). Decided: not in v1; say "runs as you" clearly instead, and revisit once the daemon token (question 3) is in.

## 13. Notes for the builders

- Every build task above starts by reading `docs/daemon-api.md` for the files it touches and ends by updating it (this document stays the design; `docs/daemon-api.md` becomes the contract as each part lands). Plugin routes, tables and the events go there, in the same tables.
- Code that is added must follow the repo's rules: no new code comments (CI runs `no-added-comments`), one `--name=value` argv element per option, no value starting with `-` (`plain`), `redact` on anything logged.
- `web/src/api/types.ts` mirrors every new Rust type field for field, with its type test.

## 14. Writing a plugin

A plugin is a folder in a git repo; a marketplace is a git repo with a `marketplace.json` that lists plugins. The complete working example is `crates/townhall-core/tests/fixtures/plugins/sample-marketplace/` (marketplace) with `plugins/notes/` (plugin): one townhall hook, one Claude MCP server, one agent type and one UI section. Copy it and rename.

### 14.1 Layout and manifest

```
my-plugin/
  .townhall-plugin/plugin.json
  townhall-hooks.json
  bin/on-started.sh
  agent-types/note-taker.toml
  mcp/servers.json
  mcp/server.mjs
  ui/session.html
  ui/session.js
  ui/townhall-plugin-sdk.js
```

`.townhall-plugin/plugin.json` names every part (section 2.3). Every path is relative to the plugin root and stays inside it. Unknown keys are an install error, so a typo is caught at once. The smallest useful manifest:

```json
{
  "name": "notes",
  "version": "1.0.0",
  "apiVersion": 1,
  "permissions": {
    "api": ["tasks:read", "storage"],
    "hooks": ["task.started"],
    "claude": { "mcp": { "servers": { "notes": ["search_notes"] } } },
    "ui": ["session.section"]
  },
  "contributes": {
    "agentTypes": ["agent-types/note-taker.toml"],
    "hooks": "townhall-hooks.json",
    "ui": [{ "slot": "session.section", "title": "notes", "entry": "ui/session.html", "height": 120 }]
  },
  "claude": { "mcp": "mcp/servers.json" }
}
```

### 14.2 Permissions

`permissions` is what the user approves when installing (the install prompt lists it and its hash must be sent back), and it is checked against use: a hook on an event, an MCP server or a UI slot that is not declared is an install error. Declare only what you use. `api` scopes bound what the plugin's token may call (section 9.2); `hooks` lists townhall events (section 4.1); `claude.mcp.servers` maps each server name in the MCP file to the tool names townhall may allow without asking (`["*"]` for all); `ui` lists slots; `env` names the daemon environment variables the plugin's hooks and servers may read. An update that adds a permission stays on the old version until the user approves again.

### 14.3 Hooks

`townhall-hooks.json` maps an event to commands (section 4). The command runs as `sh -c` with the plugin's data folder as working directory, the payload as JSON on stdin and `TOWNHALL_EVENT`, `TOWNHALL_PLUGIN`, `TOWNHALL_PLUGIN_ROOT`, `TOWNHALL_PLUGIN_DATA`, `TOWNHALL_API_URL` and `TOWNHALL_API_TOKEN` in its environment. `${TOWNHALL_PLUGIN_ROOT}` in the command is replaced by the plugin's folder. Exit 0 continues, exit 2 blocks where the event allows it (stderr is the reason), anything else fails open. The sample's `bin/on-started.sh` writes the payload of `task.started` to its data folder.

### 14.4 Agent types and Claude MCP servers

An agent type is the `AgentDef` TOML of section 3.1; it is listed as `<plugin>-<name>` (`notes-note-taker`). A type a plugin ships selects that plugin's Claude-side parts by itself; a user type must name the plugin in `plugins = [...]`. The MCP file is Claude Code's `.mcp.json` shape (`mcpServers`) at the path `claude.mcp` names; `${CLAUDE_PLUGIN_ROOT}` is replaced by the plugin's folder, and townhall renames each server to `plugin_<plugin>_<server>`, so the sample's `search_notes` is `mcp__plugin_notes_notes__search_notes`. The config is written to the task's folder, never the command line. Each stdio server gets `TOWNHALL_API_URL` and `TOWNHALL_TOKEN`, a token for that task only (at most `tasks:read`, `conversations:read` and `storage`, revoked when the task ends). Other environment variables reach the server only if declared under `permissions.env`.

### 14.5 UI and the SDK

A `contributes.ui` entry is an HTML file in a folder of your plugin; the daemon serves that folder (and only it) into a sandboxed frame (section 7.2): scripts must be files, nothing can be fetched, every call to townhall goes through the host. Copy `web/src/plugins/sdk/townhall-plugin-sdk.js` into the folder and load it before your own script. It sends `hello` at load and defines `window.townhall`:

| call | does |
|------|------|
| `townhall.ready` | promise of the `init` message: `slot`, `context`, `theme`, `plugin`, `scopes` |
| `townhall.api.call(method, path, body?)` | `api.call`; resolves with the data, rejects with the host's error text. Paths are under `/api/plugin/v1/` and need a scope |
| `townhall.events.subscribe(topics, handler)` | `events.subscribe`; the handler gets each event of those topics |
| `townhall.ui.resize(height)`, `toast(text, tone?)`, `navigate(path)` | `ui.resize`, `ui.toast`, `ui.navigate` |
| `townhall.composer.insert(text)`, `onAction(handler)` | `composer.insert` and the `composer.action` click (composer slot) |
| `townhall.theme.onChange(handler)` | the `theme` message |

Every request returns a promise. You may skip the SDK and speak the protocol of section 7.3 directly.

### 14.6 Testing with a local marketplace

A marketplace is any local git repo, so no network is needed:

1. `git init` a folder holding `.townhall-plugin/marketplace.json` and your plugin folder (`"source": "./plugins/notes"`), then `git add -A` and commit. A change reaches townhall only once committed.
2. Add it: `POST /api/marketplaces {"source": "/path/to/folder"}` (the plugins page, "add marketplace").
3. Install: `GET /api/plugins/preview?plugin=notes@<marketplace>`, then `POST /api/plugins {"plugin": ..., "approve": <permissionsHash>}`, then `PATCH /api/plugins/<id> {"enabled": true}`. After a new commit, `POST /api/plugins/<id>/update`.
4. Create a task of your agent type. `crates/townhall-core/tests/plugin_e2e.rs` does these steps against a fake `claude` (`tests/scheduler_support`): it asserts the `--mcp-config` file and the task token, that the hook ran and wrote to its data folder, and that the UI files are served, and it runs the sample's MCP server over stdio. Copy its shape for your own plugin.
