# Components

Every class is `.th-*`. Shared parts are in [components.css](components.css), the conversation in [conversation.css](conversation.css), agent terminals in [terminal.css](terminal.css), images and files in [attachments.css](attachments.css), control center, home and repos in [views.css](views.css). Rendered states are in [preview.html](preview.html). Each section names the app file it replaces, or says it is new.

## Sidebar

Left column, 272px, `surface`. Top to bottom:

1. `.th-sidebar-top`: the pixel logo (18 × 14) and the wordmark `/townhall` with the `/` in amber. It links to the control center at `/townhall`, so the wordmark is its own route.
2. `.th-nav`: four `.th-nav-item` rows with a glyph, a label and a hint. `+ new conversation` (`n`), `/ search` (`/`), `~ repos` (count), `$ control center` (agents by state). The current route gets `aria-current="page"`, `surface-2` and an amber glyph.
3. `.th-sidebar-scroll`: `active N` and `older N` section rules (`.th-section`). The active rule holds a `by repo` toggle (`.th-section-action`, `aria-pressed`). Grouped, each repo gets a fold header (`.th-group-head`, `aria-expanded`).
4. `.th-sidebar-foot`: the usage meter (below), your first name as written (`William`, full name in `title`), then ghost `settings`. No org, no email.

Replaces: the `App.tsx` header and `desktop/RepoSettings.tsx`.

## Usage meter

