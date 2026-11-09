<p align="center">
  <img src="docs/style-guide/img/wordmark.svg" alt="/townhall" width="264">
</p>

<p align="center">
  describe the work. townhall plans it into tasks,<br>
  and runs each one as an agent in its own git worktree.
</p>

<p align="center">
  <code>rust</code> · <code>react</code> · <code>tauri v2</code> · <code>tmux</code> · built for 10 to 50 agents at once
</p>

---

## status

Early. The core runs agents, and the daemon now runs conversations that turn into tasks
across several repos, with agent states, attachments, steps and shipped stats. The
interface for them works; polish comes next.

|   | part | what it does |
|---|------|--------------|
| ● | core | one git worktree and one tmux session per agent, adapters for `claude` and `shell` |
| ● | daemon | http and websocket api on `127.0.0.1:7777`: several repos, conversations, tasks, agent states, attachments, steps and shipped stats, live terminal snapshots |
| ● | desktop | tauri app with repo picker, tray and a notification when an agent ends |
| ◐ | interface | terminal-native redesign from [`docs/style-guide`](docs/style-guide/README.md): home with shipped stats, conversations, a sidebar with search, attachments, the control center, repos and first run. the full key map, small windows and end to end tests come next |
| ● | conversations | talk to townhall; it finds the repo and creates tasks, and each task starts its own agent. a thread with task blocks and direct answers, a session panel and an agent's terminal in a split |
| ● | control center | one timeline for every agent in every conversation: the loop strip, stats, conversations, tasks and their steps, a now line and a repo filter |

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
  folder of a git repo, or a plain folder (townhall runs `git init` and commits what is in
  it), and is added through `POST /api/repos`, so the daemon keeps running. In a browser the
  same field also takes a GitHub or GitLab link (https, `git@host:owner/repo.git` or
  `ssh://`), which townhall clones into `~/.townhall/repos/`. The `ssh | https` choice, ssh
  by default, decides how it is cloned whatever form you paste: ssh clones
  `git@host:owner/repo.git`, https clones `https://host/owner/repo.git`. A link with a port
  is cloned as written.
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

### private repos

An app started from Finder does not get your shell's environment. For every `git`, `ssh`,
`gh` and `glab` it runs (clone, fetch, push, pull request), townhall asks your login shell
(`$SHELL -l -i -c`, given up after 5 seconds) for `PATH`, `SSH_AUTH_SOCK` and
`GIT_SSH_COMMAND`, once per start. Then:

- `PATH` is your shell's, then the app's, then `/opt/homebrew/bin` and `/usr/local/bin`.
- `SSH_AUTH_SOCK` is the app's own if set, else your shell's, else (macOS) launchd's
  (`launchctl getenv SSH_AUTH_SOCK`). Load your key with `ssh-add` so the agent holds it.
- Nothing ever waits for input: `GIT_TERMINAL_PROMPT=0`, and ssh runs with
  `-o BatchMode=yes` unless you set `GIT_SSH_COMMAND` (in the app's environment or your
  shell), which is used as it is.
- https remotes need a credential helper or token that works without a prompt (for
  example `gh auth setup-git`); `gh` and `glab` must be logged in (`gh auth login`).

A refused login shows as `authentication failed: ...` with what to check: an ssh key that
is not loaded (`ssh-add -l`), a host key that is not trusted yet (run `ssh -T git@<host>`
once in a terminal), or missing https credentials.

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
- Agents only come from tasks in a conversation. There is no route or form that starts
  one directly; a running agent can still be sent to, stopped and watched.
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
