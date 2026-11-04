# Components: dialog, toast, banner, empty, skeleton

Reference rendering: [preview.html](preview.html) sections "Dialog, toast, banner" and "Empty and loading". Code today: `StopButton.tsx` (inline two-step confirm), `.banner.error` in `App.tsx`, `.muted` loading and empty text, `useDeathNotifications.ts` (OS notification).

## Confirm dialog and destructive confirm

Replaces today's inline "Stop NAME and remove its worktree? Confirm / Cancel". Inline confirm is acceptable in compact density, where a dialog per stop would be slow; the wording and buttons are identical.

```
+-- 2px rule ---------------------------------+
| Stop fix-login?                              |   display font, --th-text-lg
| The agent ends and its worktree is deleted.  |
| Uncommitted work is lost.                    |
| [ worktree: ~/.townhall/worktrees/fix-login ]|   consequence strip, mono
|                    [ Cancel ] [Stop and remove] |
+----------------------------------------------+
```

| Part | Spec |
|------|------|
| Backdrop | `--th-scrim`, z `--th-z-overlay`; click does nothing for destructive dialogs |
| Surface | `--th-surface-raised`, 1px `--th-border`, top edge `--th-rule-width` solid `--th-border-strong`, radius `--th-radius-lg`, `--th-shadow-lg`, padding `--th-space-6`, width `min(26rem, 100%)`, z `--th-z-dialog` |
| Title | `--th-font-display` `--th-text-lg` semibold, a question naming the object ("Stop fix-login?") |
| Body | `--th-text-md`, plain sentences stating the consequence |
| Consequence strip | optional, `--th-surface-sunken`, mono `--th-text-sm` `--th-text-muted`: the exact path or branch affected |
| Actions | right aligned, wrap on narrow: ghost "Cancel" first, solid danger "Stop and remove" last. The confirm label repeats the verb and object, never "OK" or "Yes" |
| Enter animation | 280ms fade plus 4px rise; none under reduced motion |

Behaviour: `role="alertdialog"`, `aria-labelledby`, `aria-describedby`, focus moves to **Cancel** (the safe action), focus is trapped, Escape cancels, focus returns to the Stop button. While running: confirm button becomes "Stopping...", `aria-busy`, both buttons disabled. On failure the dialog stays open and shows an error banner inside, above the actions.

Non-destructive confirmations (rare) use a primary button and focus the primary.

## Toast and notification

For events that happen while you look elsewhere: agent needs input, agent died, agent created.

| Part | Spec |
|------|------|
| Container | fixed bottom-right (bottom-center under 40rem), gap `--th-space-3`, max width 24rem, z `--th-z-toast`, `role="status"` (use `role="alert"` for error toasts) |
| Toast | `--th-surface-raised`, 1px `--th-border`, 4px left bar in the status color, radius `--th-radius-md`, `--th-shadow-md`, padding `--th-space-4` |
| Content | 16px status glyph, bold one-line title (`fix-login needs input`), muted one-line detail, ghost icon button to dismiss |
| Duration | success and info: 5s. Waiting, died, error: stay until dismissed or the state clears |
| Stacking | newest at the bottom, at most 3 visible; beyond that collapse to "+4 more" which links to Activity |
| Hover / focus | pauses the timer |
| Enter | 280ms slide and fade, none under reduced motion |

Desktop OS notifications (Tauri notification plugin) carry the same title and detail, only for waiting, died and error, and only when the window is not focused. In-app toasts are suppressed for events on an agent whose terminal is currently open.

## Error banner

Inline, full width of its container, inserted where the failure belongs (top of page for refresh failure, above actions in a form or dialog).

| Part | Spec |
|------|------|
| Surface | `--th-danger-bg`, 1px `--th-danger` border, 4px left border, radius `--th-radius-md`, padding `--th-space-4` |
| Content | error triangle icon in `--th-danger`, bold lead sentence in `--th-danger`, then detail in `--th-text`, optional small secondary action ("Retry now") on the right |
| Role | `role="alert"` for new errors, so it is announced |
| Dismiss | none for state-based errors (they clear when resolved); dismissible for one-off errors |
| Info variant | `--th-working-bg` / `--th-working`, info icon, `role="status"` (used for "Reconnecting.") |

Replaces `.banner.error` and the bare `.error` paragraphs; field-level errors stay in the form field (components.md).

## Empty state

Shown when a list has no content. Centered column in a dashed 1px `--th-border-strong` box, radius `--th-radius-lg`, `--th-surface`, padding `--th-space-8 --th-space-5`.

- Hall glyph, 48px, `--th-border-strong` fill (a quiet watermark; no illustration).
- Title: `--th-font-display` `--th-text-2xl`.
- Body: one or two sentences, `--th-text-muted`, max 32rem.
- One primary action when there is an obvious next step.

Variants and copy are in [voice.md](voice.md). Filtered-empty ("No events match") shows no glyph and a ghost "Clear filters" button. Not used for errors: errors use the banner.

## Loading skeleton

Used when the first load takes longer than 200ms (do not flash skeletons for fast loads).

- Same footprint as the thing loading: agent card skeletons keep the 4px bar (in `--th-border`), a name line (45% wide), a branch line (70%), and the last-output strip (100%). Feed skeletons are rows of the same height as `--th-row-h`.
- Bars use `--th-skel` style: gradient `--th-surface-sunken` to `--th-border`, radius `--th-radius-sm`, 1.4s linear shimmer. Reduced motion: static, no shimmer.
- Container has `aria-busy="true"` and an `aria-label` ("Loading agents"). Show 6 skeleton cards (or 8 rows), never a spinner over an empty page.
- Replace in place with real content; no layout shift. A refresh of already loaded data shows no skeleton, only a small "Refreshing..." text in the page header's meta area.
