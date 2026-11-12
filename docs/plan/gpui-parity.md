# GPUI parity checklist

The native app (`crates/townhall-gpui`, binary `townhall-native`) must look and behave 1:1 like the Tauri + React app (`web/`). This is the list of everything `web/src` shows or does, with the native state.

Status:

- `missing`: nothing native yet.
- `built`: exists natively, not compared against the web yet.
- `confirmed (scene)`: compared with `scripts/parity.sh <scene>` and accepted as 1:1. No row is confirmed yet: the harness had not run end to end when this list was written (see "State of the first run" at the bottom).

A `built` row can still carry a known difference in its note. "seen" means seen in a native screenshot (light theme, empty daemon, 1200x800); "source" means read from `view.rs` / `theme.rs`, not yet seen on screen.

Run the comparison: `scripts/parity.sh [scene...]` writes `docs/plan/gpui-parity/<scene>.{web,native,diff}.png` and `report.md`. Scenes are in `scripts/parity/scenes.json`; add `-light` for the light theme. Native flags: `--url`, `--size WxH`, `--theme dark|light`, `--route /c/<id>`. A route that is not built exits with code 3.

## Shell (`shell/`, `ui/`, `router/`, `theme/`)

| element or interaction | status | note |
|---|---|---|
| window 1200x800, dark and light | built | native follows the OS appearance unless `--theme` is given; web default is `dark` and the choice is stored |
| theme tokens (`styles/tokens.css`) | built | `theme.rs` palette, `Palette::dark/light`; values not compared |
| theme choice dark / light / system, stored | missing | native has no setting and no storage |
| plugin theme override (`PluginTheme`) | missing | |
| sidebar column, brand (logo + `townhall` wordmark, links to control center) | built | seen: native shows a block glyph and `/townhall`; not the web logo or wordmark |
| nav item `+ new conversation` with `n` key hint | missing | |
| nav item `search` (inline query, `/` key) | missing | |
| nav item `repos` with count | missing | |
| nav item `agents` | missing | |
| nav item `plugins` | missing | |
| nav item `control center` with state glyph counts | missing | |
| nav items from plugins (`PluginNavItems`) | missing | |
| current nav item highlight (`aria-current`) | missing | |
| sidebar foot: usage meter (see below) | missing | |
| sidebar foot: plugin slot `sidebar.foot` | missing | |
| sidebar foot: `settings` button | missing | |
| top navbar at narrow width (`menu` button, wordmark) | missing | |
| menu drawer: `m` toggles, esc closes, closes on route change | missing | scene `menu-drawer` (400x800) |
| narrow layout (`max-width: 1199px`): rail and drawer rules | missing | scene `narrow` |
| key bar footer: hints of the current view | missing | seen: native footer shows the daemon note only |
| key bar: `? help` hint | missing | |
| key bar: system stats (see below) | missing | |
| key bar: state glyph counts, labelled | missing | |
| help overlay (`?`, esc closes) | missing | |
| global keys `n`, `/`, `?`, `m` ignored in text fields, terminal and modals | missing | |
| daemon-down banner `cannot reach the daemon: ...` with `retry` (busy `retrying…`) | missing | scene `daemon-down`; native only prints `error: ...` text in the thread pane |
| router: `/`, `/c/:id`, `/townhall`, `/repos`, `/agents`, `/plugins`, `/settings`, `/search`, `/p/:plugin/:page`, not found | built | native opens one view and `--route /c/<id>`; every other route exits 3 |
| not found page | missing | |
| window drag region and title bar | missing | native uses the default macOS title bar; web hides it in the desktop app |
| error boundary | missing | |
| external links open in the browser | missing | |
| Button (variants, sizes, busy state), Banner, Kbd, Loading, Spinner, Segmented, Switch, Stepper, EmptyState, StatusGlyph, Logo, Menu | missing | no shared native components yet; `gpui-component` is initialised |

## System stats (`shell/SystemStats.tsx`)

| element or interaction | status | note |
|---|---|---|
| `cpu N%` cell with meter bar | missing | polls `GET /api/system` every 5 s, only while the window is visible |
| `ram used/total GB` cell with meter bar | missing | one decimal under 100 GB |
| `disk used/total GB` cell with meter bar | missing | |
| warn level over 85%, hot level over 95% (colour) | missing | |
| hidden when the request fails | missing | |
| meter `aria-label`, values | missing | |

## Usage meter (`usage/UsageMeter.tsx`)

