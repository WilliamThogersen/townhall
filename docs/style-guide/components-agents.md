# Components: status, badge, agent card, terminal

Reference rendering: [preview.html](preview.html) sections Status, Agent cards, Terminal. Code today: `AgentCard.tsx` (`.card`, `.dot`, `.badge`), `TerminalPanel.tsx`, `Terminal.tsx`, `SendBox.tsx`, `StopButton.tsx`.

## Status indicator

Five states. The API currently exposes only `alive: boolean`; `working`, `waiting` and `error` need daemon support (output-activity and prompt detection, last error). Until then the UI maps `alive` to Alive and `!alive` to Dead.

| State | Glyph (16px) | Label | Bar pattern | Color token | Meaning |
|-------|--------------|-------|-------------|-------------|---------|
| alive | filled dot | ALIVE | solid | `--th-ok` | Running, idle |
| working | open ring with an arc (rotates; static under reduced motion) | WORKING | dashed (8px on, 3px off) | `--th-working` | Output changed recently |
| waiting | diamond with "!" | WAITING | dotted (3px on, 3px off) | `--th-waiting` | Needs your input |
| dead | box with X | DEAD | outline only (1px, no fill) | `--th-dead` | Session ended |
| error | triangle with "!" | ERROR | solid | `--th-danger` | Something failed |