`.th-usage` in the middle column of the sidebar foot, a `1fr auto 1fr` grid: `settings` in the right column, whatever else sits there (a name in the preview, nothing in production) in the left. When both sides fit, the meter is exactly on the foot's center axis; when they don't, it shifts rather than overlapping `settings` (Hive's `SidebarUsage`). One button: an amber 8px asterisk (an inline SVG, centered on the same midline as the bars and numbers) and two 4px bars with their numbers, `30% 5h` and `54% wk` (`.th-usage-meter`, `.th-usage-track`, `.th-usage-fill` sized by `--used`). The text is `fg-muted`, `fg` on hover, focus and while the popover is open, and the pointer is a hand: it is a button. Bars are faint, amber from 60% (`data-level="warn"`), red from 80% (`hot`). A window whose reset has passed reads 0% until the next reading.

- Click opens `.th-usage-pop` above the foot, centered on the foot's axis (so on the meter when it is centered) and never wider than the foot, so it stays inside the sidebar (`role="dialog"`, `aria-expanded` on the button; Escape or a click elsewhere closes it). Per window: a header line with the name left and `30% used` right, then a full-width 6px bar (`.th-usage-bar` on the same `.th-usage-track` / `.th-usage-fill`, `role="progressbar"`, `aria-label` the window name, `aria-valuenow` the displayed percent, same faint / amber 60% / red 80% levels, a passed reset reads 0%), then one smaller faint line of details that wraps when narrow: `resets 15:10 (in 2h 30m)` (`Wed 08:00` on another day), and `lasts until reset`, `limit reached` or `runs out ~Fri 13:04 (in 1d 0h)`, amber when it runs out before the reset. Windows are a step further apart than their own lines. The foot says `Updated 12:39`.
- Stale: a failed refresh keeps the last numbers. Only the bars dim to half (`data-stale="true"`); the text and the asterisk stay as readable as ever, the popover foot reads `Stale · Updated 12:39` and why (`Offline`, `Rate limited, retrying later`, `Sign in to Claude Code to refresh`).
- Nothing to show (no token, not loaded, daemon unreachable, no windows): no meter at all, the foot is as before.
- New. Data: `GET /api/usage` through `useUsage`.

## Conversation row

```html
<button class="th-convo" data-state="error" aria-current="page">
  <span class="th-status" data-state="error"></span><span class="th-convo-title">flaky ci on main</span><span class="th-convo-time">9m</span>
  <span class="th-convo-status">e2e-smoke failed</span><span class="th-convo-repo">shop</span>
</button>
```

- Two lines on a three-column grid: one status glyph, then title and age, then under them a short status and the repo (right-aligned under the age, dropped when grouped by repo).
- One glyph for the conversation, the most urgent of its tasks: `◆` needs you, `×` failed, spinner thinking, `●` running, `○` ended. Never a strip of one glyph per agent: it can't be read.
- The status is short enough to never truncate: `fix-login needs you` (amber, row tinted), `e2e-smoke failed` (red), `thinking`, `your turn · 3 running`, `ended · 2 merged`. Details live in the conversation.
- Order in active: rows that need you first, then newest activity. Older keeps newest first. Searching filters by title at once and by message text when the daemon answers.
- States: rest, hover, focus (inset ring), current (2px amber left edge, bold title), waiting, error, ended (title in `fg-muted`).
- Right-click: `end conversation`, `repo: auto`, `repo: <name>` (Hive's menu, `.th-menu`).
- New.

## Key bar

Bottom of the window, 24px, `xs` in `fg-faint`. Left: hints for the current context only (`<kbd class="th-kbd">r</kbd>reply`). Right (`.th-keybar-status`): agents across every conversation by state, like the right side of a tmux status line. Hidden under 640px and on touch. New.

## Status glyph

`<span class="th-status" data-state="alive|working|waiting|error|dead" aria-hidden="true"></span>`

- A 2ch box: `●` alive, spinner working, `◆` waiting, `×` error, `○` dead. For `working`, put a `.th-spin` inside.
- Always `aria-hidden`; the state also exists as text nearby.
- `.th-glyphs` packs several tight for row sublines and counts.
- Replaces: `.dot.alive`, `.dot.dead` and the `alive`/`dead` badge in `AgentCard.tsx`.

## Conversation header

`.th-convo-head` (36px): bold title, the repo townhall picked in faint (`~ townhall · main`, or `multiple repos`), then on the right the `session` toggle (`.th-rail-toggle`, `aria-expanded`) and ghost `end`. The toggle carries `◆ N` while tasks need you, so a folded panel never hides that.

- `end` stops townhall's own session and moves the conversation to older. With unfinished tasks it asks first: `end login redirect loop? 2 tasks keep running`.
- No tabs. The timeline lives in the control center; the session panel has a `control center ↗` link filtered to this conversation.

## Thread

Chat. `.th-thread-inner` caps the thread at 76ch and centers in its column; inside it your messages sit on the right and townhall's replies on the left, each at most `--th-bubble` (85%) wide. Events center between them. The composer box centers on the same axis.

| Part | Class | Look |
|------|-------|------|
| Your message | `.th-msg.th-msg--you` | Right-aligned `surface-2` bubble, 2px amber right edge, no gutter sign, time on the right |
| Townhall's reply | `.th-msg.th-msg--townhall` | Left-aligned `info-tint` bubble, 2px blue left edge, empty gutter. GitHub markdown via react-markdown + remark-gfm: headings, lists, task lists, `code`, code blocks, tables (scroll sideways when wide), quotes, rules, web and mail links only, no raw HTML or images |
| Event | `.th-event` | One faint centered `sm` line with a glyph: `~` repo picked, `✓` passed, `→` sent, `×` failed, `○` ended, spinner for thinking |
| Task block | `.th-msg.th-msg--block` + `.th-block` + `.th-agent-line` | The tasks a reply created, on the replies' left edge and as wide as the widest reply (`--th-bubble`): `3 tasks for townhall · main` with the time on the right of that head, then status, bold name, kind, title, `open` |
| Needs you | `.th-msg.th-msg--ask` + `.th-ask` | Left like a reply, amber tint and edge, `◆` in the gutter, `<agent> needs you`. A permission dialog splits into the command (`.th-ask-context`, one line), the question, and one `.th-ask-option` button per numbered option (`3  Yes, and switch to auto mode`, hint faint). y/n questions keep `send y`, `send n` and a free text answer. Once answered: `◇`, no tint, `<agent> asked`, the question muted and `→ <picked option>`; the `sent` line that carried the answer is not shown again |

- Answers in a needs-you block send that text to the agent's terminal and add a `→ sent to <agent>` event.
- New; the thread replaces the create form and card grid in `App.tsx`.

## Composer

```html
<form class="th-composer">
  <div class="th-composer-box">
    <span class="th-composer-sign" aria-hidden="true">›</span>
    <textarea class="th-composer-input" placeholder="reply, or ask for more work"></textarea>
    <div class="th-composer-tools">
      <button class="th-btn th-btn--ghost th-btn--sm" aria-label="Attach files or images">+ attach</button>
      <span class="th-help">⏎ send · ⇧⏎ new line · paste or drop images</span>
      <button class="th-btn th-btn--sm" type="submit">send</button>
    </div>
  </div>
</form>
```

- Tools row: `+ attach` (conversation) or `~ 4 repos known` linking to `/repos` (home), the key hint, and the send button. No repo picker.
- `⏎` sends, `⇧⏎` adds a line. The box grows to 8 lines, then scrolls. `field-sizing: content` does this in Chromium; WebKit (Tauri on macOS) needs a small auto-grow hook.
- Focus: amber border plus inset ring, amber `›`.
- While townhall thinks: input disabled, placeholder `townhall is thinking…`, the button becomes `stop`.
- `.th-composer--hero` is the home variant: 3 lines tall, on `surface`, primary `start` button.
- Replaces: `CreateAgentForm.tsx`.

## Session panel

`.th-session`, the right column of `.th-convo-layout` (320px, `surface`). Everything the conversation produced, so the chat column stays a clean transcript.

1. Head (36px): `session`, `5 tasks`, ghost `control center ↗` (the control center filtered to this conversation).
2. Loop strip for this conversation: `plan › code › test › review › ship` with counts, `◆ needs you N` on its own line.
3. `tasks N`: one two-line row per task (`.th-task`). Line 1: state glyph, agent name, state words on the right (`needs you`, `testing`, `after fix-login`, `checks failed`, `merged`), PR link (`#41`). Line 2: the task title in faint and tags (`pr open`). Sorted by attention: needs you, error, working, queued, in ci, merged, ended.
4. `repo`: `~ townhall · main` and why (`you named it`, `it has src/search/query.ts`).
5. `files N`: every image and file shared in the conversation, by you or by agents. Images as a 3-column thumbnail grid that opens the lightbox, files as chips.
6. `context`: townhall's context meter and `56k of 200k tokens`.

- Clicking a task row swaps the panel for that agent's terminal (`.th-convo-layout[data-pane="true"]`, half the width). `close` in the pane head brings the panel back. The open row keeps an amber left edge.
- `session` in the header, or `s`, folds the panel away and the chat takes the full width (`.th-convo-layout[data-rail="closed"]`). The choice is remembered per user (`th-session-panel`).
- Under 1200px the panel starts folded and opens as an overlay from the right (`data-rail="open"`).
- New. Replaces the card grid in `App.tsx`.

## Repo inference

No repo picker. Townhall works out the repo from what you write, against the repos on `/repos`:

1. You name it (`townhall needs a settings page`): event `~ repo townhall · main, you named it`.
2. You describe the code: townhall searches the known repos and says why (`~ repo shop · develop, it has src/search/query.ts`).
3. It can't tell: a needs-you block from townhall itself, `two repos have a checkout flow. which one?`, with one button per candidate and `both`.

Every task it creates carries its repo and base branch, so the agent that picks it up starts its worktree from the right place. A wrong guess is fixed from the conversation menu (right-click a row): `repo: auto`, `repo: <name>`.

## Context meter

`.th-ctx` in the session panel: a 32px bar and a percentage of townhall's own context window (Hive shows the same as a ring). Faint until 80%, then amber, because you may want to start a fresh conversation.

## Attachments

Images and files, in both directions. Styles in [attachments.css](attachments.css).

- **Adding**: `+ attach` in the composer tools, paste (screenshots), or drop anywhere on the conversation. While dragging, `.th-drop` covers the conversation: dashed amber border, `drop to attach`, `images, text, code, pdf · up to 20 MB each`.
- **In the composer** (`.th-composer-attachments`, above the text): images as 64 by 48 thumbnails with `×` (a finished one is a `.th-thumb-open` button that opens the lightbox), files as cards (`.th-file`: page icon, name with ellipsis, a small `TYPE · size` line, and `×`). The `×` of either shows on hover or focus. Uploading shows a blue progress line at the bottom of the thumbnail; a failed one gets a red border and `×`, and the tools row says why in red (`error: screenshot-3.png is over 20 MB`). Send stays enabled for the rest.
- **In messages**: `.th-msg-images` (104px tall thumbnails, more than 3 collapse into `+N`) and `.th-msg-files` (the same cards, which download). A pasted marker such as `[Image #1]`, or a line that is just an attached file's path, is dropped from the text because the thumbnail or card replaces it. Corners stay square like everything else. Agents can post images too, for example a screenshot after a fix; those say `from <agent>` in the lightbox.
- **Where they go**: images go to townhall's session as image input. Files are stored with the conversation (`~/.townhall/conversations/<id>/files/`) and attached to the tasks that need them, so the agent gets the path.
- New.

## Lightbox

Native `<dialog class="th-lightbox">` opened with `showModal()`, filling the window over `--th-lightbox-bg`.

- Bar (36px): file name (bold), size and origin (`1440 × 900 · 96 KB · from you · 10:42`), `2 / 3`, ghost `copy`, `download`, `close esc`.
- Stage: the image at its natural size up to the window, 1px `line-strong` border, `‹ ›` buttons on the sides (disabled at the ends).
- Foot: a strip of the gallery's thumbnails, the current one with an amber border, and the key hints.
- The gallery is the images of the message (or of the session panel's files) you clicked in. `← →` step, `esc` closes, clicking outside the image closes. Focus returns to the thumbnail you opened.

## Composer

```html
<form class="th-composer">
  <div class="th-composer-box">
    <span class="th-composer-sign" aria-hidden="true">›</span>
    <textarea class="th-composer-input" placeholder="reply, or ask for more work"></textarea>
    <div class="th-composer-tools">
      <button class="th-btn th-btn--ghost th-btn--sm" aria-label="Attach files or images">+ attach</button>
      <span class="th-help">⏎ send · ⇧⏎ new line · paste or drop images</span>
      <button class="th-btn th-btn--sm" type="submit">send</button>
    </div>
  </div>
</form>
```

- Tools row: `+ attach` (conversation) or `~ 4 repos known` linking to `/repos` (home), the key hint, and the send button. No repo picker.
- `⏎` sends, `⇧⏎` adds a line. The box grows to 8 lines, then scrolls. `field-sizing: content` does this in Chromium; WebKit (Tauri on macOS) needs a small auto-grow hook.
- Focus: amber border plus inset ring, amber `›`.
- While townhall thinks: input disabled, placeholder `townhall is thinking…`, the button becomes `stop`.
- `.th-composer--hero` is the home variant: 3 lines tall, on `surface`, primary `start` button.
- Replaces: `CreateAgentForm.tsx`.

## Session panel

`.th-session`, the right column of `.th-convo-layout` (320px, `surface`). Everything the conversation produced, so the chat column stays a clean transcript.

1. Head (36px): `session`, `5 tasks`, ghost `control center ↗` (the control center filtered to this conversation).
2. Loop strip for this conversation: `plan › code › test › review › ship` with counts, `◆ needs you N` on its own line.
3. `tasks N`: one two-line row per task (`.th-task`). Line 1: state glyph, agent name, state words on the right (`needs you`, `testing`, `after fix-login`, `checks failed`, `merged`), PR link (`#41`). Line 2: the task title in faint and tags (`pr open`). Sorted by attention: needs you, error, working, queued, in ci, merged, ended.
4. `repo`: `~ townhall · main` and why (`you named it`, `it has src/search/query.ts`).
5. `files N`: every image and file shared in the conversation, by you or by agents. Images as a 3-column thumbnail grid that opens the lightbox, files as chips.
6. `context`: townhall's context meter and `56k of 200k tokens`.

- Clicking a task row swaps the panel for that agent's terminal (`.th-convo-layout[data-pane="true"]`, half the width). `close` in the pane head brings the panel back. The open row keeps an amber left edge.
- `session` in the header, or `s`, folds the panel away and the chat takes the full width (`.th-convo-layout[data-rail="closed"]`). The choice is remembered per user (`th-session-panel`).
- Under 1200px the panel starts folded and opens as an overlay from the right (`data-rail="open"`).
- New. Replaces the card grid in `App.tsx`.

## Repo inference

No repo picker. Townhall works out the repo from what you write, against the repos on `/repos`:

1. You name it (`townhall needs a settings page`): event `~ repo townhall · main, you named it`.
2. You describe the code: townhall searches the known repos and says why (`~ repo shop · develop, it has src/search/query.ts`).
3. It can't tell: a needs-you block from townhall itself, `two repos have a checkout flow. which one?`, with one button per candidate and `both`.

Every task it creates carries its repo and base branch, so the agent that picks it up starts its worktree from the right place. A wrong guess is fixed from the conversation menu (right-click a row): `repo: auto`, `repo: <name>`.

## Context meter

`.th-ctx` in the composer tools: a 32px bar and a percentage of townhall's own context window (Hive shows the same as a ring). Faint until 80%, then amber, because you may want to start a fresh conversation.

## Agent pane

An agent's terminal, opened from a task row, a task block or a control center row. It takes the session panel's place at half the width (`.th-convo-layout[data-pane="true"]`); `close` brings the panel back.

- Head (36px, lines up with the conversation header): status, bold name, kind, branch (truncates first), stream status, ghost `close`, danger `stop`.
- Stream status (`data-status`): `● live` green, spinner `connecting`, `○ ended: <reason>`, `× error: <reason>` red.
- Body: xterm on `term-bg`, themed from tokens ([foundations.md](foundations.md#terminal-theme)).
- Prompt (32px): `›` turns amber on focus; `send` is disabled while empty. The pane with focus gets the amber frame.
- Stop asks in place: red tint, `stop <name> and remove its worktree?`, ghost `cancel` (focused), `stop and remove` (danger-solid).
- Replaces: `TerminalPanel.tsx`, `Terminal.tsx`, `SendBox.tsx`, `StopButton.tsx`.

## Control center (conveyor belt)

`/townhall`, opened from the wordmark or `$ control center`. One timeline for every conversation; `control center ↗` in a conversation opens it filtered to that one.

1. **Loop strip** (`.th-loop`): `plan › code › test › review › ship` with how many tasks sit at each stage; stages with agents are bright. `◆ needs you N` sits on the right in amber.
2. **Stats** (`.th-trace-stats`): conversations, tasks, agents running, failing, merged today, tool calls. Label `xs` faint, value `lg` bold.
3. **Spans** (`.th-trace-body`): a 300px label column and a timeline. Sticky axis with ticks. Rows:
   - depth 0: a conversation, with its repo
   - depth 1: a task, with its status glyph and current stage
   - depth 2: the agent's steps (read, edit, run tests, ask, commit), folded by default with `▸`
4. **Bars** (`.th-bar`, positioned with `--start` and `--len` as percentages): done is neutral, `running` blue, `waiting` amber (dashed for steps), `error` red, `queued` a dashed stub at now. Depth 0 and 1 are solid 10px; steps are 8px outlined. A label after each bar gives duration and result (`54s · 15 passed`).
5. **Now** (`.th-now`): 2px amber line with a square head.
6. **Tags** (`.th-tag`): `slowest`, `needs you` (amber), `2 failed` (red), `15 passed` (green). At most one per row.

- Clicking a conversation row opens it; a task row opens it with that agent's terminal in the split.
- Stages come from what the agent does: editing code is `code`, a test command is `test`, waiting on you or on CI is `review`, a merged branch is `ship`.
- New. Needs the daemon to record steps (Claude's transcript has the tool calls).

## Home

`.th-home` is a 720px column centered both ways. **It never scrolls.** It is a size container, and sections drop out when its height runs out, in this order: the heatmap (under about 530px of content height), then the stat tiles (under about 360px), then the starters and the logo (under about 210px). Greeting and composer always stay.


1. `.th-greeting`: the pixel logo in amber and `good afternoon, William` (`xl` bold). The first name comes from `git config user.name` and keeps its capitals; with no name set the greeting is just `good afternoon`.
2. Hero composer, placeholder `what should we work on? name a repo, or describe the code`. `start` creates the conversation and opens it.
3. `.th-home-starters`: three ghost buttons that fill the composer.
4. `.th-stats`: shipped with townhall.

## Shipped stats

- Head: `shipped with townhall · last 12 months · all repos` and a year switch (segmented).
- `.th-tiles`: four tiles, also the metric picker (`aria-pressed`, amber bottom edge on the chosen one). Each: label, compact value, delta vs the same period last year (green when up).

| Tile | Counts |
|------|--------|
| bug fixes | `fix:` commits that reached the base branch from a `townhall/*` branch |
| features | `feat:` commits, same rule |
| lines shipped | Lines added by those merges |
| agents run | Agents started |

- `.th-heatmap`: 53 weeks by 7 days, Monday first, month labels, `mon wed fri`. Square fluid cells in the heat ramp. Hover shows `3 bug fixes · tue 14 oct`. Caption: `265 bug fixes shipped in the last 12 months`. Legend `less ▢▢▢▢▢ more`.
- The grid is `aria-hidden`; a visually hidden table lists the monthly totals.
- New.

## Repos page

`.th-page` with a header (`repos`, count, primary `add repository…`) and one `.th-repo` row per repo: bold name with the short path under it (`~/Developer/…/clients/shop`, Hive's `shortRepoPath`), base branch, conversation count, agent glyphs, ghost `new conversation`. Removing a repo asks first and says the folder and worktrees stay on disk. While `forge accounts` (settings) lists any, each row gets a second line `.th-repo-accounts` (`data-accounts="true"` on the row): per forge that has an account, a faint `github account` or `gitlab account` and a `.th-segmented` of `auto` plus one choice per registered login, saved at once; `auto` is the first account for the repo's host. A pick whose account was removed shows as `<login> (not registered)`. New.

## Settings

`/settings`, from `settings` in the sidebar foot. `.th-settings` is an 800px column of groups (`.th-setting-group`: a rule title and a bordered list of `.th-setting-row`). Each row: bold label and a faint help line on the left, the control on the right; under 680px the control drops below. Changes save at once (`saved as you change them`), no save button.

**Agents** group: one row, `agents at once`. Permissions are not settings: each agent type sets its own `permission_mode` (`default`, `acceptEdits`, `plan` or `bypassPermissions`, which is skip all; `acceptEdits` when a type sets none), shown in the agents page type detail. Saving a type that sets `bypassPermissions` always asks the user to confirm its TOML.

**Other rows**: `agents at once` (stepper, 1 to 50), `when the repo is unclear` (`ask` or `guess`), `session panel` open by default (switch), `theme` (`dark`, `light`, `system`), `notify me when` (`needs you`, `+ failed`, `everything`), `learn about me` (switch, off by default), `profile` (full-width row: textarea, `save`, `reset` with an inline confirm; `no profile yet` when empty; the help line says it is not used while the switch is off).

**Forge accounts** (group between `conversations` and `appearance and notifications`, one full-width `accounts` row): a faint `.th-account-login` line (`log in first: gh auth login or glab auth login`), a bordered `.th-accounts` list, one `.th-account` per registered login (bold label, faint `github · login@host`, ghost `remove`, no confirm: adding it again is one step), or `no accounts yet: gh and glab run as whoever is logged in`. Under it a `.th-account-add` form, every control `--th-control-lg` high on one baseline: a `forge` caption over a `github` or `gitlab` segmented, `login`, `host` (placeholder `github.com` or `gitlab.com`, any self-hosted name) and `label` fields (the same faint-caption-over-`.th-input` as the add repository form) and `add`. A refused entry (not a login, a bad host, already registered) shows as a `.th-field-error` line under the form; the rest saves at once like every row. Tokens are never entered or shown here: townhall asks `gh` or `glab` for them when it runs a command.

**Controls**:

- `.th-switch` (`role="switch"`, `aria-checked`): 36 by 20, a square knob. On is inverse video, like a checked segment.
- `.th-stepper`: `−`, the value in bold, `+`. The buttons disable at the limits.
- `.th-segmented` for two to five choices; a choice that is dangerous gets `data-tone="danger"`.

## Buttons and kbd

| Variant | Rest | Hover | Use |
|---------|------|-------|-----|
| default | 1px `line-control` border | `surface-2`, `fg-muted` border | `send`, `retry`, `cancel` |
| primary | `fg` fill, `on-inverse` text, bold | amber fill | One per view: `start`, `add repository…` |
| ghost | no border, `fg-muted` | `surface-2`, `fg` | Headers, blocks, toasts: `end`, `close`, `open` |
| danger | red text | red border, red tint | `stop` |
| danger-solid | red fill, bold | `danger-strong` | Confirm step only: `stop and remove` |

- Heights 28px, `sm` 24px. Busy: disabled with a spinner and `starting…`. Icon-only buttons need `aria-label`.
- `<kbd class="th-kbd">`: 18px, 1px `line-strong` border, `xs` text.

## Feedback

- **Banner** (`.th-banner`, `--warn`, `--info`): top of the main area, tint plus a 2px left edge, `error:` / `warn:` / `info:` first, optional action on the right. One at a time.
- **Toast** (`.th-toast`): only for other conversations. Status glyph, bold agent, what happened, `· conversation`, one ghost action. Max 3, 6 seconds, hover pauses. Container has `role="status"`. The browser twin of the desktop notification in `useDeathNotifications.ts`.
- **Empty and first run** (`.th-empty` with `.th-logo`): before any repo exists, home shows `add your first repository` and `add repository…`. Replaces the setup screen in `DesktopApp.tsx`.
- **Loading**: spinner and a verb, shown after 300ms. Replaces `Loading agents...`, `Loading terminal...`, `Starting...`.
