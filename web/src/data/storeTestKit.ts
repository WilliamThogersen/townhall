import { vi } from "vitest";
import * as client from "../api/client";
import * as conversationsApi from "../api/conversations";
import type { ConnectionStatus } from "../api/events";
import * as reposApi from "../api/repos";
import * as tasksApi from "../api/tasks";
import { createDataStore } from "./store";
import { createFakeEvents } from "./testing";

export function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  let reject: (reason: unknown) => void = () => undefined;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

export const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

export function spyApi() {
  return {
    listConversations: vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([]),
    getConversation: vi.spyOn(conversationsApi, "getConversation"),
    listTasks: vi.spyOn(tasksApi, "listTasks").mockResolvedValue([]),
    listAgents: vi.spyOn(client, "listAgents").mockResolvedValue([]),
    listRepos: vi.spyOn(reposApi, "listRepos").mockResolvedValue([]),
  };
}

export function startStore(initial: ConnectionStatus = "open", agentsPollMs = 60_000) {
  const events = createFakeEvents(initial);
  const store = createDataStore({ connect: events.connect, agentsPollMs });
  store.start();
  return { events, store };
}
