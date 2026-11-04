# Foundations

Direction: **match the town hall icon**. The app icon is a flat black civic building: pediment, entablature, four rounded columns, a thin base rule. The interface borrows its calm, its rounded stone slabs and its thin rules, nothing more. Marble and basalt neutrals, one brass accent.

All values live in [tokens.css](tokens.css). Names below are the real custom properties. The reference rendering is [preview.html](preview.html).

## Principles

1. **Scan first.** The product watches 10-50 agents. Name, state and last line of output must be readable in one glance per card or row. Civic does not mean sparse.
2. **State is shape plus text plus color.** Never color alone (see Status).
3. **One accent.** Brass means "you can act on this". It is used for the primary button, the current tab and links. Nothing else is brass.
4. **Quiet by default, loud on need.** Only two states ask for attention: waiting (needs input) and error. Everything else is neutral or muted.
5. **Borders over shadows.** Surfaces are separated by 1px stone lines. Shadows are for things that float (dialog, toast).
6. **Columns, not cards-in-space.** The agent grid reads as a row of columns; each card carries a slim vertical state bar, echoing the icon's columns.
7. **Accessible by construction.** AA contrast, visible focus, reduced motion, status without color, minimum targets.

## Theme

- `:root` is light ("marble"). Dark ("basalt") applies under `prefers-color-scheme: dark` unless `<html data-theme="light">`, and always under `<html data-theme="dark">`.
- `data-theme` absent means follow the OS. Persist the override as `localStorage["th-theme"]` (`light` | `dark`; remove for auto), read before first paint.
- `color-scheme` is set per theme so native controls and scrollbars match.

## Color tokens

Ratios are WCAG 2.x, from `node docs/style-guide/check-contrast.mjs` (all pass). "On" means the text or glyph color drawn on that surface.

### Surfaces and borders

| Token | Light | Dark | Use |
|-------|-------|------|-----|
| `--th-bg` | `#f3f1ec` | `#111213` | Page background |
| `--th-surface` | `#faf9f6` | `#181a1b` | Cards, feed, header, panels |
| `--th-surface-raised` | `#ffffff` | `#202224` | Inputs, dialogs, toasts, hovered cards |
| `--th-surface-sunken` | `#e9e6df` | `#0c0d0e` | Last-output strip, group headings, skeleton base, hover on secondary buttons |
| `--th-border` | `#d5d0c5` | `#2e3033` | Decorative dividers, card outlines (not required for comprehension) |
| `--th-border-strong` | `#8f897c` | `#6a6d70` | Input and button outlines, header rule. 3.1:1 / 3.6:1 on bg (meets 3:1 non-text) |

### Text

| Token | Light | Dark | Use | Ratio on bg (L / D) |
|-------|-------|------|-----|------|
| `--th-text` | `#1c1b19` | `#ebe8e2` | Body, names | 15.3 / 15.3 |
| `--th-text-muted` | `#57534b` | `#a6a39b` | Secondary text, labels, branch | 6.8 / 7.4 |
| `--th-text-subtle` | `#6b665c` | `#8f8c85` | Placeholders, disabled text, hints | 5.1 / 5.6 |

All three pass 4.5:1 on bg, surface and surface-raised. On `surface-sunken`: text 13.8 / 15.9, muted 6.1 / 7.7.

### Brass accent

| Token | Light | Dark | Use |
|-------|-------|------|-----|
| `--th-accent` | `#7a5a14` | `#d2a64a` | Primary button fill, current-tab underline, selected marker |
| `--th-accent-hover` | `#6a4e10` | `#dfb55e` | Primary hover |
| `--th-accent-active` | `#5a420c` | `#c4962f` | Primary pressed |
| `--th-on-accent` | `#ffffff` | `#1a1407` | Text on the three above: 6.4 / 7.8 / 9.5 light, 8.1 / 9.5 / 6.8 dark |
| `--th-accent-text` | `#7a5a14` | `#dcb25a` | Links, accent text on bg: 5.6 / 9.4 |
| `--th-accent-subtle` | `#efe6cf` | `#2b2410` | Tint behind accent badges. Text on it: 5.1 / 7.8 |

### Focus

