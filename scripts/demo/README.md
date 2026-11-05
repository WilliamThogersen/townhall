# townhall demo

A throwaway townhall with realistic data, for screenshots next to `docs/style-guide/preview.html`. One command starts a daemon and Vite on your own ports, with its own home, tmux server and Claude transcripts. Several demos can run at once on different ports. Nothing touches `~/.townhall`, the default ports 7777 and 5173, or the default tmux server, and the real `claude` never runs.

## Usage

```sh
scripts/demo.sh --daemon-port 7782 --web-port 5182 --dir /tmp/th-demo-3x   # seeded, runs until ctrl-c
scripts/demo.sh --daemon-port 7782 --web-port 5182 --dir /tmp/th-demo-3x --empty   # no repos: first run
scripts/demo.sh --stop --dir /tmp/th-demo-3x                               # stop it from another shell
```

- Use the ports from your brief. 7777 and 5173 are refused.
- `--dir` is optional (default: a new `mktemp -d` folder, printed at start). It must be empty, new, or an earlier demo folder (that one is wiped first). Pass it when you want to `--stop` from another shell.
- It builds the daemon (`cargo build -p townhall-daemon`; set `CARGO_TARGET_DIR` to share one build between worktrees), seeds in about 20 s, prints the web url and one `/c/<id>` link per conversation, then runs until ctrl-c or `--stop`, which stop Vite, the daemon and the private tmux server.
- From an agent: start it in the background, wait for `<dir>/run/ready`, read the ids from `<dir>/demo.json`, and always end with `--stop --dir <dir>`.
- Logs: `<dir>/logs/daemon.log`, `vite.log`, `seed.log`.

Open `http://localhost:<web port>/` for home, `/townhall` for the control center, `/c/<id>` for a conversation (ids in the printed list and in `demo.json`).

## What is seeded

Repos (made under `<dir>/repos`, registered through `POST /api/repos`): `townhall` (main), `shop` (develop), `wexo-access` (staging), `data-hub` (main). Each has about 70 to 220 `townhall/*` branches merged into its base branch with `fix:` and `feat:` commits over the last two years, most in the last 12 months, so `GET /api/stats` fills the heatmap and the year comparison.

Conversations, newest first (times are moved back so the list reads like the preview):

| title | repo | state | what its thread holds |
|-------|------|-------|-----------------------|
| `login redirect loop` | townhall | your turn | the preview's conversation: `you` with 2 images and `localhost.har`, `repo` (named), `townhall`, `tasksCreated` (fix-login, auth-tests, logout-cookie), a second `you`, `tasksCreated` (redirect-guard, session-ttl), `needsYou` from fix-login, agent `ended`, `failed`, `passed`, `sent`. A loose `fix-login-after.png` is in its files (4 files) |
| `upgrade to vite 8` | townhall | thinking (the turn never ends; `stop` interrupts it) | `you`, `repo` |
| `flaky ci on main` | shop | your turn | `you`, `repo`, `townhall`, `tasksCreated`, agent `failed` |
| `search api is slow under load` | shop | your turn | `repo` found by code (`it has src/search/query.ts`), a `you` with `slow-queries.log`, agent `ended` and `sent` |
| `i18n keys cleanup` | wexo-access | your turn | one working task |
| `nightly export drops rows` | none yet | your turn | `repoAsk`: shop or data-hub, unanswered |
| `checkout coupon rounding` | shop | ended, 2 days ago | agent `passed` (merged into develop) |
| `dark mode contrast` | townhall | ended, 5 days ago | `townhall` reply, then an `error` entry (`claude failed: API Error: 529 ...`) |

Tasks, one per state:

| task | state | notes |
|------|-------|-------|
| fix-login | needsYou | asks `commit the fix to townhall/fix-login? (y/n)`; `send y` commits, the agent ends and logout-cookie starts |
| auth-tests, search-index, i18n-keys | working | a running step at the end of their steps |
| logout-cookie | queued | `after: [fix-login]` |
| ci-cache | inCi | pr #52, checks pending |
| load-test | inReview | pr #57, checks passed |
| session-ttl, coupon-rounding | merged | pr #38 and #33, merged for real into the base branch |
| query-plan | ended | |
| redirect-guard | error | exited 1, pr #41 with failed checks |
| e2e-smoke | error | exited 1 |

Agents: every started task has a live tmux pane (failed ones are dead panes) and a Claude transcript, so `GET /api/agents/<name>/steps` returns steps. Every agent comes from a task: there is no other way to start one.

## How it works

- `fake-claude.mjs` is `TOWNHALL_CLAUDE_BIN` (and `claude` on the demo's `PATH`). With `-p` it answers townhall's stream-json turn: the canned reply in `scenes/turns.mjs` for the seeded messages (it calls the conversation's MCP endpoint for `set_title` and `create_task`), else a short demo reply plus one task. Without `-p` it is an agent in tmux: it prints a Claude-like screen, writes a transcript under `TOWNHALL_CLAUDE_PROJECTS` and waits for input (`scenes/agents.mjs`).
- `seed.mjs` uses the API wherever a route exists. It writes SQLite only for what has none: attachments on the first message, task states `inCi` and `inReview` and the pr fields, and the timestamps.
- `web/vite.config.ts` proxies `/api` to `TOWNHALL_DAEMON_URL` (default `http://127.0.0.1:7777`). The daemon only accepts the 5173 origin for websockets and uploads, so the proxy sends that origin for pages served by this Vite server itself, on a loopback host.
- Every process gets `TOWNHALL_HOME`, `TOWNHALL_TMUX_SOCKET`, `TOWNHALL_CLAUDE_PROJECTS` and `GIT_CONFIG_GLOBAL` inside `<dir>`; git commits are made as William Thøgersen, so `GET /api/info` greets William.
