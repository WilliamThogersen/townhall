import type { ConnectionStatus } from "../api/events";
import type {
  AgentInfo,
  AvailablePlugin,
  Conversation,
  ConversationDetail,
  Marketplace,
  Plugin,
  PluginContributions,
  Repo,
  Task,
} from "../api/types";

export interface PluginsData {
  readonly marketplaces: readonly Marketplace[];
  readonly available: readonly AvailablePlugin[];
  readonly installed: readonly Plugin[];
}

export interface Resource<T> {
  readonly data: T | null;
  readonly settled: boolean;
  readonly error: string | null;
}

export interface DataState {
  readonly connection: ConnectionStatus;
  readonly generation: number;
  readonly conversations: Resource<readonly Conversation[]> | null;
  readonly details: ReadonlyMap<string, Resource<ConversationDetail>>;
  readonly tasks: Resource<readonly Task[]> | null;
  readonly agents: Resource<readonly AgentInfo[]> | null;
  readonly repos: Resource<readonly Repo[]> | null;
  readonly plugins: Resource<PluginsData> | null;
  readonly contributions: Resource<PluginContributions> | null;
}

export const INITIAL_STATE: DataState = {
  connection: "connecting",
  generation: 0,
  conversations: null,
  details: new Map(),
  tasks: null,
  agents: null,
  repos: null,
  plugins: null,
  contributions: null,
};

export function pending<T>(): Resource<T> {
  return { data: null, settled: false, error: null };
}

export function loaded<T>(data: T): Resource<T> {
  return { data, settled: true, error: null };
}

export function failed<T>(previous: Resource<T> | null, error: string): Resource<T> {
  return { data: previous?.data ?? null, settled: true, error };
}

function mapLoaded<T>(resource: Resource<T>, change: (data: T) => T): Resource<T> {
  if (resource.data === null) return resource;
  const data = change(resource.data);
  return data === resource.data ? resource : { ...resource, data };
}

export function mapResource<T>(resource: Resource<T> | null, change: (data: T) => T): Resource<T> | null {
  return resource === null ? null : mapLoaded(resource, change);
}

export function mapDetails(
  details: DataState["details"],
  change: (detail: ConversationDetail) => ConversationDetail,
): DataState["details"] {
  const entries = [...details].map(([id, resource]) => [id, mapLoaded(resource, change)] as const);
  const same = entries.every(([id, resource]) => resource === details.get(id));
  return same ? details : new Map(entries);
}

export function withDetail(
  details: DataState["details"],
  id: string,
  resource: Resource<ConversationDetail>,
): DataState["details"] {
  return new Map([...details, [id, resource]]);
}
