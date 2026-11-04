# Plan: storage in SQLite, no config files

Status: planned, not built. Comes before phase 2 of [`build-brief.md`](../style-guide/build-brief.md), because 2.1 repos, 2.2 conversations and 2.3 tasks all store their data here.

## Goal

Townhall has **no config files**. Everything it stores lives in one SQLite database. Townhall never reads a config file at startup.

## Layout

```
~/.townhall/
  townhall.db                      every repo, setting, conversation, task, definition and run
  worktrees/<repo-key>/<agent>/    unchanged: one git worktree per agent
```

- One file, `~/.townhall/townhall.db` (or under `$TOWNHALL_HOME`).
- Nothing is written into the repo, as before.

## Rules

- **One owner.** The daemon is the only process that opens the database. The desktop app runs the daemon in process, and when port 7777 is taken it uses that daemon, so there is always one owner. The web ui and the desktop app read and write through the daemon's API.
- **Live agent state stays in git and tmux.** A row refers to an agent by name. Alive, branch and worktree are looked up from `git worktree list` and tmux when needed, never stored, so the database can never disagree with what is running.
- **Files only for blobs.** Attachments (2.6) stay files on disk, referenced by a row. Everything else is a row.

## Setup

- `rusqlite` with the `bundled` feature, so the app does not depend on the system's SQLite.
- `PRAGMA journal_mode = WAL`, `foreign_keys = ON`, `busy_timeout = 5000`.
- Calls run in `tokio::task::spawn_blocking` behind one connection. Write volume is low.
- The connection and migrations live in `townhall-core`, in a new `store` module, so the daemon and the tests share one implementation.

## Migrations

- Numbered SQL files, `migrations/0001_init.sql` and onward, embedded with `include_str!`.
- Applied in order at startup inside one transaction each, tracked with `PRAGMA user_version`.
- A database newer than the app refuses to open: `error: townhall.db is from a newer townhall`.
- A migration never edits an older migration file. Changes are new files.

## What moves into the database

| today or planned | becomes |
|------------------|---------|
| `~/.townhall/config.json` (chosen repo, desktop) | `repos` and `settings` tables |
| 2.1 repo registry, planned in `config.json` | `repos` table |
| 1.6 settings, saved in the browser for now | `settings` table, read and written through the daemon |
| 2.2 conversations, planned under `~/.townhall/conversations/<id>/` | `conversations`, `messages` and `events` tables |
| 2.3 tasks | `tasks` table |
| agents, workflows and runs | tables in [`agents-and-workflows.md`](agents-and-workflows.md) |

The unit that owns a table designs its columns and adds its migration. This plan only fixes the shared ones:

```sql
create table settings (
  key        text primary key,
  value      text not null,
  updated_at text not null
);

create table repos (
  name        text primary key,
  path        text not null unique,
  base_branch text not null,
  added_at    text not null
);
```

- `settings.value` is JSON, so a setting can be a string, number, boolean or list.
- `GET /api/settings` and `PUT /api/settings/:key` replace the browser storage from 1.6. `skip all` stays reachable only through this endpoint, as the build brief requires.

## Moving existing installs

1. On first start without `townhall.db`, townhall creates it and runs the migrations.
2. If `config.json` exists, its repo goes into `repos` and its settings into `settings`.
3. The file is renamed to `config.json.imported`, never deleted.
4. The older `settings.json` migration in `src-tauri/src/settings.rs` keeps working, now into the database.
5. Settings saved in the browser by 1.6 are sent to `PUT /api/settings/:key` once on the next load, then cleared from the browser.

## Unit

Add as unit 2.0 in the build brief, first in phase 2.

| unit | scope | needs |
|------|-------|-------|
| 2.0 storage | `store` module, connection setup, migrations, `settings` and `repos` tables, settings endpoint, `config.json` and browser settings import, README "where things live" update | nothing |

## Tests

- Migrations from an empty home, from a home with `config.json` and from one with the old `settings.json`.
- A newer `user_version` refuses to open.
- A second open while the daemon holds the database fails with a clear error.
- Settings round trip through the API, and `skip all` is only accepted there.
- Every test uses `TOWNHALL_HOME` in a temp folder.

## Ask the user before

- adding `rusqlite` with `bundled`: it compiles SQLite into the app, heavier than the brief's "small library" rule
- shipping the `config.json` import: it changes the `~/.townhall` layout, which the brief says to confirm first

## Open questions

1. Backups: should townhall keep a daily copy with `VACUUM INTO`, and how many?
2. Should settings be per repo as well as global, or global only?
