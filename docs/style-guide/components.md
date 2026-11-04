# Components: shell and controls

Class names (`.th-*`) match the reference CSS in [preview.html](preview.html). Tokens are from [tokens.css](tokens.css). Existing app code today: `web/src/App.tsx` (`.app`, `header`, `h1`, `.count`), `CreateAgentForm.tsx` (`.create`), `StopButton.tsx`, `SendBox.tsx`, `desktop/RepoSettings.tsx`, `desktop/DesktopApp.tsx`. Current CSS is one dark file (`web/src/styles.css`) with `--bg --panel --border --text --muted --accent --danger --ok`; the migration map is in [layout.md](layout.md).

Common state rules for every interactive component:

| State | Rule |
|-------|------|
| default | as specified |
| hover | background step one level (see each); 90ms `--th-ease-out`; never the only cue |
| focus-visible | 2px ink ring, 2px offset ([foundations](foundations.md#focus)) |
| active | one more step than hover |
| disabled | `--th-text-subtle` text, `--th-surface-sunken` fill, `--th-border`; `cursor: not-allowed`; use `disabled`, not just a class |
| loading | label changes to a present participle ("Creating..."), `aria-busy="true"`, spinner optional, control stays disabled to prevent double submit |
| error | red outline plus a message with an icon; see form field |

## App shell and header

Anatomy: sticky header (`.th-header`) with a 2px `--th-border-strong` bottom rule (the icon's base line), then a centered page column.

```
[hall glyph] townhall   Activity [3 need input]   Agents [24]            [repo]  [theme] [density]
=== 2px rule ====================================================================================
```

- Header: `--th-surface`, min-height `--th-header-h` (48px), z `--th-z-sticky`. Contents in `max-width: --th-page-max`, side padding `--th-page-pad`, gap `--th-space-5`.
- Wordmark: `--th-font-display`, `--th-text-xl`, semibold, hall glyph 24px in `--th-text`. It is a link to `/`.
- Tabs follow the wordmark; tools (repo label, theme, density) are right-aligned with `margin-left: auto`.
- Page column: `--th-bg`, `padding: --th-space-6 --th-page-pad --th-space-8`, sections separated by `--th-space-7`.
- The `<h1>` is the page title inside the page, not the wordmark. Today's `h1 townhall` becomes the wordmark; each page gets its own `h1` in `--th-font-display` `--th-text-xl`.
- Agent count moves out of the header (today `.count`) into the Agents tab badge.
- Desktop: the repo control (`RepoSettings`) sits in the tools slot: path as `--th-font-mono` `--th-text-sm` `--th-text-muted`, truncated in the middle with `title`, followed by a secondary small button "Change repo". See [layout.md](layout.md) for the drag region.
- Narrow (under 40rem): header is not sticky, tools wrap to a second line, full width.

## Tabs and nav

`<nav aria-label="Sections">` with links (`.th-tab`), not ARIA tabs, because each is a route. Current page: `aria-current="page"`.

| Property | Value |
|----------|-------|
| Height | `--th-hit-min` minimum, stretches to header height |
| Padding | `0 --th-space-4` |
| Text | `--th-font-ui` `--th-text-md` medium; default `--th-text-muted`, hover and current `--th-text` |
| Current marker | 2px `--th-accent` underline sitting on the header rule |
| Badge | count badge or warn badge (see badge), 8px gap |
| Focus | ring inset (`outline-offset: -2px`) so it is not clipped |
| Disabled | not used; hide instead |

Keyboard: Tab through links, Enter to go. Badges must be read: include text such as "3 need input" in the accessible name, not a bare number.

## Button

One element, four variants, two sizes. Height `--th-control-h` (36 / 28px compact / 44 coarse), padding `0 --th-control-px`, radius `--th-radius-md`, 1px border, `--th-text-md` medium, gap `--th-space-3` between icon and label, `white-space: nowrap`, label sentence case.

| Variant | Default | Hover | Active | Disabled |
|---------|---------|-------|--------|----------|
| Primary | fill `--th-accent`, border same, text `--th-on-accent` | `--th-accent-hover` | `--th-accent-active` | `--th-surface-sunken`, `--th-text-subtle`, `--th-border` |
| Secondary | fill `--th-surface-raised`, border `--th-border-strong`, text `--th-text` | fill `--th-surface-sunken` | fill `--th-border` | as above |
| Ghost | transparent, text `--th-text-muted` | fill `--th-surface-sunken`, text `--th-text` | fill `--th-border` | text `--th-text-subtle`, no border |
| Danger | transparent, border and text `--th-danger` | fill `--th-danger-bg` | border `--th-danger-solid-hover` | as secondary disabled |
| Danger, confirm | fill `--th-danger-solid`, text `--th-on-danger` | `--th-danger-solid-hover` | same | as above |

- One primary per view region (header excluded). Create agent is primary; everything per-agent is secondary or danger.
- Danger (outline) starts a destructive flow; Danger confirm (solid) finishes it, only inside a confirm dialog or the confirm step.
- Small (`.th-btn-sm`): height `--th-hit-min`, `--th-text-sm`, padding `0 --th-space-3`. Use inside cards and rows.
- Loading: swap label to "Creating...", add a 14px spinner (`.th-spin`), `aria-busy="true"`, `disabled`.
- Maps to today: `button.primary` becomes Primary; `button.danger` becomes Danger; bare `button` becomes Secondary.

## Icon button

Square: width equals height (`--th-control-h`), `padding: 0`, one 16px icon, required `aria-label`, tooltip via `title`. Variants as Button; default is Ghost for close and dismiss, Secondary for a toolbar action. Never smaller than `--th-hit-min`.

## Text input and textarea

| Property | Value |
|----------|-------|
| Height | `--th-control-h`; textarea min 72px, vertical resize only |
| Fill / border | `--th-surface-raised`, 1px `--th-border-strong`, radius `--th-radius-md` |
| Text | `--th-text-md`, `--th-text`; placeholder `--th-text-subtle` |
| Font | UI by default; `--th-font-mono` `--th-text-sm` for names, branches, commands, prompts to the agent |
| Hover | border `--th-text-muted` |
| Focus-visible | standard ring (the border stays) |
| Disabled | `--th-surface-sunken`, `--th-text-subtle`, `--th-border` |
| Error | `aria-invalid="true"`: border `--th-danger` plus a 4px inset left bar in `--th-danger` (a non-color cue); message below |
| Loading | disabled, label unchanged; the submit button carries the busy state |

Placeholders are examples, not labels ("fix-login"). Every input has a visible label or an `aria-label` (the send box uses `aria-label="Input for NAME"`).

## Select

Same box as input. Native `<select>` with `appearance: none`, 32px right padding and a CSS chevron drawn in `--th-text-muted`. Options use the platform menu. States as input. Use for short fixed lists (Adapter: claude, shell).

## Form field and validation message

```
Label (optional marker)
[ input ]
hint text            or      (!) error message
```

- `.th-field`: grid, gap `--th-space-2`. Label `--th-text-sm` medium `--th-text-muted`; "(optional)" in `--th-text-subtle` regular. Never mark required with a red asterisk; mark the optional ones, as the app does.
- Message row `--th-text-sm`. Hint: `--th-text-muted`. Error: `--th-danger` with the 16px error triangle icon, so it is not color alone. Wire with `aria-describedby`; the input has `aria-invalid`.
- Validation timing (matches today's `touched` logic): show errors after the first submit attempt, then live. Do not validate on first keystroke.
- Server errors that belong to no field render as an error banner above the submit button, not under a random field.
- Form layout: single column under 40rem; otherwise `repeat(auto-fit, minmax(17rem, 1fr))`, gap `--th-gap`. Submit button aligned to the end. The create form lives in a panel (`--th-surface`, 1px `--th-border`, radius `--th-radius-lg`, padding `--th-space-5`), or in a dialog once the Activity page owns the index route.

## Send box

One row: mono text input (flex 1, min width 160px) plus secondary button "Send". Disabled while busy ("Sending..."); button also disabled while empty. Error appears below as a field error. Pressing Enter submits.

## Repo settings (desktop)

Inline in the header tools: mono path label, secondary small button. Disabled with a `title` explanation when connected to an external daemon (label "existing daemon on :7777" is `--th-text-muted`). Errors render as a compact banner below the header, not inline in it.

## First-run screen (desktop, no repo)

Centered column, max width 32rem: `h1` in `--th-font-display` `--th-text-3xl`, the sentence "Choose the git repository your agents will work in." in `--th-text-lg`, a primary button "Choose repository...", and an error banner above the button on failure. Same empty-state surface as [components-feedback.md](components-feedback.md#empty-state).
