# composer

The message box of home and of a conversation: `.th-composer` from `styles/conversation.css`, markup as in `preview.html` `#home` (hero) and `#conversation` (thread).

```tsx
import { Composer, type ComposerHandle } from "../composer";

<Composer
  variant="hero"
  tools={<a className="th-repos-known" href="/repos">4 repos known</a>}
  onSubmit={(text) => startConversation(text)}
  ref={composer}
  autoFocus
/>

<Composer key={id} conversationId={id} busy={thinking} onStop={stop} onSubmit={send} ref={composer} />
```

| prop | default | what it does |
|------|---------|--------------|
| `onSubmit(text, attachmentIds)` | required | Returns a promise. `text` is trimmed. Text and attachments clear when it resolves; on reject they stay and the tools row shows `error: <message>` in red |
| `variant` | `thread` | `hero`: 3 rows, `th-composer--hero`, primary `start`, help `⏎ start · ⇧⏎ new line`. `thread`: 2 rows, default `send` |
| `conversationId` | none | Turns attachments on: `+ attach`, paste, chips above the input, `addFiles`. Uploads need a conversation, so home has none |
| `tools` | none | Nodes before the help text, such as the `N repos known` link (`th-repos-known`) |
| `busy` | `false` | Townhall is thinking: input disabled, placeholder `townhall is thinking…`, the button becomes `stop` when `onStop` is given (else a disabled send) |
| `onStop` | none | Called by `stop` while `busy` |
| `disabled` | `false` | Input, `+ attach` and the button disabled |
| `placeholder`, `ariaLabel`, `submitLabel` | per variant | Hero: `what should we work on? name a repo, or describe the code`, `Describe the work`, `start`. Thread: `reply, ask for more work, or drop files here` (`reply, or ask for more work` without attachments), `Message`, `send` |
| `autoFocus` | `false` | Focus on mount |
| `focusKey` | none | Focus whenever this value changes (bump it for the `r` key) |
| `ref` | none | `ComposerHandle`: `focus()`, `fill(text)` for the home starters, `addFiles(files)` for the page's drop zone |

- `⏎` sends, `⇧⏎` adds a line, `esc` blurs. Enter during an input method composition does nothing.
- Empty or blank text does not send unless finished attachments are there. Send is disabled while any upload runs; failed uploads do not block it and are dropped on send. Only finished uploads' ids go to `onSubmit`.
- The button shows `starting…` / `sending…` with a spinner while `onSubmit` runs; the input is read-only meanwhile.
- The box grows to 8 lines (`field-sizing: content`, or `useAutoGrow` in WebKit).
- Give it `key={conversationId}` so each conversation starts with an empty draft.
- Drops: see `../attachments/README.md`.
