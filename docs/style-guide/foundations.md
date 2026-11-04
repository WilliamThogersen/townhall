# Foundations

Every value lives in [tokens.css](tokens.css). Names below are the real custom properties, without the `--th-` prefix where a table says so. See them rendered in [preview.html](preview.html).

## Color

Dark is the default (`:root`). Light overrides colors only, under `:root[data-theme="light"]`.

### Neutrals

| Token | Dark | Light | Use |
|-------|------|-------|-----|
| `bg` | `#0b0b0c` | `#f4f3ef` | App background, terminal, inputs |
| `surface` | `#111113` | `#fbfbf8` | Sidebar, headers, key bar, your messages, composer area |
| `surface-2` | `#18181b` | `#ecebe6` | Hover, current row, focused prompt, toast |
| `surface-3` | `#212125` | `#e2e1db` | Pressed, disabled primary fill, finished trace step |
| `line` | `#26262a` | `#deddd6` | Dividers between regions and panes, trace ticks |
| `line-strong` | `#38383e` | `#c9c7bf` | Task blocks, tags, menus, toasts, kbd, disabled borders |
| `line-control` | `#6a6a72` | `#8c8a82` | Input, button and segmented borders (3:1, WCAG 1.4.11) |
| `fg` | `#e8e6e1` | `#191917` | Text, primary button fill |
| `fg-muted` | `#a8a6a0` | `#52514c` | Secondary text, kind, branch, stat labels |
| `fg-faint` | `#8a8882` | `#6b6a64` | Meta, hints, placeholders, timestamps, events, dead agents |
| `on-inverse` | `#0b0b0c` | `#f4f3ef` | Text on an `fg` fill (primary button, checked segment) |

Inputs use `bg`, one step darker than the `surface` they sit on, like a terminal prompt.

### Accent and state

| Token | Dark | Light | Means |
|-------|------|-------|-------|
| `accent` | `#ffb238` | `#955700` | You: your messages, focus ring, caret, cursor, selection, active pane, current row and tab, now on the timeline, waiting agents, primary hover |
| `ok` | `#5fd38d` | `#1b7543` | Alive, live stream, passed, shipped, positive deltas |
| `danger` | `#ff6b6b` | `#c02b2b` | Error, stop, invalid field |
| `info` | `#6cb6ff` | `#1c5cbb` | Working spinner, running trace step, info banner |

Each has a `*-tint` (10% alpha) for row and banner backgrounds, and `accent`, `danger` have `*-strong` for hover. `on-accent` and `on-danger` are the text colors on solid fills.

Status aliases point at these: `status-alive` = ok, `status-working` = info, `status-waiting` = accent, `status-error` = danger, `status-dead` = fg-faint. Components use the aliases, so a state can change color in one place.

Rules:

- Never use a state color for decoration. If it is green, something is alive or passed.
- Amber is never used for warnings that are not about you. A daemon warning uses `warn:` with the accent tint because it limits what you can do.
- Dead is not an error. It is faint grey with a hollow circle. Red is only for things that went wrong.

### Heat ramp

`heat-0` to `heat-4`: one green hue from empty to most, used only by the shipped heatmap. It is sequential, so each theme has its own steps: in dark the scale climbs from near-black to bright green, in light from pale to deep green.

| Step | Dark | Light |
|------|------|-------|
| `heat-0` (none) | `#18181b` | `#e5e4de` |
| `heat-1` | `#1c4a2f` | `#b5e0c2` |
| `heat-2` | `#287a4a` | `#74c290` |
| `heat-3` | `#42ad6b` | `#36975a` |
| `heat-4` | `#72e6a0` | `#1a663a` |

A day's level is `ceil(value / max * 4)` over the visible period, 0 when nothing shipped. Every cell has a hover tooltip with the exact number, and the same numbers exist as a table for screen readers, so color is never the only way in.

### Contrast

From `node docs/style-guide/check-contrast.mjs`. Text needs 4.5:1, control borders and the focus ring need 3:1.

