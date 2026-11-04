# Build brief: townhall v2

For an orchestrating agent (Opus) that starts worker agents (Sonnet) to build the design in this folder. Hand it this file: `/orchestrate docs/style-guide/build-brief.md`.

## Your role

You orchestrate. You do not write product code yourself.

1. Read everything listed under "read first".
2. Split each phase into units small enough for one worker (one area of files, a clear done state).
3. Start Sonnet workers, each in its own git worktree, at most 5 at once.
4. Check every finished unit with a separate Sonnet verifier that did not build it.
5. Merge verified units, then report per phase.

Track each phase as a WEXO MCP task (project `townhall`, create it if missing) with the units as its todo list.

## Read first

1. `docs/style-guide/README.md`, then open `docs/style-guide/preview.html` with agent-browser and screenshot every section in dark and light. This is the target.
2. `foundations.md`, `components.md`, `layout.md` (routes, data model, daemon work, keyboard map, migration map).
3. The reference CSS: `tokens.css`, `components.css`, `conversation.css`, `terminal.css`, `attachments.css`, `views.css`.
4. The current app: `web/src`, `crates/townhall-core`, `crates/townhall-daemon/src/routes`, `src-tauri/src`, `README.md`.
5. Hive, for the conversation, task and repo model: `~/Developer/desktop/hive`, especially `src/shared/types.ts` (`Conversation`), `src/shared/conversationTasks.ts`, `src/renderer/src/components/conversationRepo.ts`, `ConversationsView.tsx`, `settings/AutomationPage.tsx`, and how `src/main` runs a conversation's Claude session.

## Rules every worker gets

- **Use the reference CSS, do not restyle by eye.** Copy the CSS files from `docs/style-guide` into `web/src/styles/` and import them. Every value comes from a token. Classes are `.th-*`.
- **Match the preview.** Before done, screenshot the screen at 1280 by 780 in dark and light and compare it with the matching `preview.html` section. Differences need a reason.
- **No em dash or en dash** anywhere: code, copy, commits, PRs.
- **No code comments** in `.ts .tsx .js .mjs .sh .yml` (CI: `.github/ci/check-no-comments.mjs`). Names explain the code.
- Immutable updates, files under 400 lines (800 max), functions under 50 lines, no `console.log`.
- **Tests first.** Vitest and Testing Library in `web`, `cargo test` in Rust. 80% coverage or more on touched code.
- Self-host the font with `@fontsource/jetbrains-mono`; the Tauri CSP blocks remote fonts.
- The desktop window can be 640 by 480. Nothing may break there, and home never scrolls.
- Copy follows the voice table in `README.md`: lowercase interface, people's names keep capitals, messages start with `error:` / `warn:` / `info:`.
- Conventional commits as the configured git user. Work on branches, never on `main`. One PR per phase.

## Done means

- `cargo test --workspace` passes
- `npm --prefix web run typecheck`, `npm --prefix web test` and `npm --prefix web run build` pass
- `bash .github/ci/check-tree-hygiene.sh` passes, and the no-comments check passes on the diff
- screenshots match the preview section, checked by the verifier

## Phases

### Phase 1: frontend foundation, current daemon only

Unit 1.1 goes first; the rest run in parallel after it.

| Unit | Scope |
|------|-------|
| 1.1 styles | Reference CSS into `web/src/styles`, JetBrains Mono, theme (dark default, `data-theme`, stored as `th-theme`), `Logo` component, favicon from `img/logo.svg` |
| 1.2 shell | `.th-shell`, sidebar (wordmark links to `/townhall`, nav, foot), key bar with context hints and agent counts, routes `/`, `/c/:id`, `/townhall`, `/repos`, `/settings`. Pick the smallest router option and say why in the PR |
| 1.3 primitives | React components: status glyph, button, kbd, segmented, switch, stepper, banner, toast stack, menu, spinner (one shared timer, reduced motion), empty state |
| 1.4 agent pane | `TerminalPanel`, `Terminal`, `SendBox`, `StopButton` become `.th-pane` with the inline stop confirm; xterm theme read from tokens and re-applied when the theme changes |
| 1.5 control center v0 | `/townhall` lists today's agents (alive and dead) as rows that open their pane. Keeps a temporary `new agent` form here until phase 3 removes it |
| 1.6 settings and repos v0 | `/settings` with every row from the preview, saved locally until the daemon has them; `/repos` shows the one repo from `/api/info` |