Rules:
- Always render glyph plus text label. The bar is a third cue, never the only one. Do not use a bare colored dot (today's `.dot`).
- Label: `--th-text-xs`, semibold, uppercase, `--th-tracking-caps`, in the state color. In dense feed rows the label may be a verb ("Needs input", "Finished", "Died") but keeps its glyph.
- Markup: `<span class="th-status" data-status="waiting"><svg aria-hidden="true">...</svg>Waiting</span>`. The text is the accessible name; no extra `aria-label`.
- Changes to waiting or error also notify (toast, and OS notification in desktop); a state change alone does not move focus.

## State bar

`.th-bar`: 4px wide (`--th-bar-width`), radius `--th-radius-xs`, runs the full height of the card or row at its left edge, patterned as in the table. It is the "column" of the icon; scanning 50 cards, the eye follows bar colors and patterns down the grid.

## Badge and tag

`.th-badge`: height 20px, padding `0 --th-space-3`, 1px `--th-border`, radius `--th-radius-sm`, `--th-text-xs` medium.

| Variant | Fill / text | Use |
|---------|-------------|-----|
| default | `--th-surface-sunken` / `--th-text-muted`, mono font | agent kind (`claude`, `shell`), branch tags |
| accent | `--th-accent-subtle` / `--th-accent-text`, UI font | "new" |
| count | `--th-accent-subtle` / `--th-accent-text`, pill, tabular numbers, min width 20px | tab counts |
| warn | `--th-waiting-bg` / `--th-waiting`, semibold, with the waiting glyph | "3 need input" |

Badges are not interactive. No hover, no focus. Never the only carrier of status (use the status indicator). Max one line, no wrapping, truncate with ellipsis at 16ch.

## Agent card

The unit of the Agents grid (`<li>` in `ul.th-grid`).

```
| bar | (!) WAITING                                   [claude] |
| 4px | fix-login                                              |
|     | agent/fix-login                                        |
|     | [ Apply the migration to the staging database? (y/n) ] |
|     |                                     [ Open ] [ Stop ]  |
```

| Part | Spec |
|------|------|
| Container | `--th-surface`, 1px `--th-border`, radius `--th-radius-md`, padding `--th-card-pad` (12 / 8px compact), left padding `--th-space-3`, gap `--th-space-4` between bar and body, `min-width: 0` |
| Bar | state bar, stretches full height |
| Top row | status indicator left; kind badge right |
| Name | `--th-font-mono` `--th-text-md` semibold, single line, ellipsis, full name in `title` |
| Branch | `--th-font-mono` `--th-text-sm` `--th-text-muted`, ellipsis |
| Last output | one line of the most recent non-empty terminal line: mono `--th-text-sm`, on `--th-surface-sunken`, radius `--th-radius-sm`, padding `--th-space-2 --th-space-3`, ellipsis. This is the main scan aid and needs a daemon field (`last_line`); omit until available |
| Actions | right-aligned, small buttons: Open / Close (secondary, `aria-expanded`), Stop (danger) |
| Gap | `--th-space-2` between body rows |

Variants and states:

| State | Treatment |
|-------|-----------|
| default | as above |
| hover | background `--th-surface-raised`, border `--th-border-strong` |
| focus-within | border `--th-border-strong`; the buttons inside show their own rings |
| waiting | border 1px `--th-waiting`, background `--th-surface-raised`. Draws the eye without a second color field |
| error | border 1px `--th-danger`; last-output strip shows the error text |
| dead | name and branch in `--th-text-muted`; bar is the outline pattern |
| opened | spans all columns (`grid-column: 1 / -1`), contents below the head are the terminal panel. Open button reads "Close" |
| loading | skeleton card ([feedback](components-feedback.md#loading-skeleton)) |
| disabled | not used; unavailable actions are disabled individually |

Compact density: single row, 28px high. Order: bar, status, name, last output (flex 1, muted), actions. Branch moves into `title`. See [layout.md](layout.md#density-modes).

Opened cards keep the whole head as one row on wide screens (status, name, kind badge, actions right); on phones the actions wrap below.

## Terminal panel

Mounted only for opened agents (as today: only they hold a WebSocket).

```
(o) LIVE                                   <- stream status line
+----------------------------------------+
| xterm, 13px mono, --th-term-* colors    |   height 380px (300px phone)
+----------------------------------------+
[ Send input to the agent            ] [Send]
```

- Panel: grid, gap `--th-space-3`, top margin `--th-space-3`.
- Stream status line: `--th-text-xs` `--th-text-muted`. "Live" uses the alive indicator; "Stream connecting..." and "Stream closed" use working and dead glyphs; "Session ended: reason" uses dead. Text always present.
- Terminal frame: `--th-term-bg`, 1px `--th-border`, radius `--th-radius-md`, padding `--th-space-4`. Font `--th-font-mono` 13px (xterm `fontSize`), line height 1.2 (xterm default).
- Loading: the lazy chunk shows "Loading terminal..." in `--th-text-muted` inside a skeleton of the same height.
- Read only: `disableStdin: true` stays; input goes through the send box.

### xterm theme mapping

Read the tokens at runtime with `getComputedStyle(document.documentElement)` and rebuild the xterm `theme` when `data-theme` or the OS scheme changes (assign `term.options.theme`).

| xterm key | Token |
|-----------|-------|
| `background` | `--th-term-bg` |
| `foreground` | `--th-term-fg` |
| `cursor`, `cursorAccent` | `--th-term-cursor`, `--th-term-bg` |
| `selectionBackground` | `--th-term-selection` |
| `black red green yellow blue magenta cyan white` | `--th-term-black ... --th-term-white` |
| `brightBlack ... brightWhite` | `--th-term-bright-black ... --th-term-bright-white` |

Palettes (values in tokens.css):

| | Light terminal | Dark terminal |
|-|----------------|---------------|
| background / foreground | `#fbfaf7` / `#25231f` | `#0c0d0e` / `#dedbd4` |
| red, green, yellow, blue | `#a52a22 #1f6b3a #7a5a14 #1f4f8a` | `#f0766c #5fc084 #dcb25a #7fb0ec` |
| magenta, cyan | `#7a3a80 #14666b` | `#c792d0 #5cc3c9` |
| white | `#57534b` (mid-dark, so "white" text stays readable on a light ground) | `#dedbd4` |

The terminal follows the page theme; it does not stay dark in light mode. Programs that hard-code 256-color or truecolor output are not remapped.
