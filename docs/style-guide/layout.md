# Layout

How the app is put together: routes, the shell, the data the screens need, keys, and how today's files map onto the new parts. Rendered in [preview.html](preview.html).

## Routes

| Route | Screen | Sidebar current |
|-------|--------|-----------------|
| `/` | Home: greeting, composer, starters, shipped stats | `+ new conversation` |
| `/c/:id` | Conversation: thread, task list, composer. A task's terminal opens as a split on the right | the conversation row |
| `/townhall` | Control center: loop strip, totals, one timeline for every conversation. `?c=:id` filters to one | `$ control center` and the wordmark |
| `/repos` | Repo list | `~ repos` |
| `/settings` | Permissions and behaviour | `settings` in the sidebar foot |
| `/search?q=` | Search results over titles and message text | `/ search` |

Starting from home creates the conversation and moves to `/c/:id`. There is no other way to create an agent. There is no repo picker anywhere: townhall infers the repo (see below).

## Shell

```
┌──────────────┬──────────────────────────────────────────────┐
│ ▟ /townhall  │ main: home, conversation, control center,    │
│ + new        │ repos                                        │
│ / search     │  conversation:                               │
│ ~ repos      │  ┌ header: title · ~ repo · end ──┬ session ──┐
│ $ control    │  │      ┌ centered thread ┐       │ loop strip│
│──────────────│  │      │ › you  [img][img]│       │ tasks 5   │
│ active 5     │  │      │   townhall …     │       │ repo      │
│  rows…       │  │      └──────────────────┘       │ files 4   │
│ older 12     │  ├ composer: [+ attach] › … [send] │ context   │
│              │  └────────────────────────────────┴───────────┘
│              │  (opening a task swaps "session" for its terminal)
│ William      │                                              │
├──────────────┴──────────────────────────────────────────────┤
│ key hints for this context              ◆ 1  × 1  ● 6       │
└─────────────────────────────────────────────────────────────┘
```

- `.th-shell` is a two-column grid: sidebar (272px) and main, with the 24px key bar across the bottom. No top bar.
- `.th-main` is a column: an optional banner, then the screen, which fills the rest.
- Home never scrolls (see components.md). Every other screen scrolls inside its own region (thread, session panel, control center body, repo list), never the window.
- Every header is 36px (`--th-bar`), so the sidebar top, conversation header, pane header and page header line up across the window.
- A conversation is `.th-convo-layout`: the chat column and the 320px session panel. Opening a task's terminal (`data-pane="true"`) makes it two equal columns: chat (min 420px) and terminal (min 440px). The thread and composer center inside the chat column at 76ch.

## Breakpoints

| Width | Change |
|-------|--------|
| 1200px and up | Everything side by side, session panel open |
| 1024 to 1199px | Session panel starts folded and opens as an overlay from the right (`session` button, `s`) |
| 640 to 1023px | Sidebar 232px. An open terminal stacks under the chat. Tiles go 2 by 2. Control center label column 220px |
| under 640px (browser only; the Tauri window is at least 640px) | One column. `data-view="main"` on the shell swaps the sidebar for the main area. Key bar hidden |

## Data model, borrowed from Hive

Hive (`~/Developer/desktop/hive`) already runs planning conversations that create tasks. Townhall takes the same shape.