| element or interaction | status | note |
|---|---|---|
| hidden when no usage or no windows | missing | `GET /api/usage` |
| trigger button: icon, `N% 5h` and `N% wk` mini bars | missing | scene `usage-meter` |
| level colours by percent (`usageLevel`) | missing | |
| bar tooltip text (`meterLabel`) | missing | |
| hover state | missing | scene `usage-meter` hovers the button |
| popover on click: `5 hours` and `Week` rows with used, bar, reset time, run-out estimate | missing | scene `usage-open` |
| run-out warning tone | missing | |
| popover foot: updated label, stale reason | missing | |
| stale state (`data-stale`) | missing | |
| closes on outside mouse down and esc, focus returns to trigger | missing | |
| values re-computed as time passes (`useNow`) | missing | |

## Sidebar conversations (`sidebar/`)

| element or interaction | status | note |
|---|---|---|
| list of conversations from `GET /api/conversations`, live via events | built | scene `sidebar` |
| sections `active` and `older` with counts | built | seen: `active 0`, `older 0` labels are lower-case mono; web sections and ordering not compared |
| row: title, age, status text, repo | built | native row shows title, task count and thinking marker only; no age, status or repo |
| row selected highlight | built | |
| row `thinking` indicator | built | |
| group by repo (`RepoGroups`, preference stored) | missing | |
| search filter with message matches (`useMessageMatches`) | missing | |
| row context menu (right click): end, link repo | missing | scene `row-menu` |
| row menu error line | missing | |
| keyboard: arrows / enter through rows (`useConversationKeys`) | missing | |
| empty and loading states | built | native prints `error: ...` for a failing list; no loading or empty copy |
| age labels refresh over time (`useNow`) | missing | |

## Conversation page (`views/conversation/`)

| element or interaction | status | note |
|---|---|---|
| header: title, status (`thinking`, your turn, ended), repo, actions | missing | scene `conversation` |
| header: stop, end (inline confirm `y/n`) | missing | |
| header: session rail toggle | missing | |
| thread: you message with time and quoted reply | built | bubble with tint and rule; no time, no quote |
| thread: townhall message rendered as markdown | built | plain text today; the markdown thread is being built in the parallel task `gpui-p1-thread` |
| markdown: headings, lists, code blocks, inline code, links, tables | missing | owned by `gpui-p1-thread` |
| thread: attachments thumbnails (`Thumbs`), file cards | missing | |
| attachment lightbox, image actions, focus trap | missing | |
| thread: `repo` event line | built | event line with glyph |
| thread: `repoAsk` block with choices | missing | |
| thread: `tasksCreated` task block | built | event line only |
| thread: `needsYou` block with answer input and `y/n` | missing | |
| thread: agent event lines (`ended`, `failed`, `passed`, `sent`) | built | event lines |
| thread: handoff, plugin and error events | built | error entry as a line; handoff and plugin events not checked |
| thread: stick to bottom, scroll anchoring | missing | |
| thread: ended conversation state | missing | |
| error banner `dismiss` | missing | native shows a red line in the pane |
| composer: textarea, `›` sign, placeholder, help line | built | seen: placeholder `reply, or ask for more work`, help `⏎ send · ⇧⏎ new line`, `send` button |
| composer: Enter sends, Shift+Enter newline, grows to 8 lines | built | |
| composer: drafts per conversation, kept across restarts | built | |
| composer: IME composition, paste | built | |
| composer: thinking placeholder and `stop` button | built | sending is blocked while thinking; no stop button |
| composer: attach button, file input, upload chips, drop zone | missing | |
| composer: message queue list, up-arrow pulls last, esc restores | missing | |
| composer: reply bar (quote a message) | missing | |
| composer: plugin actions | missing | |
| composer: error line and `retry` for failed uploads | missing | |
| keys: `r` reply, `s` session panel, esc leaves input | missing | |
| session panel (rail): tasks list with state glyph, name, words, title, agent tag, pr tags, blocked | missing | scene `session-panel` |
| session panel: repo, files, context sections | missing | |
| session panel: loop strip, proof section | missing | |
| session panel: link to control center | missing | |
| session panel: keyboard (arrows, enter, close) | missing | |
| session panel: plugin slot | missing | |
| session panel open by default setting | missing | |
| task terminal pane (xterm, live pty over websocket, close) | missing | scene `terminal` |
| terminal: esc goes to the program, key hints change | missing | |
| terminal: reconnect, loading and dead states | missing | |
| terminal theme from the palette | missing | |

