import type { ServerEvent } from "../api/types";
import { createCell } from "./cell";
import { createEventsLink, type EventsLinkDeps } from "./eventsLink";
import { createLoader } from "./loader";
import { applyEvent } from "./reducers";
import { INITIAL_STATE, type DataState } from "./state";
import { PLUGINS_KEY } from "./resources";
import {
  createConversationActions,
  createPluginActions,
  createRepoActions,
  type PluginActions,
  type RepoActions,
} from "./storeActions";
import type { ConversationActions } from "./types";

export type DataStoreDeps = EventsLinkDeps;

export interface DataStore extends RepoActions, PluginActions {
  readonly getState: () => DataState;
  readonly subscribe: (listener: () => void) => () => void;
  readonly start: () => void;
  readonly stop: () => void;
  readonly ensure: (key: string) => void;
  readonly refetch: (key: string) => void;
  readonly actions: ConversationActions;
}

export function createDataStore(deps: DataStoreDeps, initial: DataState = INITIAL_STATE): DataStore {
  const cell = createCell(initial);
  const loader = createLoader(cell);
  const apply = (event: ServerEvent) => {
    loader.record(event);
    cell.update((state) => applyEvent(state, event));
  };
  const link = createEventsLink(cell, loader, apply, deps);
  const access = { update: cell.update, apply };
  return {
    getState: cell.get,
    subscribe: cell.subscribe,
    start: link.start,
    stop: () => {
      link.stop();
      loader.abortAll();
    },
    ensure: loader.ensure,
    refetch: loader.load,
    actions: createConversationActions(access),
    ...createRepoActions(access),
    ...createPluginActions(() => loader.load(PLUGINS_KEY)),
  };
}