`--th-focus`: `#1c1b19` light, `#f1eee8` dark (15.3 / 16.2 on bg). Ink, not brass, so focus never looks like a hover or selected state.

### Status

Foreground tokens color text, glyphs and the state bar; `-bg` tokens tint rows and banners. Every foreground passes 4.5:1 on bg, surface, raised and its own `-bg`.

| Token | Light fg / bg | Dark fg / bg | Meaning | fg on surface (L / D) |
|-------|---------------|--------------|---------|------|
| `--th-ok` / `--th-ok-bg` | `#1f6b3a` / `#dfeee3` | `#5fc084` / `#12261a` | alive, finished, success | 6.2 / 7.8 |
| `--th-working` / `-bg` | `#1f4f8a` / `#dde8f5` | `#7fb0ec` / `#14233a` | working, in progress, info | 7.8 / 7.8 |
| `--th-waiting` / `-bg` | `#9a4a06` / `#f8e6d2` | `#f0a24f` / `#33220f` | needs input (the one that asks you to act) | 5.9 / 8.3 |
| `--th-danger` / `-bg` | `#a52a22` / `#f6dcd9` | `#f0766c` / `#35171a` | error, destructive outline and text | 6.8 / 6.3 |
| `--th-dead` / `-bg` | `#57534b` / `#e9e6df` | `#a6a39b` / `#202224` | session ended (neutral, not an error) | 7.3 / 6.9 |
| `--th-danger-solid` | `#a52a22` | `#c93c33` | Fill of the confirming destructive button |  |
| `--th-danger-solid-hover` | `#8f221b` | `#b3332b` | its hover |  |
| `--th-on-danger` | `#ffffff` | `#ffffff` | Text on solid danger: 7.1 / 5.0 (hover also passes) |  |

### Terminal

`--th-term-bg`, `--th-term-fg`, `--th-term-cursor`, `--th-term-selection` and 16 ANSI colors `--th-term-{black,red,green,yellow,blue,magenta,cyan,white}` plus `--th-term-bright-*`. Mapping to xterm is in [components-agents.md](components-agents.md). Every text color is at least 4.5:1 on `--th-term-bg` (light: fg 15.0, red 6.8, green 6.2, yellow 6.1, blue 7.9, magenta 7.3, cyan 6.4; dark: fg 14.1, red 7.0, green 8.7, yellow 9.8, blue 8.6, magenta 7.8, cyan 9.4). Dark `black` is deliberately dim (3.8:1) because programs use it for de-emphasis.

### Shadows and scrim

`--th-shadow-sm | md | lg` and `--th-scrim` (dialog backdrop). Warm-tinted in light, black in dark.

## Typography

| Token | Stack | Use |
|-------|-------|-----|
| `--th-font-display` | Iowan Old Style, Palatino Linotype, Palatino, Book Antiqua, Georgia, Times New Roman, serif | Wordmark, page titles, dialog titles, empty-state titles. Never for controls or data |
| `--th-font-ui` | ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, Arial, sans-serif | Everything else: labels, buttons, body, badges |
| `--th-font-mono` | ui-monospace, SF Mono, Menlo, Consolas, Liberation Mono, monospace | Agent names, branches, commands, timestamps, last-output, terminal |

All stacks resolve to installed fonts on macOS, Windows and Linux, so the app works offline and in the Tauri webview. If web fonts are adopted later they must be self-hosted woff2 files under `web/public/fonts/` loaded with `@font-face` and `font-display: swap`, never a CDN or Google Fonts link. The suggested set is Source Serif 4, Inter and JetBrains Mono; the system stack stays as fallback.

| Token | Size | Use |
|-------|------|-----|
| `--th-text-xs` | 12px | Badges, status labels (caps), timestamps in feed, captions |
| `--th-text-sm` | 13px | Dense UI, card meta, last output, messages, compact body |
| `--th-text-md` | 14px | Default body and controls |
| `--th-text-lg` | 16px | Dialog body, section titles |
| `--th-text-xl` | 20px | Wordmark, page title (display) |
| `--th-text-2xl` | 24px | Empty-state title (display) |
| `--th-text-3xl` | 32px | First-run hero (display) |

