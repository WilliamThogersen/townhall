# Where idle CPU and RAM go in the desktop app

Status: idle start view measured for all four processes; the other scenarios need clicking and are not measured. Read "What was and was not measured" first.

The report that started this: about 25% CPU and about 500 MB RAM while the app is "idle" (macOS, user's own measurement, not reproduced here).

Headline: on the idle start view the whole app is about 1.3% of one core and about 116 MB across the four processes. That does not reproduce 25% and 500 MB. Whatever produces the user's numbers is not present on the start view, so it is in the scenarios that need agents, conversations or terminals.

## What was and was not measured

| Item | State |
| --- | --- |
| townhall-daemon alone, empty home, release build | Measured (below) |
| Main process, WebContent, GPU, Networking, idle start view, 60 s, release build | Measured (below) |
| `sample` of main and WebContent, 10 s each, idle start view | Measured (below) |
| One conversation open, agent terminal visible | Not measured: needs clicking |
| Window hidden to tray | Not measured: no flag or deep link hides the window; it only hides on close (`lib.rs:85`), which needs clicking |
| Timer, poll and request counts | Counted from the source, not from a running app |
| Re-renders per second | Not measured; the WebContent sample shows about 1.4 rendering updates per second on the start view |
| Every CPU and RAM saving in the cause list | Estimate, not measurement |

## How the GUI numbers were taken

- Build: `npm --prefix web ci`, then `npm run tauri -- build --bundles app` (release). `cargo tauri` is not installed; the repo's `@tauri-apps/cli` is used through `npm run tauri`.
- Launch: `open -n --env TOWNHALL_HOME=<empty scratch dir> .../Townhall.app`.
- Isolation was not possible for the port. `127.0.0.1:7777` is hardcoded (`src-tauri/src/daemon.rs:15`) and a dev townhall-app already held it, so the measured app ran in external mode: it adopted that daemon and did not run one in-process. The isolated `TOWNHALL_HOME` kept the app's own store and settings away from the real ones. The webview talked to the existing daemon on 7777 read-only (first-run view, nothing clicked). The main-process number therefore excludes the in-process daemon; add the daemon floor below (about 0.02% and 23 MB) for the embedded case.
- Processes: main pid from `pgrep -fl` on the bundle path. The WebKit GPU, Networking and WebContent helpers were the three new `com.apple.WebKit.*` pids that appeared at launch (other apps' WebKit helpers were already running and were left alone). Only those four pids were sampled and only the app's main pid was terminated at the end.
- CPU and RSS: `top -l 13 -s 5 -stats pid,command,cpu,rsize,threads,idlew -o cpu -n 100`, filtered to the four pids, about 60 s after a 20 s settle. CPU is percent of one core per 5 s interval.
- Stacks: `sample <pid> 10` on main and on WebContent, 1 ms interval.
- The window was not looked at; the view was the first-run view (`no_repo_folder` in `commands.rs`), assumed from the empty isolated settings.

## Measured: the daemon alone

Method: `cargo build --release -p townhall-daemon`, then a python3 harness spawned the binary with an empty `TOWNHALL_HOME`, its own tmux socket, a non-default port and empty repo and Claude project folders (so it could not touch the live townhall). It waited, sent SIGTERM and read `wait4` rusage (user and system CPU, peak RSS). Two runs of different length; the difference removes startup cost. The harness polled `/api/system` once per 0.2 s only until the first answer.

| Run | Wall | User CPU | System CPU | Peak RSS |
| --- | --- | --- | --- | --- |
| A | 20.0 s | 0.017 s | 0.044 s | 23.2 MB |
| B | 80.2 s | 0.030 s | 0.044 s | 23.1 MB |
| B minus A (60.2 s) | 60.2 s | 0.013 s | 0.000 s | n/a |

Idle floor of the daemon with nothing to do: about 0.02% of one core and about 23 MB. This is a floor: no agents, no tmux server, no open sockets, no tasks. Everything below that the daemon does with agents is not covered by this number. Peak RSS of the daemon is also part of the src-tauri process in the app, because the daemon runs in-process there.

## Per process and scenario

CPU is percent of one core, RSS in MB. Idle start view: 11 five-second intervals, mean CPU and typical RSS. The twelfth interval is excluded and shown below.

| Scenario | Main (townhall-app) | WebContent | GPU | Networking |
| --- | --- | --- | --- | --- |
| No conversation open (idle start view, measured) | 0.12% / 36 MB | 0.42% / 51 to 53 MB | 0.23% / 17 to 20 MB | 0.53% / 8.4 MB |
| One conversation open | needs clicking | needs clicking | needs clicking | needs clicking |
| Agent terminal visible | needs clicking | needs clicking | needs clicking | needs clicking |
| Window hidden to tray | needs clicking | needs clicking | needs clicking | needs clicking |
| Daemon alone, empty home | 0.02% / 23 MB (measured) | n/a | n/a | n/a |

Per-interval ranges: main 0.1 to 0.3%, WebContent 0.3 to 0.6%, GPU 0.2 to 0.3%, Networking 0.3 to 0.7%. Sum of the four: about 1.3% of one core and about 116 MB.

The last interval was different: WebContent 2.2% and 69 MB, GPU 0.7% and 121 MB, main 0.4%. It coincided with the end of the run and nothing was done to the app; cause unknown, not reproduced, not included in the means. It shows the GPU process can briefly reach 100+ MB.

### Hot stacks from `sample` (10 s, idle start view)

WebContent main thread, 8855 samples:
- 8820 (99.6%) idle in the run loop (`mach_msg`). 35 samples (0.4%, matching `top`) did any work.
- 21 samples in WebCore timers: 14 in `RemoteLayerTreeDrawingArea::updateRendering` (layout then paint: `RenderBlock::paint`, `RenderFlexibleBox::paintChildren`, `RenderLayer::paintLayer`), 4 in `WindowEventLoop` running a JS event listener, 2 in a `DOMTimer` whose callback calls `fetch` (a poll), 1 in backing-store volatility marking. 12 further samples in a mutex unlock.
- No sample in a JS spinner tick or in xterm: no terminal and no spinner is mounted on this view.
- All other threads (JSC heap helpers, JIT worklist, scavenger) idle.

Main process, 8010 samples per thread:
- Main thread: 8008 idle in the AppKit run loop; about 25 samples in `CA::Layer::commit_if_needed` (Core Animation commits).
- 12 `tokio-rt-worker` threads, all parked in `kevent` or `park_condvar`; about 12 samples of work in total across them.
- `CVDisplayLink` threads (3) waiting; one display-refresh callback woke the run loop once. `tracing-appender`, `Log work queue`, `WebCore: Scrolling` idle.
- Nothing in the app's own code shows as hot.

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

Ranking is by evidence. The idle start view was measured and samples taken; the scenarios that carry the user's 25% and 500 MB (conversation open, agent terminal visible) were not reachable without clicking, so causes that only exist there stay ranked by code evidence and are marked unconfirmed. Each saving is an estimate unless stated.

### Confirmed by the samples (idle start view)

1. **Polling and the render updates it triggers.** The WebContent main thread was busy 0.4% of the time; of 21 timer samples, 14 were `updateRendering` (layout and paint of block and flex boxes), 4 a JS event listener and 2 a `DOMTimer` callback calling `fetch`. About 1.4 rendering updates per second, in line with the poll periods in the table below, not a 12.5 Hz clock. Networking is the busiest helper relative to its size (0.53%), which is the request traffic. Evidence: sample. Fix as before: skip `setState` on identical JSON (`data/useLoad.ts:34`, `useAgents.ts:38`, `useSteps.ts:23`), pause pollers on `visibilitychange`, merge the duplicate agent polls, drive from events. Corrected estimate: the whole WebContent process costs 0.42% here, so the saving on the start view is at most about 0.4 points of WebContent plus part of the 0.5 in Networking, not the earlier 1 to 3 points. It matters more with more views mounted (more pollers, a bigger DOM to lay out per update; up to about 1.75 requests per second when every poller is mounted).

### Not reached by the measurement: ranked by code, unconfirmed

2. **Mounted terminals do continuous work.** `agents/Terminal.tsx` uses xterm's DOM renderer (no WebGL or canvas addon), `cursorBlink: true` and `scrollback: 5000`. Each terminal is a PTY running `tmux attach-session` over a websocket (`routes/attach.rs`), so a live Claude TUI that animates writes into the DOM several times a second even when the user is doing nothing. Hidden panes keep their terminal and connection unless the component unmounts (not confirmed per view). Evidence: source (Terminal.tsx:46 to 55). Not on the start view, so not in any sample. It is now the first suspect for the gap between 1.3% measured and 25% reported, because it is the only listed item that scales with live agent output. Fix: cursor blink off when not focused, dispose terminals that are not visible, scrollback 1000 to 2000, WebGL addon for the visible one. Estimate 3 to 10 points of WebContent and tens of MB per retained terminal (unmeasured).
3. **The spinner clock at 12.5 Hz.** `ui/Spinner.tsx` runs one `setInterval(tick, 80)` that notifies every Spinner through `useSyncExternalStore` while any Spinner is mounted; it does not pause on `visibilitychange` (Spinner.tsx:5, 27). No spinner is mounted on the start view and the sample shows no 80 ms timer, so this was not exercised; the 3 to 8 points estimate is unconfirmed and was ranked first before the measurement. With the whole WebContent at 0.4% for about 1.4 updates per second, 12.5 notifies plus layout per second would plausibly cost several times that, but that is arithmetic, not a measurement. Fix: CSS `steps()` animation, or 8 to 10 Hz and stop when hidden.
4. **Daemon passes every 1.5 s and 2 s.** The main process was idle in this run (12 tokio workers parked, about 12 work samples in 10 s, 0.12%), but the daemon ran in another process (external mode), so this says nothing about those passes. The daemon floor (0.02%, 23 MB) holds with no agents; with agents it is not measured. Fix: back off to 5 to 10 s when nothing is active, wake on events, cache `merged::detect`. Estimate under 1 point.
5. **`GET /api/system` rebuilds the disk list on every call** (`system.rs`: `Disks::new_with_refreshed_list`), every 5 s. Well under 1 point; the only polled endpoint that touches the OS each time. Unmeasured.
6. **Plugin iframes.** `plugins/PluginFrame.tsx` mounts an iframe per plugin slot; each is its own JS context. Not measured; count iframes in a running view before acting. Estimate tens of MB each if always mounted.

### Inherent to the webview

7. **The WebKit process set.** Measured on the start view: WebContent 51 to 53 MB, GPU 17 to 20 MB (121 MB in one interval), Networking 8.4 MB, plus the main process at 36 MB, about 116 MB in total. The earlier claim that WebContent alone is "well over 100 MB" and the bulk of the 500 MB is contradicted for the start view. The rest of the reported 500 MB must come from views with content (DOM and JS heap growth, xterm buffers), the in-process daemon, or other processes counted with it. Not measured.
8. **Compositing and timer wakeups.** Seen in the sample as `CA::Layer::commit_if_needed` in the main process (about 25 samples) and the GPU process at 0.23%. Small at idle. Reduced by fixes 1 and 3, not removed.
9. **Process hops.** Every poll crosses WebContent, Networking and the daemon over loopback HTTP. Networking at 0.53% is the cost of this on the start view. Fix 1 reduces the count; the hop stays.

## Estimate after the quick fixes

Replaces the earlier estimate, which assumed the 25% was visible on the start view. It is not.

- Idle start view: already about 1.3% and 116 MB. Fix 1 can only take a fraction of a point off it. A native UI would not materially improve idle CPU here.
- The user's 25% and 500 MB sit in the unmeasured scenarios. The next measurement is one conversation open with a live agent, then a visible terminal, taking the same `top` series and a `sample` of WebContent. If WebContent shows xterm rendering or the websocket message handler, fix 2 is the priority; if it shows a Spinner tick, fix 3.
- A decision on a native UI (GPUI) should wait for those numbers: on the start view the three WebKit helpers cost about 80 MB and 1.2% together, much less than the 250 to 350 MB floor estimated earlier.

## How to measure the other scenarios

Ignore other WebKit apps: helper processes are not children of the app, so identify the new `com.apple.WebKit.*` pids that appear at launch. Build and launch as under "How the GUI numbers were taken", put the app in the scenario by clicking, then take the `top` series and a `sample` for each process. `top` is used because `ps %cpu` on macOS is a lifetime average.

```sh
top -l 13 -s 5 -stats pid,command,cpu,rsize,threads,idlew -o cpu -n 100 | grep -E "^(<main>|<gpu>|<net>|<wc>) "
sample <pid> 10 -file /tmp/sample.txt
```

Also available: `xcrun xctrace record --template 'Time Profiler' --attach <pid> --time-limit 30s --output /tmp/wc.trace`. In Safari's Web Inspector (Develop menu, the app, the webview) the Timelines tab shows timers, layout and JS per second.

## Notes

- Branch was up to date with origin/main at the time of writing; nothing was merged.
- No application code was changed. The measurement harness lived in the agent's scratch directory and is not part of the repo.
