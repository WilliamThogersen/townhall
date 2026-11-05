# Web data contract (phase 3)

The layer every phase 3 screen builds on. `docs/daemon-api.md` is the daemon side; this file is the web side: the API functions in `web/src/api/` and the data hooks in `web/src/data/`.

## Ownership

- Unit 3.0 owns `web/src/api/*` (except `attachments.ts` and `search.ts`, which belong to their units), `web/src/data/*` and this file.
- Every other unit imports from them and never edits them. Need a change? Ask the orchestrator; 3.0 makes it.
- Screens import hooks from the barrel `web/src/data` (`import { useConversations } from "../../data"`), never from the files behind it, so the store can move without breaking anyone.
- Types come from `web/src/api/types.ts` (the daemon's types, field for field) and `web/src/data/types.ts` (hook states).

## Errors

- Every API function rejects with `ApiError` (`web/src/api/errors.ts`): `kind` is `network` (daemon unreachable), `http` (the daemon said no; `status` is set) or `protocol` (a response that is not the documented shape).
- `message` is lowercase and has no `error:` prefix (the daemon's own text for `http`). The UI adds the prefix: `error: ${message}`.
- Hooks never throw on a failed load: they put the message in `error`. Actions (`add`, `remove`, `start`, `send`, ...) return promises that reject with the `ApiError`; the caller shows it.

## API functions

All return parsed, readonly data. `signal` aborts the fetch.

| file | function | route | resolves to |
|------|----------|-------|-------------|
| `client.ts` | `listAgents(signal?)` | `GET /agents` | `readonly AgentInfo[]` (with `repo` and `state`) |
| `client.ts` | `getInfo(signal?)` | `GET /info` | `Info`, `userName` always present (`string` or `null`) |
| `client.ts` | `stopAgent`, `sendText`, `getOutput`, `streamUrl` | phase 1 routes | unchanged. no `createAgent`: agents only come from tasks (2.13) |
| `client.ts` | `sendKey(name, key: AgentKey)` | `POST /agents/{name}/send` with `{ key }` | the name. the needs-you block uses it for claude's folder trust dialog (`thread/trustDialog.ts`) |
| `conversations.ts` | `listConversations(signal?)` | `GET /conversations` | `readonly Conversation[]`, newest activity first |
| `conversations.ts` | `getConversation(id, signal?)` | `GET /conversations/{id}` | `ConversationDetail` |
| `conversations.ts` | `startConversation(input: NewMessageInput)` | `POST /conversations` | `ConversationDetail` |
| `conversations.ts` | `sendMessage(id, input: NewMessageInput)` | `POST /conversations/{id}/messages` | `ThreadEntry` (yours) |
| `conversations.ts` | `patchConversation(id, patch: ConversationPatch)` | `PATCH /conversations/{id}` | `Conversation` |
| `conversations.ts` | `stopTurn(id)` | `POST /conversations/{id}/stop` | `Conversation` |
| `conversations.ts` | `endConversation(id)` | `POST /conversations/{id}/end` | `Conversation` |
| `tasks.ts` | `listTasks(filter?: TaskQuery, signal?)` | `GET /tasks?conversation=&repo=&state=` | `readonly Task[]`, oldest first |
| `tasks.ts` | `getTask(name, signal?)` | `GET /tasks/{name}` | `Task` |
| `repos.ts` | `listRepos(signal?)` | `GET /repos` | `readonly Repo[]` |
| `repos.ts` | `addRepo(input: AddRepoInput)` | `POST /repos` | `Repo` |
| `repos.ts` | `removeRepo(name)` | `DELETE /repos/{name}` | the removed `Repo` |
| `stats.ts` | `getStats(repo?, signal?)` | `GET /stats?repo=` | `Stats` |
| `steps.ts` | `getSteps(agent, repo?, signal?)` | `GET /agents/{agent}/steps?repo=` | `readonly Step[]` |
| `settings.ts` | `getSettings`, `putSetting` | settings routes | unchanged |

`TaskQuery` is `{ conversation?: string; repo?: string; state?: TaskState }`; empty values are left out of the query.

### Parse guards

One guard per response type, each `(value: unknown) => T`, throwing a `protocol` `ApiError` that names the field (`unexpected response shape: task.state`):

- `guards.ts`: `parseEnvelope`, `parseAgent(s)`, `parseAgentState`, `parseAgentInfo(s)`, `parseInfo`, `parseRepo(s)`, `parseString`
- `conversationGuards.ts`: `parseConversation(s)`, `parseContextUsage`, `parseEntryBody`, `parseThreadEntry`, `parseAttachment(s)`, `parseConversationDetail`
- `taskGuards.ts`: `parseTask(s)`, `parsePr`
- `statsGuards.ts`: `parseStep(s)`, `parseShippedDay`, `parseStats`
- `eventGuards.ts`: `parseServerEvent`
- `guardKit.ts`: the building blocks (`record`, `str`, `num`, `bool`, `nullable`, `oneOf`, `list`, `strings`, `listOf`) for a unit that adds its own API file (`attachments.ts`, `search.ts`)

A nullable field accepts `null` or a missing key and always comes out as `null`.

### Events socket: `events.ts`

- `eventsUrl(loc?, win?)`: `ws(s)://<page host>/api/events` in a browser, `ws://127.0.0.1:7777/api/events` on desktop (same base as `streamUrl`).
- `connectEvents(onEvent, options?) -> { close() }`: one socket to `/api/events`. Each text frame is parsed into a `ServerEvent`; a frame that is not JSON, not a known `type` or not the documented shape is skipped. On close it reconnects after 500 ms, doubling to at most 10 s, and the delay resets once a socket opens. `close()` stops it for good.
- `options`: `onStatus(status: ConnectionStatus)` (`"connecting" | "open" | "closed"`), `url`, `openSocket` (a test seam), `baseDelayMs`, `maxDelayMs`.
- Screens never call it: `DataProvider` owns the only socket.

## Data hooks

One `DataProvider` per app (`App.tsx` wraps `AgentsProvider` and the shell in it). Every hook below throws `data hooks need a DataProvider` outside one.

Common rules:

- `loading` is `true` only until the first answer for the current input (id, filter, repo). Refetches (resync, reconnect, `reload`, polling) keep the data on screen and do not set `loading`.
- `error` is the last failure (no `error:` prefix), cleared by the next success. A failed refetch keeps the last good data.
- Lists are `[]` while loading and on a first-load error, except `repos`, which is `null` until the first answer (so a screen can tell "no repos" from "not known yet").
- Data loads on demand: nothing is fetched until a hook that needs it mounts. Once loaded it stays in the store and stays live (see below), so a screen that mounts again shows it at once. A resource whose first load failed is fetched again when a hook for it mounts.

| hook | returns | while loading | on error |
|------|---------|---------------|----------|
| `useConversations()` | `{ conversations: readonly Conversation[]; loading; error }`, newest `updatedAt` first | `[]`, `loading: true` | last good list (or `[]`), `error` |
| `useConversation(id: string \| null)` | `{ detail: ConversationDetail \| null; loading; error; reload(): void }` | `detail: null`; `null` id is idle (`loading: false`) | last good detail (or `null`), `error` |
| `useTasks(filter?: { conversationId?; repo? })` | `{ tasks: readonly Task[]; loading; error }`, oldest first | `[]` | last good list, `error` |
| `useAgentInfos()` | `{ agents: readonly AgentInfo[]; loading; error }` | `[]` | last good list, `error` |
| `useRepos()` | `{ repos: readonly Repo[] \| null; loading; error; add(input: AddRepoInput): Promise<Repo>; remove(name): Promise<void> }` | `repos: null` | last good list, `error`; `add` / `remove` reject with `ApiError` |
| `useStats(repo?: string)` | `{ stats: Stats \| null; loading; error }` | `null` | last good stats, `error` |
| `useSteps(agent: string \| null, options?: { live?; repo?; intervalMs? })` | `{ steps: readonly Step[]; loading; error }` | `[]`; `null` agent is idle | last good list, `error` |
| `useConversationActions()` | `{ start(input) -> Promise<ConversationDetail>; send(id, input) -> Promise<ThreadEntry>; stop(id) -> Promise<Conversation>; end(id) -> Promise<Conversation>; patch(id, patch) -> Promise<Conversation> }` | | each rejects with `ApiError` |
| `useConnection()` | `"connecting" \| "open" \| "closed"` | `"connecting"` | `"closed"` while waiting to reconnect |

`useTasks` filters one live list of every task in the browser (`listTasks()` without a filter), so all filters share one fetch and one event stream; `repo` matches ignoring case. `useSteps` polls every `STEPS_POLL_MS` (2 s, or `intervalMs`) while `live` is true and `agent` is not `null`; steps have no event. `useStats` has no event either: it loads once per `repo` and again on resync or reconnect.

## Live updates

```
/api/events frame -> readFrame -> ServerEvent -> store reducer -> hooks (useSyncExternalStore) -> screen
```

The store lives inside `DataProvider` and only ever replaces objects (immutable updates). Per event:

| event | store change |
|-------|--------------|
| `conversationUpdated` | replaces that conversation in the list (adds it when new, keeps newest activity first) and in a loaded detail; a copy with an older `updatedAt` than the one held is ignored |
| `entryAppended` | in a loaded detail: entries stay ordered by `id`; an entry whose `id` is there already replaces it (a `tasksCreated` entry grows in place) |
| `taskUpdated` | replaces or adds the task (by `name`, ignoring case) in the task list and in the loaded detail of its `conversationId`, then refetches the agent list when it is loaded (a task start or end adds or removes an agent) |
| `agentUpdated` | replaces or adds the agent in the agent list (by `repo` and `name`, ignoring case) |
| `resync` | refetches everything that is loaded |

- Every reconnect (an `open` after the first) also refetches everything that is loaded, since events were missed while closed. The first `open` refetches only what had already finished loading before it, since those answers may predate the socket.
- Resync and reconnect bump a generation counter that `useStats` and `useSteps` follow, so they refetch too.
- The agent list is also refetched every 10 s (`AGENTS_POLL_MS`) while it is loaded: `agentUpdated` only fires when a state changes.
- An event that arrives while a resource is loading is replayed onto the answer, so nothing is lost between the request and the socket. A refetch asked for while one is in flight runs once more after it.
- Actions feed their answers into the store as well (a started conversation with its detail and tasks, your sent entry, a patched, stopped or ended conversation, an added or removed repo), so a screen does not wait for the event.
- Errors from loads are stored without the `error:` prefix (a stray one from the daemon is stripped).

Files behind the barrel (3.0 only): `state.ts` (the state shape), `reducers.ts` (pure, immutable updates per event), `resources.ts` (what each resource fetches and how events reduce it), `loader.ts` (loads, in-flight replay), `eventsLink.ts` (socket, resync, reconnect, agent poll), `cell.ts`, `store.ts`, `storeActions.ts`, `dataContext.ts`, `DataProvider.tsx` and one file per hook.

## Testing screens

- Wrap the screen in `TestDataProvider` from `web/src/data/testing.tsx`. It uses a fake socket, never the network: `createFakeEvents(initial = "open")` gives `{ connect, emit(event), setStatus(status), isConnected() }`; pass it as `<TestDataProvider events={fake}>` and call `act(() => fake.emit({ type: "taskUpdated", task }))` to drive live updates.
- Stub the daemon with `vi.spyOn(conversationsApi, "listConversations").mockResolvedValue(...)` (import `* as conversationsApi from "../../api/conversations"`), as phase 2 tests do with `client`.
- Builders for every type are in `web/src/api/fixtures.ts`: `aConversation`, `aDetail`, `anEntry`, `aYouEntry`, `aTask`, `anAttachment`, `anAgentInfo`, `aRepo`, `aStep`, `someStats`, each taking overrides.
- A test that renders the whole `App` stubs the socket: `vi.spyOn(eventsApi, "connectEvents").mockReturnValue({ close: () => undefined })`.
