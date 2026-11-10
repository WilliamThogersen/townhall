import { listAgents } from "../api/client";
import { getConversation, listConversations } from "../api/conversations";
import { listMarketplaces } from "../api/marketplaces";
import { listPlugins } from "../api/plugins";
import { listRepos } from "../api/repos";
import { listTasks } from "../api/tasks";
import type { ServerEvent } from "../api/types";
import { keepPlugins, keepRepos, newestFirst, reduceAgents, reduceConversations, reduceDetail, reduceTasks } from "./reducers";
import { failed, loaded, pending, withDetail, type DataState, type Resource } from "./state";

export const CONVERSATIONS_KEY = "conversations";
export const TASKS_KEY = "tasks";
export const AGENTS_KEY = "agents";
export const REPOS_KEY = "repos";
export const PLUGINS_KEY = "plugins";
const DETAIL_PREFIX = "detail:";

export const detailKey = (id: string): string => `${DETAIL_PREFIX}${id}`;

interface ResourceSpec<T> {
  readonly fetch: (signal: AbortSignal) => Promise<T>;
  readonly read: (state: DataState) => Resource<T> | null;
  readonly write: (state: DataState, resource: Resource<T>) => DataState;
  readonly reduce: (data: T, event: ServerEvent) => T;
}

export type Settle = (state: DataState, replay: readonly ServerEvent[]) => DataState;

export interface Loadable {
  readonly read: (state: DataState) => Resource<unknown> | null;
  readonly request: (state: DataState) => DataState;
  readonly fetch: (signal: AbortSignal) => Promise<Settle>;
  readonly fail: (state: DataState, error: string) => DataState;
}

function erase<T>(spec: ResourceSpec<T>): Loadable {
  return {
    read: spec.read,
    request: (state) => (spec.read(state) === null ? spec.write(state, pending()) : state),
    fetch: async (signal) => {
      const data = await spec.fetch(signal);
      return (state, replay) => spec.write(state, loaded(replay.reduce(spec.reduce, data)));
    },
    fail: (state, error) => spec.write(state, failed(spec.read(state), error)),
  };
}

const FIXED: ReadonlyMap<string, Loadable> = new Map([
  [
    CONVERSATIONS_KEY,
    erase({
      fetch: (signal) => listConversations(signal).then(newestFirst),
      read: (state) => state.conversations,
      write: (state, conversations) => ({ ...state, conversations }),
      reduce: reduceConversations,
    }),
  ],
  [
    TASKS_KEY,
    erase({
      fetch: (signal) => listTasks({}, signal),
      read: (state) => state.tasks,
      write: (state, tasks) => ({ ...state, tasks }),
      reduce: reduceTasks,
    }),
  ],
  [
    AGENTS_KEY,
    erase({
      fetch: (signal) => listAgents(signal),
      read: (state) => state.agents,
      write: (state, agents) => ({ ...state, agents }),
      reduce: reduceAgents,
    }),
  ],
  [
    REPOS_KEY,
    erase({
      fetch: (signal) => listRepos(signal),
      read: (state) => state.repos,
      write: (state, repos) => ({ ...state, repos }),
      reduce: keepRepos,
    }),
  ],
  [
    PLUGINS_KEY,
    erase({
      fetch: async (signal) => {
        const [marketplaces, { available, installed }] = await Promise.all([
          listMarketplaces(signal),
          listPlugins(signal),
        ]);
        return { marketplaces, available, installed };
      },
      read: (state) => state.plugins,
      write: (state, plugins) => ({ ...state, plugins }),
      reduce: keepPlugins,
    }),
  ],
]);

function detailLoadable(id: string): Loadable {
  return erase({
    fetch: (signal) => getConversation(id, signal),
    read: (state) => state.details.get(id) ?? null,
    write: (state, resource) => ({ ...state, details: withDetail(state.details, id, resource) }),
    reduce: reduceDetail,
  });
}

export function loadableFor(key: string): Loadable {
  if (key.startsWith(DETAIL_PREFIX)) return detailLoadable(key.slice(DETAIL_PREFIX.length));
  const loadable = FIXED.get(key);
  if (!loadable) throw new Error(`unknown data key ${key}`);
  return loadable;
}

export function requestedKeys(state: DataState): readonly string[] {
  const fixed = [...FIXED].filter(([, loadable]) => loadable.read(state) !== null).map(([key]) => key);
  return [...fixed, ...[...state.details.keys()].map(detailKey)];
}
