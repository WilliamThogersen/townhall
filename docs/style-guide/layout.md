# Layout

Tokens: [tokens.css](tokens.css). Reference rendering: [preview.html](preview.html). Today the app is one centered column (`.app`, max 1200px, 16px padding) with a header, the create form, then `.grid` of `minmax(420px, 1fr)` cards.

## Page structure

```
header (sticky, 48px, 2px base rule)
main  max-width 90rem (1440px), centered, padding 24px 16px 48px
  page title (h1, display) + page tools
  page content (sections separated by 32-48px)
```

Routes (planned): `/` Activity, `/agents` Agents. Desktop shows a repo chooser instead when no repo is set.

Width: content max `--th-page-max` (90rem), wider than today's 1200px so a 1440px window shows four columns. Page padding `--th-page-pad` 16px (12px under 40rem).

## Agent grid

`ul.th-grid`: `display: grid; gap: var(--th-gap); grid-template-columns: repeat(auto-fill, minmax(min(var(--th-col-min), 100%), 1fr))`.

| Density | `--th-col-min` | Gap | Card height (closed) |
|---------|----------------|-----|----------------------|
| Comfortable | 336px | 12px | about 150px (status row, name, branch, last output, actions) |
| Compact | 280px | 8px | 28px single-row list cards, see below |

Resulting columns by content width (comfortable / compact):

| Window width | Content | Comfortable | Compact |
|--------------|---------|-------------|---------|
| 390 (phone) | 366 | 1 | 1 |
| 768 | 736 | 2 | 2 |
| 1024 | 992 | 2 | 3 |
| 1280 | 1248 | 3 | 4 |
| 1440 and wider (content capped at 1408) | 1408 | 4 | 4 |

Columns equal widths; the grid reads as a row of stone columns, each card with its slim bar. An opened card spans `grid-column: 1 / -1` (as today) and pushes following cards down. At most one or two opened at a time is the expected use; with many opened, the page scrolls.

Compact cards in a single column of rows would be faster to scan at 50 agents, so below 40rem and in compact density with more than 12 agents the grid switches to one column of single-row cards (28px each, 50 agents is 1400px).

## Responsive rules

| Width | Change |
|-------|--------|
| 90rem and up | page capped at 1440px, centered |
| under 64rem | header tools wrap if needed |
| under 40rem (640px) | one column; header not sticky, tools on their own line; `--th-page-pad` 12px; feed rows two-line; terminal height 300px; dialog full width minus 16px; toasts bottom-center |
| coarse pointer | `--th-control-h` and `--th-hit-min` become 44px even in compact |

Use rem for breakpoints in `@media` (custom properties cannot be used there). The two breakpoints are `40rem` and `64rem`. Prefer intrinsic layout (`auto-fill`, `min()`, flex wrap) over adding breakpoints.

## Density modes

Set `<html data-density="compact">`; absent means comfortable. Persist as `localStorage["th-density"]`. A toggle in the header and on the Activity page.

| Token | Comfortable | Compact |
|-------|-------------|---------|
| `--th-control-h` | 36px | 28px |
| `--th-control-px` | 12px | 8px |
| `--th-row-h` (feed row) | 44px | 28px |
| `--th-card-pad` | 12px | 8px |
| `--th-gap` | 12px | 8px |
| `--th-col-min` | 336px | 280px |
| `--th-hit-min` | 36px | 24px |

Compact card: one line, 28px: bar, status glyph and label, name, last output (muted, flex), actions. The branch moves to `title`. Branch is also visible once the card is opened.

**Target size exception:** compact controls are 28px high with a minimum 24px target, which meets WCAG 2.2 AA 2.5.8 (24px) but not the 44px touch ideal. Compact is a pointer-and-keyboard mode for desk use; `@media (pointer: coarse)` forces 44px regardless. Adjacent compact targets keep at least 4px between them. Never go below 24px.

Text size does not shrink in compact (13px body stays 13px, minimum 12px); only padding, row height and gaps do.

## Activity page layout

Single column, full width of the page column (not a grid): attention strip, filter bar, feed. Group headings stick under the header (`top: var(--th-header-h)`; `top: 0` when the header is not sticky). Details in [activity-page.md](activity-page.md).

## Desktop (Tauri) specifics

- **Drag region:** the header is the window drag region (`data-tauri-drag-region` on the header container). Every interactive child (tabs, buttons, repo label) must opt out, which is automatic for buttons and links but not for plain text; give text spans `pointer-events: none` only if they must not block drag. The header must never be the only way to reach controls on the window edge.
- **Window chrome:** keep the native title bar for now. If the title bar is hidden later (overlay on macOS), reserve 80px on the left of the header for the traffic lights and the drag region covers that gap.
- **Tray-driven window behavior:** the tray icon shows and hides the window and can open it on an agent that needs input. When shown from the tray for a waiting agent, route to Activity with that agent's row focused (`?agent=NAME`), scroll it into view, and mark it with a brief `--th-accent-subtle` background for 1.2s (static highlight under reduced motion). Closing the window hides it; the app keeps running, so state (open cards, filters) must survive hide and show without re-fetch flashes: show stale content immediately and refresh in place.
- **Notifications:** OS notifications for waiting, died and error only when the window is hidden or unfocused; clicking one does what the tray does above.
- **Window size:** minimum window 360 x 480 (matches the phone layout). Default 1200 x 800. Remember size and position.
- **Theme:** `prefers-color-scheme` works in the webview; the manual override is stored like in the browser. Window background color must equal `--th-bg` in both themes to avoid a flash on show.
- **Scrollbars:** use native overlay scrollbars; `color-scheme` is set per theme so they match.
- **Text selection:** UI chrome (header, buttons) is `user-select: none` on desktop; agent names, branches, feed text and terminal remain selectable.
- **External daemon mode:** the repo control is disabled with an explanation; no other layout change.

## Migration map from current CSS

Not part of this task, for the later redesign.

| Today | Becomes |
|-------|---------|
| `--bg --panel --border --text --muted` | `--th-bg --th-surface --th-border --th-text --th-text-muted` |
| `--accent` (blue) | `--th-accent` (brass), `--th-focus` for focus |
| `--danger`, `--ok` | `--th-danger`, `--th-ok` plus glyphs |
| `.dot.alive/.dead` | `.th-status` with glyph and label, `.th-bar` |
| `.card`, `.grid`, `.badge` | `.th-card`, `.th-grid`, `.th-badge` |
| `.banner.error`, `.error` | `.th-banner`, `.th-msg-error` |
| `.terminal` (fixed `#0b0d10`) | `.th-term` with `--th-term-*` |
