# townhall

An AI agent orchestrator. Rust core, React frontend. Built to run 10-50 agents at once and keep them autonomous.

Status: early scaffold.

## Goals

- Spawn, supervise and observe many agents in parallel
- Git worktree per agent
- Adapter trait so any agent CLI can plug in
- Live terminal streaming to a React UI
- Custom behavior lives in plugins, not in core

## Running

Prerequisites: Rust, Node, git and tmux. Install web deps once with `npm --prefix web install`.

### Browser dev mode

```sh
npm run dev                  # from the repo root; agents work in the current directory
npm run dev -- /path/to/repo # or TOWNHALL_REPO=/path/to/repo npm run dev
```

Builds and runs the daemon (127.0.0.1:7777) next to the Vite dev server
(http://localhost:5173, proxies `/api`), so the UI opens in a normal browser
with hot reload and no Tauri. Ctrl-C stops both processes. Agents live in tmux
sessions named `townhall-<repo-key>-<name>` and keep running.

### Desktop app (Tauri v2)

The app wraps the same web UI and runs the daemon in-process on 127.0.0.1:7777.

```sh
npm run tauri dev            # from the root (or `npm run tauri dev` inside web/)
npm run tauri build          # packaged app; add `-- --debug` for a faster debug bundle
```

Output lands under `target/`. Behavior:

- First launch opens a native folder picker. The repo must be a git repo. The
  choice is saved in `~/.townhall/config.json` (an old `settings.json` in the app config dir is migrated on first launch) and can be changed
  with "Change repo" in the header; the in-process daemon restarts on the new path.
- If port 7777 is already taken (a daemon running separately), the app uses it
  and the repo is whatever that daemon was started with.
- Closing the window hides it to the tray; use the tray menu "Quit" to exit.
  Quitting stops only the in-process server, never the agents (tmux owns them).
- A native notification fires when an agent's session goes from alive to dead.
- The UI talks to `http://127.0.0.1:7777/api` and `ws://127.0.0.1:7777/api` inside
  the app and to same-origin `/api` in a browser. The daemon allows CORS only
  for the desktop and Vite dev origins.

## Where things live

Everything townhall creates lives in one folder, `~/.townhall` (override with
`TOWNHALL_HOME`; created with mode 0700). Nothing is ever written into your repo
(no `.townhall` folder, no `.gitignore` edits); git itself only records the
worktrees under the repo's own `.git`.

```
~/.townhall/
  config.json                      desktop app settings (chosen repo)
  worktrees/<repo-key>/<agent>/    one git worktree per agent, branch townhall/<agent>
```

- `<repo-key>` is the repo folder name plus a short hash of its canonical path,
  for example `myapp-1a2b3c4d`, so two repos with the same folder name never collide.
- tmux sessions are named `townhall-<repo-key>-<agent>`, so agent names are per repo.
- No state is stored: agents are rediscovered from `git worktree list` and tmux.
- `GET /api/info` returns `{home, repo, repoKey}` for the running daemon.