### Phase 2: daemon

Unit 2.1 first: it writes the API contract (Rust types, `web/src/api/types.ts`, the route list) that everything else builds on.

| Unit | Scope |
|------|-------|
| 2.1 contract and repos | API contract. Repo registry in `~/.townhall/config.json` (`name`, `path`, `baseBranch`), `GET/POST/DELETE /api/repos`, several repos at once (the repo key already exists) |
| 2.2 conversations | Storage under `~/.townhall/conversations/<id>/`. Townhall's own Claude session per conversation (`--session-id` first, `--resume` after), learn from Hive. Status thinking, your turn, ended (a message continues it). List, create, send, events over the websocket |
| 2.3 tasks | The tool townhall's session calls to create tasks (`name`, `title`, `prompt`, `repo`, `baseBranch`, `after`). A task becomes an agent in a worktree from that repo's base branch, queued until its `after` tasks finish and a slot is free |
| 2.4 repo inference | Named in the message, else found in the repo's code, else ask with one button per candidate. Emit the `~ repo …` event with the reason |
| 2.5 agent states and settings | working, waiting (with the question text), error (with exit code); see how Hive detects them. Permission mode passed as `--permission-mode`, default accept edits, `skip all` only through the settings endpoint. Work without asking, agents at once, repo-unclear behaviour |
| 2.6 attachments | Upload endpoint, files per conversation, images to townhall's session as images, files attached to tasks, 20 MB limit |
| 2.7 steps and stats | Steps per agent from Claude transcripts (tool calls with start, end, result). Shipped stats: merges from `townhall/*` into each base branch, classified by `fix:` / `feat:`, plus lines added |

Also ran in this phase: 2.0 storage (one SQLite database for every repo, setting, conversation and task) and 2.8a agent and workflow definitions; 2.8b library and 2.8c runs moved to a later phase.

### Phase 3: frontend on the new API

Runs in parallel once phase 2's contract is merged; units that need 2.2 to 2.7 wait for those.

| Unit | Scope |
|------|-------|
| 3.1 home | Greeting with your first name (daemon reads `git config user.name`), hero composer that starts a conversation, starters, stat tiles and heatmap. No scroll: height container queries as in `views.css` |
| 3.2 conversation | Centered thread (messages, events, task blocks, needs-you blocks with direct answers), composer, session panel (loop, tasks, repo, files, context) that folds, terminal split |
| 3.3 sidebar | Active and older, by repo, the one-glyph rows, search over titles and message text |
| 3.4 attachments | Attach, paste, drop, chips with upload states, thumbnails in messages, lightbox |
| 3.5 control center | Full timeline: conversations, tasks, steps, now line, repo filter. Remove the temporary new-agent form |
| 3.6 repos and first run | Repo page with add and remove, first run on home when no repo exists |

### Phase 4: polish

| Unit | Scope |
|------|-------|
| 4.1 keys | Full keyboard map from `layout.md`, key bar per context, `?` help overlay |
| 4.2 responsive | Session panel overlay under 1200px, stacked split under 1024px, one column under 640px |
| 4.3 accessibility | Audit with the web-design-guidelines skill; focus order, labels, contrast script still passes |
| 4.4 end to end | Playwright: start a conversation, a task becomes an agent, open its terminal, answer a needs-you question, see it merge |

## Avoid conflicts

- One worker per area of files. Two workers never edit the same file in the same round.
- Contract first in phases 2 and 3; workers build against it and may not change it without the orchestrator.
- Merge order inside a phase: the unit others depend on, then the rest, rebasing as you go.

## Stop and ask the user before

- removing something users rely on today before its replacement works
- changing the `~/.townhall` layout in a way existing installs can't read
- any permission default other than accept edits, or any way to reach `skip all` outside settings
- adding a dependency heavier than a small library

## Report after each phase

What now works (in one line each), screenshots next to the preview, test and CI results, open questions. Then wait for the user's go before the next phase.