Line heights: `--th-leading-tight` 1.2 (display), `--th-leading-snug` 1.35 (dense rows), `--th-leading-normal` 1.5 (body). Weights: `--th-weight-regular` 400, `--th-weight-medium` 500 (buttons, tabs, labels), `--th-weight-semibold` 600 (names, display, status labels). `--th-tracking-caps` 0.06em on uppercase 12px labels. Use `font-variant-numeric: tabular-nums` for counts and times. Minimum text size is 12px.

## Spacing

4px base; use only these. `--th-space-1` 2px, `-2` 4px, `-3` 8px, `-4` 12px, `-5` 16px, `-6` 24px, `-7` 32px, `-8` 48px. Inside a component use 2-4 (tight) to 5; between components 4-6; between page sections 7-8. Density tokens (`--th-gap`, `--th-card-pad`, `--th-control-h`, `--th-row-h`) are listed in [layout.md](layout.md).

## Radii and borders

`--th-radius-xs` 2px (bars, skeleton ends), `-sm` 4px (badges, last-output strip), `-md` 6px (buttons, inputs, cards, feed), `-lg` 10px (dialogs, empty state, well), `-pill` (count badges, filter chips). Rounded slabs, never fully round except chips and the working glyph.

`--th-border-width` 1px for outlines and dividers. `--th-rule-width` 2px for the "base rule" motif: header underline, dialog top edge, section heading underline in the preview. `--th-bar-width` 4px for state bars (cards, feed rows, toasts, banners).

## Elevation

Flat by default: surface plus 1px border. `--th-shadow-sm` is rarely needed. `--th-shadow-md` toasts and menus. `--th-shadow-lg` plus `--th-scrim` dialogs.

## Motion

| Token | Value | Use |
|-------|-------|-----|
| `--th-dur-fast` | 90ms | hover color changes |
| `--th-dur-base` | 160ms | expand, tab change, chip toggle |
| `--th-dur-slow` | 280ms | dialog and toast enter, new feed row |
| `--th-ease-out` | `cubic-bezier(0.2, 0.7, 0.2, 1)` | entering, hover |
| `--th-ease-in-out` | `cubic-bezier(0.45, 0, 0.2, 1)` | moving, resizing |

Animate only `opacity`, `transform`, color. Exit animations are shorter than enters or skipped.

**Reduced motion rule:** under `prefers-reduced-motion: reduce` the duration tokens collapse to ~0, and every looping animation (spinner, working glyph, shimmer, cursor blink) is turned off. Information carried by motion must also be carried by text or shape (the working state keeps its dashed bar and label). Do not rely on `:hover` animation for meaning.

## Iconography

16px grid, 2px stroke, round caps and joins, `currentColor`, no fills except the "alive" dot. Inline SVG via `<symbol>`; icons inherit text color. Glyphs in use: alive (dot), working (arc), waiting (diamond with mark), dead (box with X), error (triangle with mark), check, close, plus, terminal, info. An icon-only control needs `aria-label`. The wordmark uses the town hall silhouette filled in `currentColor`.

## Focus

Every interactive element shows `:focus-visible`: `outline: var(--th-focus-width) solid var(--th-focus); outline-offset: var(--th-focus-offset)` (2px, 2px). Inside clipped containers (tabs, segmented controls) use a negative offset so the ring is not cut. Never `outline: none` without this replacement. Focus ring is never the hover style.

## Z layers

`--th-z-base` 0, `-sticky` 10 (header, feed group headings), `-dropdown` 20, `-overlay` 30 (scrim), `-dialog` 40, `-toast` 50, `-tooltip` 60. Do not use raw z-index numbers.

## Accessibility baseline

- Text 4.5:1, large text and UI boundaries 3:1; verified by `check-contrast.mjs`.
- Status never by color alone: glyph shape, text label, and bar pattern (solid, dashed, dotted, outline).
- Hit targets: 36px comfortable, 28px compact (exception, above the 24px WCAG 2.2 AA floor), 44px on coarse pointers regardless of density.
- Live updates: toasts and banners use `role="status"` or `role="alert"`; the feed is an ordered list, new rows do not steal focus.
