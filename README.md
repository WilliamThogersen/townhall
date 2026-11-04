<p align="center">
  <img src="docs/style-guide/img/wordmark.svg" alt="/townhall" width="264">
</p>

<p align="center">
  talk to it. it turns the work into tasks,<br>
  and agents run them in their own git worktrees.
</p>

<p align="center">
  <code>rust</code> · <code>react</code> · <code>tauri v2</code> · <code>tmux</code> · built for 10 to 50 agents at once
</p>

---

## status

Early. The core runs agents, and the daemon now runs conversations that turn into tasks
across several repos, with agent states, attachments, steps and shipped stats. Their
interface comes next.

|   | part | what it does |
|---|------|--------------|
| ● | core | one git worktree and one tmux session per agent, adapters for `claude` and `shell` |
| ● | daemon | http and websocket api on `127.0.0.1:7777`: several repos, conversations, tasks, agent states, attachments, steps and shipped stats, live terminal snapshots |
| ● | desktop | tauri app with repo picker, tray and a notification when an agent ends |
| ◐ | interface | terminal-native redesign, specified in [`docs/style-guide`](docs/style-guide/README.md) |
| ◐ | conversations | talk to townhall; it finds the repo and creates tasks that agents pick up. works in the daemon, the interface comes next |
| ◐ | control center | one timeline for every agent in every conversation. agent states and steps work in the daemon, the timeline comes next |

`●` works · `◐` in progress · `○` planned

## goals

- spawn, supervise and observe many agents in parallel
- a git worktree per agent
- an adapter trait, so any agent CLI can plug in
- live terminal streaming to the ui
- custom behaviour lives in plugins, not in core

## run it

You need Rust, Node, git and tmux. Install the web dependencies once:

```sh
npm --prefix web install
```

### in a browser

```sh
npm run dev                  # from the repo root; agents work in the current directory
npm run dev -- /path/to/repo # or TOWNHALL_REPO=/path/to/repo npm run dev
```

Builds and runs the daemon (`127.0.0.1:7777`) next to the Vite dev server
(`http://localhost:5173`, proxies `/api`), so the UI opens in a normal browser
with hot reload and no Tauri. `Ctrl-C` stops both processes. Agents live in tmux
sessions named `townhall-<repo-key>-<name>` and keep running.

### as a desktop app (tauri v2)

The app wraps the same web UI and runs the daemon in-process on `127.0.0.1:7777`.

```sh
npm run tauri dev            # from the root (or inside web/)
npm run tauri build          # packaged app; add `-- --debug` for a faster debug bundle
```

Output lands under `target/`. How it behaves:

- **First launch** with no repo opens home on `add your first repository`. `add repository…`
  there and on the repos page opens a native folder picker; the folder must be the top
  folder of a git repo and is added through `POST /api/repos`, so the daemon keeps running.
  A repo chosen by an older version (saved in `~/.townhall/townhall.db`, or imported from an
  old `settings.json` or `config.json`) stays the daemon's startup repo.
- **Port 7777 taken** (a daemon running separately): the app uses that daemon, and
  the repo is whatever it was started with.
- **Closing the window** hides it to the tray; use "Quit" in the tray menu to exit.
  Quitting stops only the in-process server, never the agents (tmux owns them).
- **Notifications**: a native notification fires when an agent's session goes from
  alive to dead.
- **Network**: the UI talks to `http://127.0.0.1:7777/api` and `ws://127.0.0.1:7777/api`
  inside the app and to same-origin `/api` in a browser. The daemon allows CORS only
  for the desktop and Vite dev origins.

## where things live

Everything townhall creates lives in one folder, `~/.townhall` (override with
`TOWNHALL_HOME`; created with mode 0700). Nothing is ever written into your repo:
no `.townhall` folder, no `.gitignore` edits. Git itself only records the
worktrees under the repo's own `.git`. Townhall keeps no config files.

```
~/.townhall/
  townhall.db                                    every repo, setting, conversation and task (SQLite)
  townhall.lock                                  held while a townhall owns the database
  conversations/<id>/files/<attachment>/<name>   attachments
  tasks/<name>/brief.md                          a task's whole prompt, which its agent reads
  worktrees/<repo-key>/<agent>/                  one git worktree per agent, branch townhall/<agent>
  config.json.imported                           an older config.json, after its one-time import
```

- One owner: the daemon opens the database, and a second townhall on the same
  home fails with `townhall.db is in use by another townhall`. The desktop app runs
  the daemon in-process, or uses the one already on port 7777.
- An existing `config.json` is imported once (its repo, nothing else) and renamed
  to `config.json.imported`, never deleted.
- `<repo-key>` is the repo folder name plus a short hash of its canonical path,
  for example `myapp-1a2b3c4d`, so two repos with the same folder name never collide.
- tmux sessions are named `townhall-<repo-key>-<agent>`.
- Live agent state is never stored: agents are rediscovered from `git worktree list` and tmux.
- The daemon serves every registered repo (`GET /api/repos`); the routes, the
  websocket messages and the tables are in [`docs/daemon-api.md`](docs/daemon-api.md).

## repo map

```
crates/townhall-core     worktrees, tmux sessions, agent adapters
crates/townhall-daemon   http and websocket api
src-tauri                desktop app: daemon in-process, tray, notifications, repo settings
web                      react ui (vite)
docs/style-guide         design: tokens, components, preview.html
```

## design

The interface follows [`docs/style-guide`](docs/style-guide/README.md): monospace
everywhere, monochrome chrome, one amber accent, square corners, and the pixel
town hall as logo. Open `docs/style-guide/preview.html` in a browser to see every
screen in dark and light.
