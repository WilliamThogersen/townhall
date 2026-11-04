import type { ConnectEvents, ConnectionStatus, EventsConnection } from "../api/events";
import type { ServerEvent } from "../api/types";
import type { Cell } from "./cell";
import type { Loader } from "./loader";
import { AGENTS_KEY, loadableFor, requestedKeys } from "./resources";

export const AGENTS_POLL_MS = 10_000;

export interface EventsLinkDeps {
  readonly connect: ConnectEvents;
  readonly agentsPollMs?: number;
}

export interface EventsLink {
  readonly start: () => void;
  readonly stop: () => void;
}

export function createEventsLink(
  cell: Cell,
  loader: Loader,
  apply: (event: ServerEvent) => void,
  deps: EventsLinkDeps,
): EventsLink {
  let connection: EventsConnection | null = null;
  let poll: ReturnType<typeof setInterval> | undefined;
  let openedBefore = false;

  const refetchAll = () => {
    cell.update((state) => ({ ...state, generation: state.generation + 1 }));
    requestedKeys(cell.get()).forEach(loader.load);
  };
  const refetchSettled = () => {
    const state = cell.get();
    requestedKeys(state).filter((key) => loadableFor(key).read(state)?.settled).forEach(loader.load);
  };
  const refetchAgents = () => {
    if (cell.get().agents !== null) loader.load(AGENTS_KEY);
  };
  const onEvent = (event: ServerEvent) => {
    if (event.type === "resync") return refetchAll();
    apply(event);
    if (event.type === "taskUpdated") refetchAgents();
  };
  const onStatus = (status: ConnectionStatus) => {
    cell.update((state) => (state.connection === status ? state : { ...state, connection: status }));
    if (status !== "open") return;
    if (openedBefore) return refetchAll();
    openedBefore = true;
    refetchSettled();
  };

  return {
    start: () => {
      if (connection) return;
      openedBefore = false;
      connection = deps.connect(onEvent, { onStatus });
      poll = setInterval(refetchAgents, deps.agentsPollMs ?? AGENTS_POLL_MS);
    },
    stop: () => {
      connection?.close();
      connection = null;
      clearInterval(poll);
    },
  };
}
