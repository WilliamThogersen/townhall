# GPUI migration plan and spike

Status: research and a small spike. Nothing in `src-tauri/` or `web/` is touched. Written against `main` at `c5ea66f` on 2026-11-12.

Why: the desktop app (Tauri 2 + React 19) uses about 25% CPU idle and about 500 MB RAM (the user's figures, not measured here). A parallel task, `idle-cpu-profile`, measures where that goes. This file is about what replacing the UI with a native GPUI app would take.

How to read the claims: "verified" means I read the source or ran it in this session (gpui 0.2.2 from the cargo registry, this repo, the spike). "Not verified" means it comes from memory of Zed or the ecosystem and must be checked by the first task that depends on it.

## 1. Recommendation

1. Decision: go (user, 2026-11-12). Start phase 0 now. Earlier analysis: about 24 engineer-weeks (range 18 to 32) for one person who already knows Rust, and phases 0 and 1 give a usable read and reply app in about 8 weeks.
2. Plugin iframes: drop them in the native app and replace them with declarative slots, plus one plugin page that opens in its own webview window (section 4).
3. Keep the daemon and its HTTP and websocket API exactly as they are. The GPUI app hosts the daemon in-process, as `src-tauri` does today, and talks to it over loopback with `townhall-core` types (section 5).

The spike shows the idle floor of GPUI is low (about 0.7% CPU, 56 MB RSS for a sidebar and a thread), but it is a small read-only view. It says nothing yet about the cost of a terminal, markdown or a text input.

## 2. GPUI status (verified unless marked)

| Question | Answer |
|----------|--------|
| Published? | Yes: `gpui` 0.2.2 on crates.io (Apache-2.0, repo `zed-industries/zed`, no `rust-version` set). Zed itself uses a git revision of the monorepo, so crates.io is a snapshot that lags `main` (not verified how often it is cut). |
| Versioning | 0.x, no stability promise. Expect API breaks on every bump; pin an exact version and upgrade on purpose. |
| macOS | Metal. The build script compiles shaders with `xcrun metal`, which needs the Metal Toolchain (`xcodebuild -downloadComponent MetalToolchain`). It was missing here and the build failed with `cannot execute tool 'metal' due to missing Metal Toolchain`. The cargo feature `runtime_shaders` skips that step and compiles the shaders when the app starts: that is what the spike uses. A release build needs the toolchain (or the same feature, at a startup cost I did not measure). |
| Linux | Wayland and X11 are default features, rendering through `blade-graphics` (needs Vulkan). Not built or run here. |
| Windows | `src/platform/windows/` exists in 0.2.2 (verified present). Not built or run here; how complete it is, I have not checked. |
| Build cost | First release build of the spike, whole dependency graph, on a 12-core Apple Silicon Mac: about 2 minutes. Release binary 6.8 MB. |
| Component library | `gpui-component` 0.9.1 on crates.io (repo `longbridge/gpui-kit`, homepage `gpui-kit.com`; the repo was `longbridge/gpui-component` when last I knew). Its published `Cargo.toml` depends on `gpui-pre =0.3.12`, "a snapshot of zed@d2a4fb6", not on Zed's `gpui` 0.2.2. Two copies of GPUI in one binary do not compose, so you pick a base: Zed's `gpui` or the `gpui-pre` snapshot that the component library follows. Who publishes `gpui-pre` is not stated in what I read; treat it as a third-party fork. |
| Webview | `gpui-webview` 0.9.1 and `gpui-wry` 0.7.3 exist on crates.io (Wry based). I did not build or read them. `tauri-plugin-gpui` 0.1.1 ("render GPUI views inside ordinary Tauri windows") also exists: the reverse hybrid. |
| Accessibility | I found no accessibility tree API in gpui 0.2.2 (no `accesskit`; the few text hits for "accessib" are incidental, I only looked at the counts). Not verified: whether Zed has any plan. This is a real regression against the web UI, which has roles, `aria-*` and screen reader tables. |

## 3. Feature map

Sizes of the web code (non-test, rounded): about 9,300 lines of TSX, 11,600 of TS, 5,500 of CSS. Most files have a sibling vitest file. About half of the TS is `web/src/api/*` (types, parse guards, fetch functions) and `web/src/data/*`, which a Rust client replaces with the existing `townhall_core::model` types: no hand-written guards.

| Feature | Today | GPUI approach | Effort and risk |
|---------|-------|---------------|-----------------|
| Window, layout, panels | CSS grid, breakpoints | `div()` flex tree with the same token values (`--th-sidebar` 272, `--th-bar` 36, ...). Breakpoints become width checks in `render`. | Low. The spike has the shell. |
| Theme (dark, light, system) and tokens | `tokens.css` custom properties, `data-theme` | A `Theme` struct generated from `docs/style-guide/tokens.css` (a small build step or a test that diffs them, so the two cannot drift), held as a gpui `Global`. `prefers-color-scheme` equivalent: window appearance. | Low. Keep the contrast script as a test. |
| Fonts (JetBrains Mono) | `@fontsource` woff2 | Embed TTF and register with `cx.text_system().add_fonts`. woff2 is not usable: ship TTF files (not in the repo today, and none were found on this machine, so the spike uses Menlo). Ligatures off must be set per text run. | Low. Needs the TTF licence file added. |
| Glyph status, spinner | text glyphs, one shared timer | Same glyphs as text. The one spinner timer is the only animation: drive it with a single `cx.spawn` loop that calls `notify` only on the entities that show it, and stop it when none is visible. This is where GPUI's idle advantage can be lost: any always-on timer redraws. | Low, but test idle CPU in CI-less manual checks. |
| Sidebar, search, rows, context menu | React, `Menu` | Virtualized list (`uniform_list`, fixed 28px rows, or `list` for variable rows) plus a popup menu. Row menu: gpui has no native context menu component in the crate, build it as an overlay `deferred` element or use `gpui-component`'s popover (see section 2 on the base choice). | Medium. |
| Conversation thread | React, `react-markdown` | `list` (variable height, measured) over entries. Auto scroll to bottom, keep position when older content grows. | Medium. Variable-height virtualization is the usual GPUI pain. |
| Markdown (GFM: headings, lists, task lists, code, tables, quotes, links, no raw HTML, no images) | `react-markdown` + `remark-gfm` + `remark-breaks` | Parse with `pulldown-cmark` (GFM options) into an element tree. Zed has a `markdown` crate, but I believe it is not on crates.io (not verified). Tables that scroll sideways, selectable text and link clicks are the work. Text selection across blocks is the hard part: GPUI text is selectable only where you build it. | High for selection, medium for rendering. |
| Composer: multiline text input, IME, auto grow to 8 lines, paste | `<textarea>` | The crate has the IME hooks (`InputHandler`: `marked_text_range`, `replace_and_mark_text_in_range`, `bounds_for_range`, verified) and an `examples/input.rs` that is single line. Multiline editing (selection, word and line moves, undo, wrap, scroll, grow) is not in the crate. Options: build on `gpui-component`'s input (if its multiline input is good enough; not verified), or write it. | High. The biggest single risk after the terminal. Budget a test per IME case (Japanese, Chinese, dead keys) on every OS. |
| Drafts, message queue, reply bar | React state in `localStorage` | Plain Rust state, persisted to a small file under the townhall home. | Low. |
| Attachments: images, files, upload progress | XHR multipart with progress | `ureq` or `reqwest` multipart; progress by wrapping the reader. Thumbnails with gpui `img()` (decodes png, jpeg, gif, webp; verified an `img` element and an image gallery example exist). | Medium. |
| Drag and drop of files | `useFileDrop`, drop zone | `div().on_drop::<ExternalPaths>(...)` and `can_drop` (verified the `on_drop` and `can_drop` hooks; `examples/drag_drop.rs` exists). Reading dragged files from paths is easier than in a browser. | Low to medium. Check the drop highlight on Linux. |
| Clipboard paste of images | `ClipboardEvent` | `cx.read_from_clipboard()` returns a `ClipboardItem`; images are in it (verified the API, not the image path on all three OSes). Copy image in lightbox: `write_to_clipboard`. | Medium. |
| Lightbox | `<dialog>` | An overlay element with focus capture and arrow keys. | Low. |
| Agent terminal (xterm.js, tmux attach over websocket) | `@xterm/xterm`, `attachSocket.ts`: binary frames out, binary in, one JSON resize frame | `alacritty_terminal` 0.26.0 (verified on crates.io, Apache-2.0) as the emulator, fed by the websocket bytes: no PTY on the GUI side, since the daemon already owns the tmux attach. Render the grid as gpui text runs per row (colors, bold, cursor, selection). Zed does exactly this with its own `terminal` crates built on `alacritty_terminal`; those crates are not published as far as I know (not verified), so this is a rewrite of the view, not a dependency. Needs: resize reporting, scrollback, mouse reporting, copy and paste, bracketed paste, alt screen, wide characters. | High. About 3 weeks and the most likely place for bugs. A cheaper first cut is read-only plus the existing `sendText` prompt, which the daemon already supports. |
| Virtualized lists | n/a (web renders all) | `uniform_list`, `list` (both present, verified). | Low. |
| Links | opener plugin | `cx.open_url(url)` (verified). Web and mail links only, as today. | Low. |
| Native dialogs: folder picker | `tauri-plugin-dialog`, `pick_folder` command | `cx.prompt_for_paths(...)` (verified). | Low. |
| Custom titlebar | `titleBarStyle: Overlay`, traffic lights at x 14 y 19, drag region | `WindowOptions::titlebar` with `appears_transparent` and `traffic_light_position`, and `window.start_window_move()` for drag (both verified to exist in 0.2.2). Linux and Windows draw their own controls: `window_decorations()` is there, the buttons are yours. | Medium on Linux and Windows, low on macOS. |
| Tray icon, hide to tray on close | `tauri` tray, template icon | gpui 0.2.2 has no public tray API that I found (there is a macOS `status_item.rs` in the platform layer, not checked). Use the `tray-icon` crate (what Tauri uses) next to GPUI, and handle close by hiding the window. Not verified that its event loop coexists with GPUI's on Linux. | Medium. Prove in phase 0. |
| Notifications | `tauri-plugin-notification` | Not in GPUI. `notify-rust` or the platform API (not verified which is better on macOS 26: `UNUserNotificationCenter` needs a signed bundle). | Medium. Needs a signed `.app` to test. |
| Keyboard shortcuts, single-key map, key bar | `globalKeys.ts`, `keyMap.ts` | gpui actions and key contexts, which fit this well: a key context per screen replaces "only when focus is not in a text field". The key bar is derived from the active context's bindings. | Low to medium. |
| Router | `router/` | An enum `Route` on the root view, history stack. | Low. |
| Toasts, banner, usage meter, switches, segmented, stepper | React components | Small components, one per style-guide part. | Low each, medium together (about 25 parts). |
| Settings, repos page, agents page, control center (timeline bars), stats heatmap | React views | Plain gpui elements. The heatmap is 53x7 colored divs. The timeline is absolutely positioned bars. | Medium each. |
| Accessibility | roles, `aria-*`, sr-only tables | No tree API found (section 2). | Unsolved. Say so to users, or contribute upstream. |
| Plugin iframes | sandboxed iframes + postMessage | Section 4. | Decision needed. |
| Auto-update, signing, packaging | Tauri bundler | `cargo-bundle` or `cargo-packager`, then notarization; Linux AppImage or deb; Windows MSI. None set up. | Medium. |

## 4. Plugin UI decision

Today (`docs/plugins.md` section 7): plugin UI is a sandboxed `<iframe sandbox="allow-scripts">` served by the daemon from `GET /api/plugins/{id}/ui/*`, in fixed slots (`sidebar.nav`, `sidebar.foot`, `page`, `session.section`, `task.section`, `settings.group`; `composer.action` is already a declarative button), talking to the host through a `postMessage` protocol (`api.call`, `events.subscribe`, `ui.resize`, `ui.toast`, `ui.navigate`). Everything else a plugin does (hooks, MCP servers, agent types, prompts, settings schema, theme) is daemon-side and does not depend on the UI.

Options:

| Option | What it costs | Verdict |
|--------|---------------|---------|
| A. Drop UI plugins, declarative slots only | Plugin authors lose arbitrary HTML and JS. Slots render a small JSON vocabulary: text, key-value rows, progress, buttons, links, markdown, lists. The host keeps the permission model, since the host makes every call. No webview, no sandbox to defend. | Good fit for the small slots. |
| B. Embed a native webview inside plugin slots (`wry` child views, or `gpui-webview`) | A native web view sits above GPUI's Metal surface, so it cannot be clipped by a scrolling `list`, cannot be covered by a GPUI popover or toast, and moves out of sync while scrolling or resizing. It brings WebKit back into the process, which is much of what this migration wants to get rid of. WebView2 and webkit2gtk have to be installed on Windows and Linux. | Reject for slots that live inside scrolling or layered GPUI content. |
| C. Keep a webview only for plugins, in its own window | Same sandbox and protocol as today, unchanged plugin code. Costs the webview RAM only while such a window is open. | Good for the `page` slot, which fills the main area anyway: open it as its own window or panel with no layering. |

Recommendation: A for `sidebar.nav`, `sidebar.foot`, `session.section`, `task.section`, `settings.group` and `composer.action`; C for `page`, opened in a separate window. No B. The postMessage protocol stays as the contract for C; for A the daemon needs one new manifest shape (`contributes.ui[].view` with the declarative JSON), which is a plugin API addition and needs a `docs/plugins.md` decision.

Checked against the official plugins in the `townhall-plugins` repo (manifests at `plugins/*/.townhall-plugin/plugin.json`): only one of the four declares `contributes.ui`, namely `sample-full`, with one `session.section` entry (`ui/session.html`). `agent-types`, `starter-prompts` and `theme-slate` declare no UI. So option A costs nothing for the shipping plugins, but it does break `sample-full`, which is a sample meant to show every capability and should be ported to a declarative `session.section` view or kept as a documented example of the iframe path.

## 5. Architecture

### 5.1 In-process calls or HTTP

Recommendation: in-process daemon, HTTP and websocket client, shared types.

- `src-tauri/src/daemon.rs` already starts the daemon inside the app process on `127.0.0.1:7777`. Keep that: the GPUI binary links `townhall-daemon` the same way.
- The server has to exist anyway: Claude agents call townhall's MCP routes over HTTP (`POST /api/conversations/{id}/mcp`, `POST /api/tasks/{name}/mcp`), plugin hooks and MCP servers call the plugin API over HTTP, and the terminal attach is a websocket. Direct calls would not remove the server, only the loopback hop.
- The route handlers hold logic (`crates/townhall-daemon/src/routes/`, 28 files). Calling `townhall-core` directly would mean either re-implementing that logic in the UI or moving it into core first. That is a refactor of the daemon with no user-visible gain.
- A typed client (`townhall-client`, using `townhall_core::model` for every request and response) gives the compile-time safety that `web/src/api/guards.ts` simulates in TS. The spike already does this for conversations: `api.rs` is 40 lines.
- Revisit later, per hot path, only if profiling shows the loopback matters. The event bus (`Context::bus()`, a tokio broadcast) is the first candidate: a GPUI entity could subscribe to it directly instead of the `/api/events` socket.
- Daemon changes this needs: `POST /api/plugins/{id}/host-token` requires an `Origin` header of an allowed web origin (`cors::browser_origin`), and a native client sends none. Issue the token in-process, or add a native origin. Only relevant to plugin slots (section 4).
- The events socket accepts a client without an `Origin` header (`cors::origin_allowed` returns true for none), so the native client connects as is.

### 5.2 The browser web UI

Verified: the daemon does not serve `web/`. There is no static file route in `crates/townhall-daemon/src`. The browser UI exists only as the Vite dev server (`npm run dev`, port 5173, proxying `/api`), which the CORS allowlist (`cors::ALLOWED_ORIGINS`) names next to the Tauri origins. Production users only ever see the Tauri window.

What dropping `web/` loses:

- Developing and trying the UI in a normal browser with hot reload and browser devtools.
- The Playwright suite (`web/e2e`), which runs against the seeded demo daemon in a browser. It has to be replaced (section 6).
- The browser-only input: the add-repository field takes a git URL in a browser and a folder in the app. The GPUI app can offer both.
- Nothing in production, because nothing serves it.

What stays: the HTTP and websocket API, so a future web or other client is possible. Plan: keep `web/` and Tauri building until phase 5 reaches parity, ship GPUI behind a second binary first, then delete `web/`, `src-tauri/` and the Vite origins from `ALLOWED_ORIGINS` in one change.

## 6. Test strategy

| Today | Replacement |
|-------|-------------|
| vitest unit tests for pure logic (`age`, `sections`, `rowSummary`, `queue`, `drafts`, `scopes`, `keyMap`, ...) | Plain `#[test]` in the Rust crate. Port the logic to Rust once, port the test table with it. This is mechanical. |
| Testing Library component tests (jsdom) | gpui's `TestAppContext` and `VisualTestContext` (`test-support` feature of gpui): simulate keystrokes, clicks and `run_until_parked`, then assert on view state and on the element tree. Not verified in detail here; this is the documented way Zed tests its UI. |
| Parse guard tests | Not needed: serde deserializes `townhall_core::model` types. Keep the daemon's own round-trip tests. |
| Playwright end to end against `scripts/demo.sh` | Two layers: (1) a Rust integration test that starts the daemon on a temp home with the fake `claude` (as `scripts/demo/seed.mjs` does) and drives the app with `TestAppContext`; (2) a short manual and screenshot checklist per release, since gpui has no DOM to drive from outside. Screenshot tests exist in Zed's tooling, not verified for crates.io gpui. |
| Contrast script `check-contrast.mjs` | Keep it, and add a test that the Rust `Theme` equals `tokens.css`. |
| Idle cost | New: a script that launches the release binary against a fixture daemon and fails when idle CPU or RSS goes over a budget. The spike's `--measure` flag is the start of it. |
| Accessibility checks | None available for GPUI today. |

## 7. Phased plan

Each phase ships something usable next to the Tauri app (a second binary, same daemon, same data), so the old app stays the fallback until phase 5. Estimates are engineer-weeks for one person who knows Rust and has built a GPUI app before; add 30 to 50% if not. They are my estimate from reading the code, not from a measured velocity.

| Phase | Ships | Weeks |
|-------|-------|-------|
| 0. Foundations | Workspace crate behind a feature or its own binary, signed and bundled `.app`, in-process daemon, `townhall-client` with events websocket, theme from tokens, JetBrains Mono, component kit (button, kbd, status glyph, menu, banner, toast), key contexts, tray and notification proofs, idle budget script, CI job on macOS with the Metal toolchain. Decide the base: Zed `gpui` or the `gpui-pre` snapshot that `gpui-component` follows. | 3 |
| 1. Read and reply | Sidebar (active, older, search, by repo), conversation thread with markdown, session panel (tasks, repo, context), composer with text only, home with starters, daemon banner, first run. | 5 |
| 2. Attachments | `+ attach`, paste, drop, upload chips, message thumbnails, lightbox, files list. | 2.5 |
| 3. Terminal | Agent pane on `alacritty_terminal`, prompt, stop with confirm, stream status, needs-you answers that send keys. | 3 |
| 4. The rest of the screens | Control center, stats and heatmap, repos, settings, agents and workflows pages, usage meter, toasts and notifications, help overlay, menu drawer for narrow windows. | 4.5 |
| 5. Plugins and parity | Declarative slots, the plugin page window, plugin management pages, key map complete, tests at parity, remove `web/` and `src-tauri/`. | 3 |
| 6. Other platforms | Linux and Windows builds, window controls, packaging, installers, per-OS IME and clipboard checks. Can run in parallel with 4 and 5 if a second person exists. | 3 |
| Total | | about 24 (range 18 to 32) |

Milestone after phase 1 is the first one to get real use: it covers the daily loop (read, reply, see tasks). The decision to continue should be made there, with idle CPU and RSS measured on a build that has markdown and a multiline composer.

## 8. Risks

| Risk | Likelihood and impact | Mitigation |
|------|-----------------------|------------|
| Idle cost is not the webview. If `idle-cpu-profile` finds a polling bug, the migration buys much less. | Unknown, high impact. | Wait for that result before phase 0. |
| `gpui` has no stability promise; the base choice (Zed `gpui` vs `gpui-pre`) is a lock-in. | High, medium impact. | Pin exact versions, wrap GPUI in a thin local component layer, upgrade on purpose. |
| Multiline text input with IME and selection is not provided. | High, high impact (the composer is the main control). | Build it first in phase 1, test on all three OSes before the rest. |
| Terminal view is a rewrite. | High, medium impact. | Read-only first, `alacritty_terminal` as engine, copy Zed's structure from its git source. |
| Markdown with selectable text across blocks. | Medium, medium impact. | Accept per-block selection first. |
| No accessibility story. | Certain, medium impact. | State it, check what upstream plans. |
| Windows and Linux support is unproven here. | Medium, medium impact. | Phase 6 is separate, and the daemon is portable already (macOS and tmux are the current limits, not the UI). |
| Native notifications and tray need a signed bundle to test. | Medium, low impact. | Set up signing in phase 0. |
| Metal Toolchain missing on build machines (this one included). | Certain until fixed. | Install it in CI and on dev machines; `runtime_shaders` is the dev workaround. |
| Plugin authors lose iframes (option A). | Depends on how many plugins use `contributes.ui`. | Count them first; keep option C for `page`. |
| Two UIs to maintain during the migration. | Certain. | Freeze `web/` to fixes only after phase 1. |

## 9. Spike

### What it is

`crates/townhall-gpui` opens a 1200x800 GPUI window styled with the dark tokens of `docs/style-guide` (square corners, 272px sidebar, 36px bars, 24px key bar, amber for you, blue tint for townhall's replies, status glyphs). It reads `GET /api/conversations` and `GET /api/conversations/{id}` from the daemon with the `townhall_core::model` types, shows the conversations in the sidebar (active and older sections, newest first), selects the newest, and renders its thread entries as read-only text (one style per entry kind). Clicking a row loads that conversation. No composer, no markdown, no events socket, no light theme.

It is outside the cargo workspace (`exclude` in the root `Cargo.toml`, and its own `[workspace]` and `Cargo.lock`), for two reasons: CI runs `cargo build --workspace --locked` and `cargo test --workspace --locked` on Linux, where gpui needs system libraries the runners may not have, and a gpui-less workspace keeps the committed `Cargo.lock` unchanged.

Build and run:

```
cargo run --manifest-path crates/townhall-gpui/Cargo.toml --release -- --url http://127.0.0.1:7777 --measure 60
```

`--measure N` makes the process wait 5 s, then report its own CPU time (`getrusage`) over N seconds as a share of one core and its resident size (`task_info`), then exit. It measures the process from inside because the numbers must not depend on another tool being run alongside.

### Did it build

Not as is. With default features the gpui build script fails on this machine: `cannot execute tool 'metal' due to missing Metal Toolchain; use: xcodebuild -downloadComponent MetalToolchain`. I did not install the toolchain (a system-wide download, outside this task). With `features = ["runtime_shaders"]` it builds and runs.

### Numbers

Machine: Apple Silicon (arm64), 12 cores, macOS 26.6.2. Release build. Data: a local fixture server (`python3`, 30 conversations, one with 60 entries of mixed kinds, not the real daemon), because starting the real daemon would have used the user's real `~/.townhall`. The window was open and idle; nobody touched it.

| Run | Window | CPU (share of one core) | RSS |
|-----|--------|-------------------------|-----|
| fixture, `--measure 60` | 5 s settle, then 60 s | 0.67% | 55.5 MB |
| fixture, `--measure 60`, again | same | 0.78% | 55.7 MB |
| fixture, `--measure 5` | 5 s settle, then 5 s | 0.50% | 67.9 MB |

Two earlier runs had nothing listening on the default `127.0.0.1:7777` as far as I know (I did not check, and a real daemon or the Tauri app may have been running there): 14.17% CPU and 107.4 MB over 10 s, and 3.42% and 115.5 MB over 30 s. I did not investigate that difference, so I cannot say whether it is the error state, a real daemon with real, larger conversations, or startup work. If it is a real daemon, the fixture numbers understate a real thread.

### Tauri comparison

Not measured. I could not read another process's CPU and RSS from this session (`ps` and `top` are not allowed to me). The only Tauri figures are the user's: about 25% CPU idle, about 500 MB RAM. The `idle-cpu-profile` task measures the real app. To compare on the same machine, with the app idle for a minute:

```
top -l 13 -s 5 -pid <townhall-app pid> -stats pid,cpu,mem,rsize
```

Note that a WKWebView app spreads its memory over the app process and `com.apple.WebKit.*` helper processes, so RSS has to be summed over all of them to be fair against the single GPUI process.

### What the spike does not show

- I did not see the window. This session has no screenshot tool, so the styling is unchecked by eye. What is checked: the app requested the list and then the newest conversation's detail from the fixture server (the server log shows both requests, in that order, so the list parsed and sorted), and it ran for 60 s.
- Not checked: that the detail response parsed and every entry rendered, scrolling, click selection, behaviour against the real daemon, Linux, Windows.
- The idle numbers are for a view with no timers, no animation and no terminal. They show GPUI does not redraw when nothing changes; they do not predict the full app.
- Menlo stands in for JetBrains Mono.

## 10. Follow-ups

- Read `idle-cpu-profile` before anything else here.
- Install the Metal Toolchain on build machines.
- Count plugins that use `contributes.ui`.
- Run the spike against the real daemon and look at it (`--url http://127.0.0.1:7777`), and measure the Tauri app with the command above.
- If the plan goes ahead: decide the gpui base (Zed `gpui` or `gpui-pre`), then do phase 0.
