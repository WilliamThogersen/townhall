# Activity page (planned, route `/`)

A live feed of events across all agents: what happened, newest first. The current grid of agent cards moves to `/agents`; the header nav gains Activity (index) and Agents. Reference rendering: [preview.html](preview.html) section "Activity page".

Purpose: answer "does anything need me?" in two seconds, then "what has been going on?". It is a log you can act on, not a dashboard of charts.

## Event types

| Event | Status | Feed verb | Detail text |
|-------|--------|-----------|-------------|
| Agent started | working | Started | adapter and prompt or command, one line |
| Needs input | waiting | Needs input | the question or last prompt line |
| Output resumed after wait | working | Resumed | none |
| Finished (clean idle after work) | alive | Finished | "Task complete in 6 min" |
| Died (session ended) | dead | Died | exit reason or code |
| Error (start, stream, command) | error | Error | the message |
| Stopped by you | dead | Stopped | "Worktree removed" |

Needs daemon support: an event stream (`/events` or WebSocket) with `{ts, agent, type, text}`. Until then the page can synthesize started, died and appeared events from polling `useAgents`.

## Page layout

```
Activity                                            (page title, display)
[(!) 3 NEED INPUT  fix-login | api-tests | migrate-db     [Review first]]   <- attention strip
[All 48] [Needs input 3] [Errors 2] [Died 1] [Finished 9]   [Filter by agent]  [Density]
+-- NOW ------------------------------------------------ 3 events --+
| bar  14:02  fix-login   (!) NEEDS INPUT  Apply the migration...  Open |
| ...                                                                    |
+-- LAST HOUR ---------------------------------------- 4 events --+
```

## Attention strip

Shown only when at least one agent is waiting or in error. `--th-waiting-bg` surface, 1px `--th-waiting`, 4px left border, radius `--th-radius-md`. Contents: waiting glyph with "N need input", the first three agent names in mono (then "+N"), and a small secondary button "Review first" that opens the oldest waiting agent. When errors exist a second strip in danger colors follows. Hidden when empty, with no placeholder. `role="status"`.

## Row anatomy

Grid `4px | 56px | 120px | 1fr | auto`, gap `--th-space-4`, min height `--th-row-h` (44px comfortable, 28px compact), bottom border `--th-border`.

| Cell | Spec |
|------|------|
| Bar | state bar (pattern per status), inset `--th-space-2` top and bottom |
| Time | `--th-font-mono` `--th-text-xs` `--th-text-muted`, tabular, `HH:mm`; full timestamp in `title` and in a `<time datetime>` |
| Agent | mono `--th-text-sm` semibold, ellipsis, link to the agent (opens its card) |
| What | status indicator with a verb label (min width ~84px so detail text aligns), then detail in `--th-text-muted`, one line, ellipsis (full text in `title`) |
| Action | ghost small "Open"; for waiting rows "Reply" |

States: hover `--th-surface-raised`. Focus: each row's action and agent link are focusable; rows themselves are not. Waiting rows are tinted `--th-waiting-bg`, error rows `--th-danger-bg`; all other rows are plain so the tinted ones pop at any scale. New rows animate in (280ms fade and 4px drop, none under reduced motion) and do not move the scroll position if you scrolled down; show a "N new" pill at the top instead.

Phone (under 40rem): two lines. Line 1: bar, agent, time right-aligned. Line 2: status verb and detail. Action hidden; the row is tappable.

## Grouping by time

Sticky group headings (`--th-surface-sunken`, `--th-text-xs` caps `--th-text-muted`, z `--th-z-sticky`, with an event count on the right): **Now** (last 5 minutes), **Last hour**, **Earlier today**, **Yesterday**, then by date (`Mon 3 Nov`). Empty groups are omitted. Within a group, newest first. Lists use `<ol>`; headings are `<h3>`.

## Filters

- Chips (`aria-pressed`, single select): All, Needs input, Errors, Died, Finished. Each shows its count in the text color; counts are never color-coded.
- Text filter by agent name (mono input, 200ms debounce, case-insensitive substring).
- Optional second row (collapsed by default): kind (claude, shell), time range.
- Filter state lives in the URL query (`?type=waiting&agent=fix`) so it survives reload and can be shared.
- No match: filtered empty state with "Clear filters".

## Density options

Same control as the global density toggle, also reachable here. Comfortable: 44px rows, detail text visible. Compact: 28px rows, `--th-text-xs` times, the "Open" action appears only on hover or focus-within, group headings 22px. Compact is recommended automatically (suggest, do not force) above 12 agents.

## How it reads at 50 agents

- Routine events (started, finished, resumed) are plain and quiet; only waiting and error are tinted, so a screen of 25 compact rows (700px tall) shows the exceptions as a few colored bands in a gray list.
- The attention strip and filter counts keep the exceptions visible without scrolling, regardless of feed length.
- Burst control: events from one agent within 10 seconds of the same type collapse into one row with a count ("Output resumed x4"). Started events for many agents created together group under one summary row ("Started 12 agents") that expands inline.
- Virtualize past 200 rows; keep group headings. Cap retained history at 1000 events (older ones load on "Show earlier").
- Hovering or focusing a row highlights the matching card on the Agents page via the agent link only; no cross-page state.
- Sorting is time only. No sort controls.

## Loading, empty, error

- Loading: 8 skeleton rows at the current density.
- Empty (no events ever): "Nothing has happened yet." plus "Create an agent and its activity shows up here." and a primary "Create agent".
- Feed connection lost: info banner "Reconnecting." above the feed; rows stay.
- Feed failure: error banner "Cannot load activity: REASON" with "Retry now".