| Pair | Dark | Light |
|------|------|-------|
| fg on bg | 15.77 | 15.86 |
| fg-muted on surface | 7.75 | 7.67 |
| fg-faint on surface-2 (worst case) | 5.00 | 4.54 |
| on-inverse on fg (primary button) | 15.77 | 15.86 |
| accent on bg | 10.94 | 5.19 |
| on-accent on accent (primary hover) | 10.94 | 5.77 |
| ok on surface | 10.05 | 5.52 |
| danger on surface | 6.80 | 5.59 |
| on-danger on danger (confirm button) | 7.09 | 5.80 |
| info on surface | 8.78 | 6.14 |
| line-control on surface (borders) | 3.52 | 3.34 |

Change a color, then rerun the script. It exits non-zero if any pair drops below its minimum.

## Type

One family: `--th-font`, JetBrains Mono with the system monospace stack as fallback. Ligatures off (`font-variant-ligatures: none`), so `->` and `!=` stay as typed.

| Token | Size / line | Weight | Use |
|-------|-------------|--------|-----|
| `text-xl` | 20 / 28 | 700 | First-run and empty-state title only |
| `text-lg` | 15 / 22 | 700 | Empty-state title in a pane |
| `text-md` | 13 / 20 | 400 | Default: body, rows, inputs, terminal |
| `text-sm` | 12 / 18 | 400 | Buttons, labels, pane header, row meta, counts |
| `text-xs` | 11 / 16 | 400 | Key hints, section rules, help and error text, kbd |

- Two weights only: 400 and 700. Bold marks the one thing to read first: an open agent's name, a pane title, a primary button.
- No uppercase, no letter-spacing, no italics.
- Numbers are already tabular in a monospace font, so counts and columns line up for free.
- The terminal is 13px at line height 1.4, a little denser than the UI.

Self-host the font in the app (the Tauri CSP has no remote `font-src`):

```ts
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
```

## Space and size

A 4px grid: `space-1` 4, `space-2` 8, `space-3` 12, `space-4` 16, `space-5` 24, `space-6` 32, `space-7` 48. Gaps between inline items inside a row are `1ch`, so text keeps its character grid.

| Token | Value | Use |
|-------|-------|-----|
| `bar` | 36px | Sidebar top, conversation header, pane header, page header, sidebar foot |
| `row` | 28px | Agent strip, agent line in a block, trace span |
| `control` | 28px | Button, nav item, picker |
| `control-sm` | 24px | Button in headers, blocks, toasts |
| `control-lg` | 32px | Input, pane prompt, banner minimum |
| `keybar` | 24px | Key hint bar |
| `sidebar` | 272px | Sidebar (232px under 1024px) |
| `measure` | 76ch | Max width of thread text and composer |
| `thread-min` | 420px | Thread width before the split stacks |
| `pane-min` | 440px | Agent pane width in the split |
| `pane-min-h` | 320px | Shortest pane before the pane column scrolls |
| `home` | 720px | Home column: greeting, composer, stats |
| `rail` | 320px | Session panel on the right of a conversation |
| `thumb-h` | 104px | Image thumbnail height in messages |
| `heat-cell` | 10px | Legend cell (grid cells are fluid and square) |
| `composer-max` | 172px | Composer grows to 8 lines, then scrolls |

Touch screens (`pointer: coarse`) get 44px for every control and row. Mouse density stays at 24 to 32px: the WCAG 2.2 AA target floor is 24px.

## Lines, corners, elevation

- Borders are 1px (`--th-border`). Regions are separated by `line`. Panes are separated by a 1px gap with a `line` outline, like tmux.
- Corners are square (`--th-radius: 0`). No exceptions.
- No shadows on anything that sits in the layout. Only menus and toasts float, and they use `--th-shadow-overlay`.

## Focus

- Every focusable element shows a 2px `accent` outline with a 2px offset (`:focus-visible`).
- Rows draw the outline inset (`outline-offset: -2px`) so it does not overlap the next row.
- Inputs turn their border amber and add a 1px inset ring, so the box reads as 2px thick.
- The pane you are in gets an amber 1px frame (`.th-pane:focus-within`). That is the "active pane", same as tmux.
- Current things get a 2px amber edge: the left edge of the current conversation row and the open task row, the bottom edge of the chosen stat tile.
- The caret is amber in every input (`caret-color: var(--th-accent)`).

## Motion

| Token | Value | Use |
|-------|-------|-----|
| `dur-fast` | 80ms | Hover background and border |
| `dur` | 140ms | Menu and toast in |
| `ease` | `cubic-bezier(0.2, 0, 0, 1)` | All transitions |
| `spin-frame` | 80ms | One frame of the working spinner |

