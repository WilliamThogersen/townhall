# Logging

Townhall writes structured JSONL logs that people, and townhall's own planner session,
can read back. Setup lives in `crates/townhall-daemon/src/logging.rs`; the desktop app and
the standalone daemon both call `logging::init` once at startup.

## Where

`<townhall home>/logs/`, which is `~/.townhall/logs` or `$TOWNHALL_HOME/logs` (created 0700).

| File | Written by |
| --- | --- |
| `daemon.YYYY-MM-DD.jsonl` | the standalone `townhall-daemon` |
| `app.YYYY-MM-DD.jsonl` | the desktop app, including its embedded daemon |

Two processes never share a file. Files rotate daily, at most 14 are kept, and the folder is
pruned to 200 MB (oldest first, never the newest file) at startup and then hourly.

Stderr also gets a human-readable layer at `warn` and above.

## Filter

`TOWNHALL_LOG` takes `EnvFilter` syntax (e.g. `info,townhall::http=debug`). Default:
`info,townhall=debug,townhall::git=info,townhall::tmux=info` (the per-command `cmd.run`
debug lines are noisy, so they stay off until you ask: `TOWNHALL_LOG=info,townhall::git=debug`).
It filters the files only.

## Line schema

One JSON object per line:

| Field | Notes |
| --- | --- |
| `timestamp` | RFC 3339, UTC |
| `level` | `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE` |
| `target` | the component, e.g. `townhall::http`, `townhall::task` |
| `event` | stable dotted name. **Required on every line.** |
| `message` | human text |

Optional fields, always with these names: `task`, `agent`, `conversation`, `repo`,
`request_id`, `error_kind` (from `Error::kind()`), `error`, `duration_ms`, `status`, `from`,
`to`, `tool`. Fields of the current span are included; the span list is not.

Emit with `tracing`, putting ids in fields rather than in the message:

```rust
tracing::warn!(target: "townhall::task", event = "task.launch_failed",
    task = %name, error_kind = e.kind(), error = %redact(&e.to_string()), "launch failed");
```

## Redaction and what never gets logged

- Never log prompt, message or transcript bodies, briefs or tool inputs. Log lengths
  (`prompt_chars`).
- Every free-form string (`error`, stderr, client-sent text) goes through
  `townhall_core::redact::redact`. It masks `sk-ant-…`, `ghp_…`, `github_pat_…`,
  `Bearer …`, `AKIA…` keys and the values of `token=`, `password=` and `secret=`
  (as `[REDACTED]`), then cuts the text to 500 chars.

## Tests

`logging::test_capture()` returns an in-memory capture and a guard for this thread:

```rust
let (capture, _guard) = townhall_daemon::logging::test_capture();
// ... run code ...
assert_eq!(capture.events("task.state").len(), 1);
```

Use a current-thread runtime when the code spawns tasks, since capture is thread-local.

## Event catalog

Later tasks add to this table as they emit events.

| Event | Target | Level | Fields |
| --- | --- | --- | --- |
| `daemon.start` | `townhall::daemon` | info | `version`, `repo`, `bind` |
| `app.start` | `townhall::app` | info | `version` |
| `panic` | `townhall::panic` | error | `message`, `location`, `backtrace` (≤4 KB) |
| `logs.pruned` | `townhall::logging` | info | `deleted` |
| `logs.prune_failed` | `townhall::logging` | warn | `error` |
| `task.created` | `townhall::task` | info | `task`, `conversation`, `repo`, `agent_type`, `after`, `prompt_chars` (never the prompt) |
| `task.state` | `townhall::task` | info | `task`, `conversation`, `from`, `to` (every change through `Store::update_task`) |
| `task.needs_you` | `townhall::task` | info | `task` |
| `task.failed` | `townhall::task` | warn | `task` (state became `error`) |
| `task.ended` | `townhall::task` | info | `task` |
| `task.launched` | `townhall::task` | info | `task`, `repo`, `duration_ms`, `worktree` |
| `task.launch_failed` | `townhall::task` | warn | `task`, `repo`, `error_kind`, `error` |
| `task.merged` | `townhall::task` | info | `task`, `base` |
| `scheduler.off` | `townhall::task` | warn | `error_kind`, `error` |
| `scheduler.pass_warning` | `townhall::task` | warn | `error` (once until it changes) |
| `cmd.run` | `townhall::git` or `townhall::tmux` | debug; warn on failure | `program`, `args` (first 2 only), `duration_ms`, `exit_code`, on failure `stderr` (redacted) or `error_kind`, `error` |
| `worktree.create` | `townhall::git` | info | `agent`, `base`, `path` |
| `worktree.remove` | `townhall::git` | info | `agent`, `path` |
| `fleet.repo_skipped` | `townhall::store` | warn | `repo`, `error_kind`, `error` |
| `repo.startup_not_registered` | `townhall::store` | warn | `path`, `error_kind`, `error` |
| `fs.tmp_remove_failed` | `townhall::store` | warn | `path`, `error` |
| `store.config_import_failed` | `townhall::store` | warn | `error_kind`, `error` |
| `store.import_repo_skipped` | `townhall::store` | warn | `path`, `error_kind`, `error` |
| `store.setting_ignored` | `townhall::store` | warn | `key`, `error` |
| `store.attachment_cleanup_failed` | `townhall::store` | warn | `path`, `error` |
| `store.agent_type_unreadable` | `townhall::store` | warn | `error_kind`, `error` |

Planned (see the logging plan): `http.request`, `http.error`, `ws.open`, `ws.close`,
`mcp.tool`, `task.launch_failed`,
`agent.state`, `agent.gone`, `agent.tool_error`, `agent.permission_denied`,
`turn.start`, `turn.end`, `turn.failed`, `claude.spawn`, `claude.exit`, `cmd.run`,
`daemon.mode`, `daemon.restart`, `web.error`,
`web.unhandled_rejection`, `web.render_error`.
