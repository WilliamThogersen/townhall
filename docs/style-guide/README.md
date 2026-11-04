# townhall style guide, v2: terminal-native

Monospace everywhere, monochrome chrome, one amber accent. Conversations are the product: every agent starts from one. `web/src/styles/` holds copies of these CSS files: change a file here and its copy there together.

Start with [preview.html](preview.html) (open it in a browser, flip dark and light at the top right).

| File | What it covers |
|------|----------------|
| [preview.html](preview.html) + [preview.js](preview.js) + [img/](img) | Home, conversation, conversation with a terminal, files and images (working lightbox), control center and repos, then every token and part |
| [tokens.css](tokens.css) | Every `--th-*` token. Dark is the default, light is `data-theme="light"` |
| [components.css](components.css) | Shell, sidebar, key bar, status glyph, buttons, fields, feedback |
| [conversation.css](conversation.css) | Conversation layout, header, centered thread, session panel, tasks, composer, context meter, menu |
| [terminal.css](terminal.css) | Agent terminal pane, stream status, prompt |
| [attachments.css](attachments.css) | Image thumbnails, file chips, upload states, drop zone, lightbox |
| [views.css](views.css) | Control center (conveyor belt), home with shipped stats, repos page |
| [foundations.md](foundations.md) | Color, contrast, type, spacing, focus, motion, glyphs, terminal theme |
| [components.md](components.md) | Each part: anatomy, states, rules, which app file it replaces |
| [layout.md](layout.md) | Routes, shell, sidebar, data model borrowed from Hive, keyboard map, migration |
| [build-brief.md](build-brief.md) | The brief for an orchestrating agent that builds this design with worker agents, in phases |
| [check-contrast.mjs](check-contrast.mjs) | `node docs/style-guide/check-contrast.mjs` checks WCAG ratios for both themes |

## Principles

1. **Conversations first.** You talk, townhall finds the repo and turns the work into tasks that agents pick up. There is no other way to start an agent.
2. **Color means state.** The chrome is black, white and grey. Green, amber, red and blue only say something about an agent, a step or a message.
3. **Amber means you.** Your messages, your focus, your cursor, the pane you are in, "now" on the timeline, and anything waiting on you.
4. **One font, lines not boxes.** Monospace everywhere, 1px lines, square corners, no shadows except on things that float.
5. **Quiet until it matters.** Nothing blinks or pulses. The working spinner is the only animation. What needs you sorts to the top.

## Voice

Text reads like good CLI output: short, lowercase, literal.

| Rule | Do | Don't |
|------|----|-------|
| Labels and buttons are lowercase verbs or nouns | `new conversation`, `start`, `send`, `end` | `New Conversation`, `Submit` |
| Messages start with their level | `error: cannot reach the daemon` | `Oops! Something went wrong.` |
| No trailing period on one-line messages | `info: conversation continued` | `Conversation continued.` |
| Confirms name the thing and the consequence | `stop auth-tests and remove its worktree?` | `Are you sure?` |
| The agent's own words are quoted as-is | `commit the fix to townhall/fix-login? (y/n)` | `fix-login has a question` |
| Progress uses `…` and the verb | `starting…`, `thinking` | `Please wait` |
| A button that opens a picker ends in `…` | `add repository…` | `add repository` |
| Names, branches and paths are verbatim | `townhall/fix-login`, `~/Developer/…/clients/shop` | `Fix Login` |
| People's names keep their capitals; lowercase is for the interface, not for people | `good afternoon, William` | `good afternoon, william` |
| Numbers compact past 9,999 | `18.4K lines` | `18,412 lines` |
| No exclamation marks, emoji, "please" or "oops" | | |

## Decisions to veto

Picked so the guide could be concrete. Each says what it costs to change.

| # | Decision | Alternative | Cost to change |
|---|----------|-------------|----------------|
| 1 | Agents are only created from a conversation. The new-agent form is gone | Keep a manual create form somewhere | Small: one dialog |
| 2 | Home (`/`) is a greeting, one composer, starters, and shipped stats | Open on the last conversation | Small |
| 3 | No repo picker. Townhall infers the repo from what you write (named, found in the code, or it asks) and stores it on every task. Several repos; the daemon is single-repo today | A picker next to the composer | Large: daemon change |
| 4 | Conversation status and ending copied from Hive: thinking, your turn, ended; a message continues an ended one | Ended is final | Small |
| 5 | Chat centered in its column; tasks, repo, files and context in a session panel on the right; a terminal takes the panel's place. The timeline lives in the control center (`/townhall`), opened from the wordmark `/townhall` | Task list under the chat (Hive's layout) | Small |
| 6 | Shipped stats count conventional commits (`fix:`, `feat:`) and lines added on merged `townhall/*` branches | Count agents' own reports | Medium: needs a git scanner |
| 7 | Heatmap and "shipped" use green; everything else stays monochrome | Amber heatmap | One token group |
| 8 | Dark is the default, light is opt-in (stored as `th-theme`) | Follow the OS | One line |
| 9 | Accent is amber `#ffb238` (dark) and `#955700` (light) | Phosphor green, but green already means alive and shipped | Four tokens |
| 10 | JetBrains Mono, self-hosted with `@fontsource/jetbrains-mono` (the app CSP blocks remote fonts) | System monospace only | Drop the import |
| 11 | No top status bar. Global agent counts sit on the right of the key bar, like a tmux status line | A status bar on top | Small |
| 12 | Square corners everywhere (`--th-radius: 0`) | 2px or 4px | One token |
| 13 | Five agent states (alive, working, waiting, error, dead). The daemon reports only alive and dead today | Ship alive and dead only | None to the design |
| 14 | Attachments: images, text, code and pdf, up to 20 MB each; images go to townhall as images, files are stored per conversation and handed to tasks | Images only | Small |
| 15 | The pixel town hall is the logo: sidebar, favicon and the app icons in `src-tauri/icons` (near-black rounded square, pixel-exact at 32 to 512px) | Keep the old line-art icon | Small |
| 16 | Agent permissions use Hive's five modes, default `accept edits`; `skip all` only from the settings page | Always ask | One setting |