- Nothing blinks, pulses or slides on a loop.
- The working spinner cycles `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` from one shared timer, not one timer per agent.
- `prefers-reduced-motion: reduce` sets both durations to 0 and freezes the spinner on `⠿`.

## Logo

The pixel town hall: an 18 by 14 grid (pediment, four columns, two-step base), drawn as one SVG path with `shape-rendering="crispEdges"`. The source is [img/logo.svg](img/logo.svg); in the UI it is inline SVG with `fill="currentColor"`.

| Where | Size | Color |
|-------|------|-------|
| Sidebar, next to `/townhall` | 18 × 14 (1px per cell) | `fg` |
| Home greeting | 36 × 28 (2px per cell) | `accent` |
| First run and full-window empty states | 108 × 84 (6px per cell) | `fg-faint` |
| Favicon and app icon | from `img/logo.svg`, light and dark aware | `fg` on transparent |
| Repo README | `img/wordmark.svg`: logo at 3px per cell and `/townhall`, light and dark aware | `fg`, amber `/` |

- Scale only by whole numbers (1, 2, 3, 6), so every cell stays a sharp square. Never smooth, round, outline or recolor parts of it.
- Clear space around it: at least one cell on every side.

## Glyphs

No icon library. Every symbol is a text glyph, so it inherits color, size and the character grid.

| Glyph | Meaning |
|-------|---------|
| `●` | alive (filled circle) |
| `⠋` spinner | working |
| `◆` | waiting on you |
| `×` | error |
| `○` | dead / ended (hollow circle) |
| `›` | prompt sign in the pane input |
| `/` | the amber slash of the wordmark `/townhall`, which is also the control center's route |
| `+ / ~ $` | sidebar nav: new conversation, search, repos, control center. `~` also marks a repo |
| `▾ ▸` | expanded and folded groups and spans |
| `✓ → ×` | event lines: passed, sent, failed |
| `·` | separator in a pane header |
| `⏎ ↑ ↓ esc` | keys, always inside `.th-kbd` |

Every state has its own shape, so status still reads without color. The glyph is `aria-hidden`; the state is also spoken through the row's text or an `.th-sr-only` label.

## Terminal theme

xterm.js needs literal colors, so read them from the tokens at mount and again when the theme changes:

```ts
const css = getComputedStyle(document.documentElement);
const v = (name: string) => css.getPropertyValue(`--th-${name}`).trim();

const theme = {
  background: v("term-bg"),
  foreground: v("term-fg"),
  cursor: v("term-cursor"),
  selectionBackground: v("term-selection"),
  black: v("ansi-black"), red: v("ansi-red"), green: v("ansi-green"), yellow: v("ansi-yellow"),
  blue: v("ansi-blue"), magenta: v("ansi-magenta"), cyan: v("ansi-cyan"), white: v("ansi-white"),
  brightBlack: v("ansi-bright-black"), brightRed: v("ansi-bright-red"),
  brightGreen: v("ansi-bright-green"), brightYellow: v("ansi-bright-yellow"),
  brightBlue: v("ansi-bright-blue"), brightMagenta: v("ansi-bright-magenta"),
  brightCyan: v("ansi-bright-cyan"), brightWhite: v("ansi-bright-white"),
};

const rowPx = 13 * 1.4;
const cellPx = 17;

new XTerm({ theme, fontFamily: v("font"), fontSize: 13, lineHeight: rowPx / cellPx });
```

Rows are 18.2px: 13px text at line height 1.4, the same rhythm as `.th-term`. xterm's `lineHeight` is not a CSS line height: it multiplies the cell height xterm measures from the font (17px for JetBrains Mono at 13px), not the font size. So the value is 18.2 / 17, about 1.07. Passing 1.4 would give 23px rows.

In the light theme ANSI white maps to a mid grey and bright white to near black, so programs that print "white" text stay readable on a light background. Watch `data-theme` on `<html>` with a `MutationObserver` and set `term.options.theme` again when it changes.

## Z-index

| Token | Value | Use |
|-------|-------|-----|
| `z-sticky` | 10 | Sticky control center axis |
| `z-overlay` | 100 | Conversation menu |
| `z-toast` | 200 | Toast stack |
