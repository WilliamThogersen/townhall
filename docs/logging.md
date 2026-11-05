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
`info,townhall=debug`. It filters the files only.

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

## Frontend reports: `POST /api/logs/client`

The web app forwards its errors here (`routes/logs.rs`). The daemon redacts them and
re-emits them with target `townhall::web`.

- Body: `{level, event, message, stack?, route?, request_id?, error_kind?, status?}`, or an
  array of those. `level` is `error`, `warn`, `info` or `debug`. `event` must be `web.*`
  or `api.*` (e.g. `web.error`, `web.unhandled_rejection`, `web.render_error`, `api.http`),
  so a client cannot pass for a daemon event. Unknown fields, a bad level or a bad event give
  `400` and nothing from that body is logged.
- Bodies over 8 KB give `413`.
- Rate limit: a token bucket of 30 reports per minute per daemon (a batch costs one per
  entry). Excess reports are dropped, still with `202`, and `web.dropped` is logged once per
  burst of drops.
- Every string field goes through `redact` (masked, cut to 500 chars).
- Response: `202` with `{success, data: {accepted, dropped}}`.

## Reading the logs back

`townhall_core::logs` reads `<home>/logs/*.jsonl` (`app.*` and `daemon.*` files only), newest
first. It stops at `since`, skips lines that are not valid log JSON, and adds `source`
(`app` or `daemon`, from the file name). It is pure and synchronous: async callers use
`spawn_blocking`. The caps live in the core functions: a `read_logs` line is cut to 300 chars
and every result stays under 16 KB of JSON (`truncated: true` when something was left out).

`since` is a duration (`90s`, `30m`, `24h`, `7d`, `2w`) or an RFC 3339 time; default `24h`.
`level` is a minimum; default `warn`.

### Planner tools (`mcp/logs.rs`)

- `read_logs` `{level?, component?, event?, task?, grep?, since?, limit?}`. `component` is a
  target prefix (`git` and `townhall::git` both work), `grep` a case-insensitive substring of
  the message or `error`, `limit` defaults to 50 (max 200). Returns
  `{lines, count, truncated}`; a line is `ts level target event message {source=.. task=..}`
  with the id fields that are set.
- `log_summary` `{since?, level?, top?}` (`top` defaults to 20, max 50). Returns `total`,
  `by_level`, `by_component`, every `panics` entry and the top recurring `groups`, each with
  `fingerprint`, `count`, `target`, `event`, `first_seen`, `last_seen`, up to 5 `tasks` and one
  redacted `sample`. When the result is over 16 KB the least frequent groups go first.

The system prompt tells the planner to start from `log_summary` when asked to improve
townhall, to create tasks for recurring groups and to cite the fingerprint in the brief. The
logs are plain files in `~/.townhall/logs/`, so Read and Grep work as well.

### Fingerprint

A group is `target` + `event` + the normalized message, hashed with FNV-1a 64 and shown as 16
hex chars. The hash is fixed so a fingerprint in a task brief keeps its meaning. Normalizing
replaces, in this order: the values of the entry's `task`, `agent`, `conversation`, `repo` and
`request_id` fields and names with a numeric suffix (`foo-1`) with `<name>`; quoted text
(`'x'`, `"x"`, `` `x` ``) with `<q>`; paths starting with `/`, `./`, `../` or `~/` with
`<path>`; uuids with `<uuid>`; hex (`0x1f`, or 7+ hex digits including a digit) with `<hex>`;
other digit runs with `<n>`. So `task foo-1 failed after 312ms` and
`task bar-9 failed after 18ms` share a fingerprint.

### HTTP mirror

`GET /api/logs` (same query as `read_logs`) and `GET /api/logs/summary` (same as
`log_summary`) in `routes/logs.rs`, with the usual `{success, data, error}` envelope. A bad
`since`, `level` or an unknown query parameter gives `400`.

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
| `web.error`, `web.unhandled_rejection`, `web.render_error`, `api.*` | `townhall::web` | as sent | `stack`, `route`, `request_id`, `error_kind`, `status` |
| `web.dropped` | `townhall::web` | warn | `limit_per_minute` |
| `mcp.tool` | `townhall::mcp` | debug when `ok`, warn otherwise | `tool`, `conversation`, `duration_ms`, `ok`, `error` (redacted, only when `ok=false`). Never the arguments or the result. |
| `mcp.task_note_failed` | `townhall::mcp` | warn | `task`, `conversation`, `error` |
| `mcp.status_no_transcript`, `mcp.status_no_steps`, `mcp.status_no_last_message` | `townhall::mcp` | warn | `task`, `error_kind`, `error` |
| `turn.start` | `townhall::runner` | info | `conversation`, `prompt_chars`, `attachments`, `task_event` (the turn answers a task event, not a user message) |
| `turn.end` | `townhall::runner` | info | `conversation`, `outcome` (`ok`, `failed` or `stopped`), `duration_ms`, `cost_usd` (when claude reports it) |
| `turn.failed` | `townhall::runner` | warn | `conversation`, `error` (redacted; claude's last stderr line or result text) |
| `turn.permission_denials` | `townhall::runner` | warn | `conversation`, `count`, `tools` (comma-separated names, never the tool input) |
| `turn.give_back_failed`, `turn.not_finished`, `turn.note_failed`, `turn.not_handed_back` | `townhall::runner` | warn | `conversation`, `error_kind`, `error` |
| `turn.stop_timeout` | `townhall::runner` | warn | `conversation` |
| `claude.spawn` | `townhall::runner` | info, warn when `ok=false` | `conversation`, `ok`, `resume`, and `error_kind`, `error` on failure |
| `claude.exit` | `townhall::runner` | info | `conversation`, `code` (absent when killed by a signal), `stopped`, `duration_ms` |
| `claude.kill_failed`, `claude.wait_failed`, `claude.read_failed`, `claude.stdin_closed`, `claude.output_stayed_open` | `townhall::runner` | warn | `error` (and `group` for a process group kill) |
| `repo.inference_failed`, `repo.answer_unreadable` | `townhall::runner` | warn | `conversation`, `error_kind`, `error` |
| `repo.linked_gone` | `townhall::runner` | warn | `conversation`, `repo` |
| `attachment.unreadable` | `townhall::runner` | warn | `attachment`, `path`, `error` |
| `notify.not_delivered` | `townhall::runner` | warn | `conversation`, `error_kind`, `error` |
| `recover.failed` | `townhall::runner` | warn | `error_kind`, `error` |

Planned (see the logging plan): `http.request`, `http.error`, `ws.open`, `ws.close`,
`task.created`, `task.state`, `task.launched`, `task.launch_failed`,
`task.merged`, `agent.state`, `agent.gone`, `agent.tool_error`, `agent.permission_denied`,
`cmd.run`, `worktree.create`, `worktree.remove`, `daemon.mode`, `daemon.restart`.