**Conversation** (Hive's `Conversation` in `src/shared/types.ts`):

| Field | Meaning |
|-------|---------|
| `id`, `sessionId` | Townhall's own Claude session: `--session-id` on the first turn, `--resume` after |
| `title` | Set by townhall once the topic is clear; `new conversation` until then |
| `status` | `thinking` (a turn is running), `your turn` (idle), plus derived attention from its agents |
| `tasks` | Tasks it created, oldest first (Hive's `taskRefs`) |
| `linkedRepo` | Repo set by hand from the row menu. Unset means auto |
| `ended` | Under older. Sending a message continues it and clears this (Hive's `archived`) |
| `createdAt`, `updatedAt` | Ordering |

**Task** (what an agent picks up): `name` (also the branch, `townhall/<name>`), `title`, `prompt`, `repo`, `baseBranch`, `after` (tasks it waits for), `state` (queued, working, needs you, in ci, in review, merged, ended, error), `pr` (number, checks).

**Repo inference** (new, no picker): when you write, townhall matches against the known repos in this order: a repo named in your message; a repo whose code has what you describe (file, route, package name); otherwise it asks with one button per candidate. It says which rule it used as a `~` event. Each task stores the result, so the agent's worktree starts from that repo's base branch.

**Repo of a conversation** (Hive's `linkedRepoOf`): a repo set from the row menu wins; else the most common repo among its tasks; a tie is `multiple repos`; nothing yet is `no repo`.

**Repo** (Hive's Settings > Repositories): `name`, `path`, `baseBranch`. Shown with `shortRepoPath`: home as `~`, the first folder, `…`, the last two (`~/Developer/…/clients/shop`).

**Task order** (Hive's `conversationTaskRows`): needs you, error, working, queued, in ci, merged, ended; insertion order within a group. The summary leaves zero parts out: `5 tasks · 1 needs you · 1 working`.

**Sidebar** (Hive's `ConversationsView`): active and older sections, newest activity first, a `by repo` toggle with foldable repo groups (a folded group still shows the current row), search by title right away and by message text after 200ms.

## Daemon work this design needs

The daemon has one repo and agents with `name, kind, branch, worktree, alive`. The design needs, in order:

1. Conversations: storage, townhall's own Claude session, title, status, ended.
2. Tasks created by a conversation (the orchestrator's tool), each with repo and base branch, picked up by agents, with `after` ordering.
3. Several repos with a base branch each, and repo inference over them.
4. Pull request number and checks per task (`gh` for GitHub, `glab` for GitLab).
5. Attachments: store files per conversation, pass images to townhall's session, attach files to tasks.
6. Agent states beyond alive and dead: working, waiting (with the question text), error (with exit code).
7. Steps per agent for the control center, from Claude's transcript (tool calls with start, end, result).
8. Shipped stats: scan each repo's base branch for merges from `townhall/*` and classify commits by conventional type.

Until then the UI can ship the shell, home and conversation chrome with alive and dead only.

## Keyboard map

Single keys work only when focus is not in a text field. `esc` leaves a field.

| Key | Where | Does |
|-----|-------|------|
| `n` | anywhere | Home, composer focused |
| `/` | anywhere | Search |
| `⌥↑` `⌥↓` | anywhere | Previous or next conversation |
| `r` | conversation | Focus the composer |
| `j` `k` `⏎` | conversation | Move through the session panel's tasks, open the task's terminal |
| `s` | conversation | Fold or unfold the session panel |
| `← →` `esc` | lightbox | Step through images, close |
| `tab` | conversation | Move focus between thread, task list, composer and open panes |
| `↑` `↓` `←` `→` `⏎` | control center | Move between rows, fold and unfold, open the conversation |
| `f` | control center | Filter by repo |
| `⏎` / `⇧⏎` | composer | Send / new line |
| `esc` | field, menu, confirm | Leave the field, close the menu, cancel the confirm |

The key bar always shows the keys that work right now.

## Desktop (Tauri)

- Default macOS title bar for now. If it moves to `titleBarStyle: "Overlay"`, the sidebar top gets 78px of left padding for the traffic lights and becomes the drag region.
- Closing hides to the tray, as today. Desktop notifications match the toasts: needs you, error, ended.
- First launch with no repo: home shows `add your first repository`.

## Migration map

| Today | Becomes |
|-------|---------|
| `styles.css` (`--bg --panel --border --text --muted --accent --danger --ok`) | `tokens.css` + `components.css` + `conversation.css` + `views.css` |
| `App.tsx` `.app`, `header`, `h1`, `.count` | `.th-shell`, `.th-sidebar`, key bar status |
| `CreateAgentForm.tsx` | Removed. The composer starts conversations; agents come from the conversation |
| `.grid` of `AgentCard.tsx` | Task list, task blocks in the thread, control center rows |
| `.dot`, `.badge` | `.th-status` |
| `TerminalPanel.tsx`, `Terminal.tsx`, `SendBox.tsx` | `.th-pane`, `.th-term`, `.th-prompt` |
| `StopButton.tsx` `.confirm` | Inline confirm in `.th-pane-head` |
| `.banner.error` | `.th-banner` |
| `desktop/RepoSettings.tsx` | `/repos` (repo inference replaces choosing one) |
| `desktop/DesktopApp.tsx` setup screen | First run on home |
| `useDeathNotifications.ts` | Also feeds toasts in the browser |