## Home (`views/home/`)

| element or interaction | status | note |
|---|---|---|
| greeting by time of day | missing | scene `home` |
| hero composer (`start`, autofocus, focus on any new key) | missing | |
| known repos chips in the composer tools | missing | |
| starter prompts, plugin starters, click fills the composer | missing | |
| shipped stats tiles | missing | |
| heatmap grid with month labels, hover tip | missing | scene `heatmap` |
| year comparison | missing | |
| first-run slot (no repos) | missing | |
| drop zone for files | missing | |

## Control center (`views/control/`)

| element or interaction | status | note |
|---|---|---|
| head: caption, connection state, repo filter menu (`f`) | missing | scene `control` |
| loop strip and stats | missing | |
| trace tree: rows with bars on a time axis, toggle, open | missing | |
| keyboard through rows | missing | |
| loading, empty (`nothing ran today`) and error states | missing | |
| filter by conversation, `all conversations` | missing | |
| live steps polling only while visible | missing | |

## Repos (`views/repos/`)

| element or interaction | status | note |
|---|---|---|
| list with name, path, base, merges | missing | scene `repos` |
| add repo modal and form | missing | |
| edit repo fields, accounts per repo | missing | |
| remove confirm | missing | |
| keys, focus return | missing | |

## Agents (`views/agents/`, `agents/`)

| element or interaction | status | note |
|---|---|---|
| head `agents` with count and `agent types` / `pipelines` switch | missing | scene `agents` |
| filter input, list rows with default marker and usage | missing | |
| detail: TOML view and save, avatar and avatar editor, copy button | missing | |
| usage graph | missing | |
| prompt view | missing | |
| pipelines section and editor | missing | |
| loading, empty and error states | missing | |
| arrow keys through rows | missing | |
| agent pane (`AgentPane`, `PaneHead`, `PaneStop`) | missing | |
| agent death notification | missing | `desktop/notify.ts`, system notifications |

## Settings (`views/settings/`)

| element or interaction | status | note |
|---|---|---|
| group `agents`: agents at once stepper | missing | scene `settings` |
| group `conversations`: repo unclear choice, session panel switch | missing | |
| group `repos`: clones folder | missing | |
| group `forge accounts` and detected accounts | missing | |
| group `about you`: learn about me, profile | missing | |
| group `appearance and notifications`: theme, plugin theme, notify when | missing | |
| plugin settings groups and frames | missing | |
| help text per row | missing | |

## Search (`views/search/`) and plugins (`views/plugins/`, `plugins/`)

| element or interaction | status | note |
|---|---|---|
| search page, hit list with highlighted matches | missing | |
| plugins page: installed, available, marketplaces | missing | |
| plugin install flow and permission prompt | missing | |
| plugin page frame (`/p/:plugin/:page`) | missing | |
| plugin slots (sidebar foot, composer actions, session panel, starters, settings) | missing | |
| plugin contributions: themes, nav items | missing | |

## Toasts (`ui/Toasts.tsx`)

| element or interaction | status | note |
|---|---|---|
| toast stack, at most 3, glyph, title, text, context | missing | scene `toasts`; only plugins push toasts, so the web shot needs a plugin and is not captured yet |
| auto dismiss after 6 s, paused on hover and focus | missing | |
| action button and dismiss button | missing | |

## Data and platform

| element or interaction | status | note |
|---|---|---|
| daemon client over HTTP and websocket events | built | `client.rs`, `--url` |
| embedded daemon, or adopt a running one | built | `daemon.rs` |
| reconnect and resync after a dropped socket | built | refetches the list and the open thread |
| settings store (`/api/settings`) | missing | |
| stats, steps, tasks, agents, pipelines, plugins, library clients | missing | |
| keyboard hints context (`keyHints`) | missing | |
| fonts (JetBrains Mono) | built | `fonts.rs` |

## State of the first run

The native flags, the capture helper, the Playwright scene list and `scripts/parity.sh` are in place. The agent that built them could execute `cargo`, `npm`, `npx` and `python3` but not shell scripts, so `scripts/parity.sh` and the demo it starts were never run end to end here. What was run: the native unit tests, `tsc`, and one native capture against an empty in-process daemon (`--route /c/none --theme light`, window found and captured with `screencapture -l`, cropped to 1200x800). No web screenshot was taken, so no row above is confirmed and no mismatch percentage exists yet. Run `scripts/parity.sh` once and update this file and `docs/plan/gpui-parity/report.md` from the result.
