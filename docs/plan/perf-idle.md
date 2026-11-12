# Where idle CPU and RAM go in the desktop app

Status: partly measured. Read "What was and was not measured" first.

The report that started this: about 25% CPU and about 500 MB RAM while the app is "idle" (macOS, user's own measurement, not reproduced here).

## What was and was not measured

| Item | State |
| --- | --- |
| townhall-daemon alone, empty home, release build | Measured (below) |
| Main process (src-tauri shell), WebContent, GPU, Networking in the four GUI scenarios | Not measured |
| Timer, poll and request counts | Counted from the source, not from a running app |
| Re-renders per second | Not measured, derived from timers and poll rates |
| Every CPU and RAM saving in the cause list | Estimate, not measurement |

Why the GUI rows are empty: the agent that wrote this had no `ps`, `top`, `sample` or `xctrace`, and could not drive a GUI (open conversations, show a terminal, minimise the window). A release Tauri build would have produced nothing usable without those, so it was not built. The script under "How to measure the GUI scenarios" fills the table in about ten minutes. Until then the ranking below is a ranking by evidence from the code, not by measured cost.

## Measured: the daemon alone

Method: `cargo build --release -p townhall-daemon`, then a python3 harness spawned the binary with an empty `TOWNHALL_HOME`, its own tmux socket, a non-default port and empty repo and Claude project folders (so it could not touch the live townhall). It waited, sent SIGTERM and read `wait4` rusage (user and system CPU, peak RSS). Two runs of different length; the difference removes startup cost. The harness polled `/api/system` once per 0.2 s only until the first answer.

| Run | Wall | User CPU | System CPU | Peak RSS |
| --- | --- | --- | --- | --- |
| A | 20.0 s | 0.017 s | 0.044 s | 23.2 MB |
| B | 80.2 s | 0.030 s | 0.044 s | 23.1 MB |
| B minus A (60.2 s) | 60.2 s | 0.013 s | 0.000 s | n/a |

Idle floor of the daemon with nothing to do: about 0.02% of one core and about 23 MB. This is a floor: no agents, no tmux server, no open sockets, no tasks. Everything below that the daemon does with agents is not covered by this number. Peak RSS of the daemon is also part of the src-tauri process in the app, because the daemon runs in-process there.

## Table to fill: per process and scenario

CPU is percent of one core averaged over 60 s, RSS in MB. Fill from the script below.

| Scenario | Main (townhall-app) | WebContent | GPU | Networking |
| --- | --- | --- | --- | --- |
| No conversation open | not measured | not measured | not measured | not measured |
| One conversation open | not measured | not measured | not measured | not measured |
| Agent terminal visible | not measured | not measured | not measured | not measured |
| Window hidden to tray | not measured | not measured | not measured | not measured |
| Daemon alone, empty home | 0.02% / 23 MB (measured) | n/a | n/a | n/a |

## Counted from the source: timers, polls and request rates

Frontend (web/src), values as in the code:

| Source | Period | Hits the daemon | Pauses when hidden |
| --- | --- | --- | --- |
| `ui/Spinner.tsx` shared `setInterval` | 80 ms (12.5 ticks/s) while any Spinner is mounted | no | no |
| `data/useSteps.ts` (live views) | 2 s | `GET steps` | no |
| `agents/useAgents.ts` | 3 s | `GET /agents` | no |
| `data/eventsLink.ts` agents refetch | 10 s | `GET /agents` | no |
| `data/useRuns.ts` | 5 s | yes | no |
| `data/useArtifacts.ts` | 5 s | yes | no |
| `views/control/StepsLoader.tsx` live steps | 5 s | yes | no |
| `shell/SystemStats.tsx` | 5 s | `GET /api/system` | yes (tested) |
| `data/useUsage.ts` | 60 s | yes | no |
| `sidebar/useNow.ts` | 30 s, local re-render | no | no |
| `views/control/useNow.ts` | 15 s, local re-render | no | no |
| `agents/attachSocket.ts` + `agents/Terminal.tsx` | PTY stream while a terminal is mounted | websocket | no |
| xterm `cursorBlink: true` | blink timer per mounted terminal | no | no |

If every poller is mounted at once the upper bound is about 1.75 requests per second (1/3 + 1/10 + 1/2 + 4/5 + 1/60, the 5 s ones being runs, artifacts, live steps and system), plus the event websocket. That is a ceiling from the code, not a measurement; most views mount only some of them.

Daemon (crates):

| Loop | Period | Work per pass |
| --- | --- | --- |
| Agent watcher `agent/watch.rs` | 1.5 s | `fleet::list`, which asks tmux (`list-panes -a`, a subprocess; answers are cached 1 s in `tmux/panes.rs`) |
| Scheduler `scheduler/mod.rs` | 2 s, also on task events | `pipeline::drive`, `all_tasks` (SQLite), reflect, verify, CI, launch, `merged::detect` |
| `routes/stream.rs` | 500 ms per connected stream | `agent::output` (capture-pane) while a client is connected |
| Reaper `reaper/mod.rs` | 5 min | |
| Usage service `usage/service.rs` | 5 min | network and keychain |
| Background prune `background.rs` | 1 h | |

The Tauri shell itself (src-tauri/src) has no timers; only the in-process daemon, the tray and the window handlers.

## Ranked causes

Ranking is by how much steady, avoidable work the code does while nothing happens, weighted by how often that work is on. Each saving is an estimate to be checked against the measured table.

### Fixable in the current Tauri app

1. **The spinner clock ticks at 12.5 Hz and runs whenever any working spinner exists.** `ui/Spinner.tsx` runs one `setInterval(tick, 80)` and each tick notifies every Spinner through `useSyncExternalStore`. Spinners appear in the sidebar status glyph, the conversation header, thread lines, span rows, buttons and the pane head, so any agent that is "working" keeps it on, and so does an agent that sits at a prompt while a task is running. It does not pause on `visibilitychange`. Evidence: source (Spinner.tsx:5, 27). Fix: draw the spinner with a CSS `steps()` animation on a pseudo-element (no JS, no React), or at minimum tick at 8 to 10 Hz and stop when the document is hidden. Expected saving: the largest single idle item in the WebContent process while a spinner shows, estimated 3 to 8 points of one core; all of it when nothing is spinning.
2. **Mounted terminals do continuous work.** `agents/Terminal.tsx` uses xterm's DOM renderer (no WebGL or canvas addon), `cursorBlink: true` and `scrollback: 5000`. Each terminal is a PTY running `tmux attach-session` over a websocket (`routes/attach.rs`), so a live Claude TUI that animates its own spinner or status line writes into the DOM several times a second even when the user is doing nothing. Hidden panes keep their terminal and connection unless the component unmounts (not confirmed per view; check). Evidence: source (Terminal.tsx:46 to 55). Fix: turn cursor blink off when the terminal is not focused, disconnect and dispose terminals that are not visible (reattach on show), lower scrollback to 1000 to 2000, use the WebGL addon for the visible one. Expected saving: 3 to 10 points of WebContent CPU and tens of MB per retained terminal in the "agent terminal visible" scenario.
3. **Polling that ignores visibility and always re-renders.** The pollers in the table above run on fixed intervals and only `SystemStats` stops when the window is hidden. `data/useLoad.ts` calls `setSnapshot` with a fresh object and fresh `data` on every success, so every poll re-renders its consumers and rebuilds downstream arrays even if the response is identical. An event websocket already exists (`api/events.ts`, `eventsLink.ts`). Evidence: source (useLoad.ts:34, useAgents.ts:38, useSteps.ts:23). Fix: skip `setState` when the new JSON equals the old, pause pollers on `visibilitychange`, merge the duplicate agent polls (3 s `useAgents` plus 10 s `eventsLink`), drive steps, runs and artifacts from events with a slow safety poll. Expected saving: 1 to 3 points of WebContent CPU, up to about 1.75 requests per second removed from the daemon and Networking process, fewer allocations.
4. **Daemon passes that run every 1.5 s and 2 s regardless of activity.** The watcher spawns a tmux subprocess and diffs all agents; the scheduler runs a full pass (SQLite read of all tasks, pipeline, merge detection that may call git). The measured floor (0.02%) shows this is negligible with no agents; with agents it is not measured. Fix: back off to 5 to 10 s when no agent is working and no task is active, wake on events, cache `merged::detect`. Expected saving: under 1 point of CPU in the main process, plus fewer subprocess spawns and wakeups (battery).
5. **`GET /api/system` rebuilds the disk list on every call** (`system.rs`: `Disks::new_with_refreshed_list`) and the key bar shows it every 5 s. Fix: refresh only the used disk, or poll every 10 to 30 s. Expected saving: well under 1 point; listed because it is the only polled endpoint that touches the OS each time.
6. **Plugin iframes.** `plugins/PluginFrame.tsx` mounts an iframe per plugin slot; each iframe is its own JS context. Not measured; count iframes in the running app before acting. Fix: mount on demand. Expected saving: tens of MB each if they are always mounted.

### Inherent to the webview

7. **The WebKit process set.** A WebKit app always carries WebContent, GPU and Networking processes. A React 19 app with xterm and react-markdown typically holds a JS heap and DOM in WebContent worth well over 100 MB, with the GPU and Networking processes adding tens of MB each; this is the bulk of the "about 500 MB" and none of it is reduced by fixing polling. These ranges are general experience, not measured here. Quick fixes can trim the heap (fewer retained terminals and scrollback, no duplicate data copies) but not the floor.
8. **Compositing and timer wakeups.** Any visible animation or timer wakes WebContent and GPU processes; the GPU process cost of a mostly static UI is small but not zero. Reduced by fixes 1 to 3, not removed.
9. **Process hops.** Every poll crosses WebContent, Networking and the in-process daemon (loopback HTTP). Fix 3 reduces the count; the hop itself stays.

## Estimate after the quick fixes

These are estimates from the code, to be replaced once the table above is filled in.

- Fixes 1 to 3 (spinner to CSS, terminals paused when not visible, visibility-aware and change-aware polling): idle CPU from about 25% to about 3 to 8% of one core in the visible-window scenarios, and to near 0% when the window is hidden to the tray. If the measured WebContent number is much lower than the user's 25%, the cause is outside this list and the first thing to check is a hidden terminal's PTY output.
- RAM from about 500 MB to about 350 to 420 MB: the reachable part is terminal buffers and duplicated data in WebContent. The WebKit process floor is what remains.
- What only a native UI such as GPUI would remove: the WebContent, GPU and Networking processes as such, i.e. the roughly 250 to 350 MB floor and the per-poll process hops. A native app would still pay for the daemon (about 23 MB, 0.02% idle, measured) and for text rendering of terminals. The estimate is that the quick fixes capture most of the CPU gain and a native UI is justified mainly by RAM and by the process floor, not by idle CPU. Verify with the measured numbers before deciding on the rewrite.

## How to measure the GUI scenarios

Close Safari and other WebKit apps first: WebKit helper processes are not children of the app, so they are found by name and would be mixed up. Build with `npm --prefix web ci && cargo tauri build` (or the repo's release command), start the app, put it in the scenario, then run the script for each scenario. `top` is used because `ps %cpu` on macOS is a lifetime average, not the last minute.

```sh
#!/bin/sh
scenario="$1"
top -l 2 -s 60 -stats pid,command,cpu,rsize -o cpu -n 60 > "/tmp/perf-idle-$scenario.txt"
awk '/^PID/ { n++ } n == 2' "/tmp/perf-idle-$scenario.txt" \
  | grep -E 'PID|townhall|WebKit.WebContent|WebKit.GPU|WebKit.Networking'
```

For a deeper look at the busy process: `sample <pid> 10 -file /tmp/webcontent.txt`, or `xcrun xctrace record --template 'Time Profiler' --attach <pid> --time-limit 30s --output /tmp/wc.trace`. In Safari's Web Inspector (enable Develop menu, Develop, the app, the webview) the Timelines tab shows timers, layout and JS per second, which gives the real re-render and timer counts to replace the derived ones above.

## Notes

- Branch was up to date with origin/main at the time of writing; nothing was merged.
- No application code was changed. The measurement harness lived in the agent's scratch directory and is not part of the repo.
