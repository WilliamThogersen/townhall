import { screen } from "@testing-library/react";
import { vi } from "vitest";
import * as client from "../../api/client";
import * as conversationsApi from "../../api/conversations";
import { aConversation, anAgentInfo, aRepo, aStep, aTask } from "../../api/fixtures";
import * as reposApi from "../../api/repos";
import * as stepsApi from "../../api/steps";
import * as tasksApi from "../../api/tasks";
import type { Conversation, Step, Task } from "../../api/types";
import { createFakeEvents, TestDataProvider, type FakeEvents } from "../../data/testing";
import { renderWithHints } from "../../test/renderWithHints";
import { ControlCenterPage } from "./ControlCenterPage";

export const at = (h: number, m = 0, day = 4) => new Date(2026, 10, day, h, m).toISOString();
export const NOW = new Date(2026, 10, 4, 11, 0);

export const CONVERSATIONS: readonly Conversation[] = [
  aConversation({ id: "c2", title: "flaky ci on main", createdAt: at(10, 10), updatedAt: at(10, 40) }),
  aConversation({ id: "c1", title: "login redirect loop", createdAt: at(10), updatedAt: at(10, 50) }),
  aConversation({ id: "c3", title: "old and done", ended: true, createdAt: at(9, 0, 2), updatedAt: at(10, 0, 2) }),
];

export const TASKS: readonly Task[] = [
  aTask({ name: "fix-login", conversationId: "c1", repo: "townhall", state: "needsYou", createdAt: at(10, 5) }),
  aTask({ name: "auth-tests", conversationId: "c1", repo: "townhall", state: "working", createdAt: at(10, 6) }),
  aTask({ name: "logout-cookie", conversationId: "c1", repo: "townhall", state: "queued", after: ["fix-login"] }),
  aTask({ name: "e2e-smoke", conversationId: "c2", repo: "shop", state: "error", createdAt: at(10, 12) }),
  aTask({ name: "coupon", conversationId: "c3", repo: "shop", state: "merged", createdAt: at(9, 5, 2) }),
];

const STEPS: Readonly<Record<string, readonly Step[]>> = {
  "auth-tests": [
    aStep({ id: "a1", tool: "Read", summary: "src/auth/session.ts", startedAt: at(10, 7), endedAt: at(10, 9), result: "ok" }),
    aStep({ id: "a2", tool: "Bash", summary: "npm test -- auth", startedAt: at(10, 50), endedAt: null, result: "running" }),
  ],
  "fix-login": [aStep({ id: "f1", tool: "Edit", summary: "src/auth/redirect.ts", startedAt: at(10, 20), endedAt: at(10, 30), result: "ok" })],
  "e2e-smoke": [
    aStep({ id: "e1", tool: "Bash", summary: "npx playwright test", startedAt: at(10, 14), endedAt: at(10, 26), result: "error" }),
  ],
};

export interface Stubs {
  readonly conversations?: readonly Conversation[] | Error | "pending";
  readonly tasks?: readonly Task[];
}

export function stubDaemon({ conversations = CONVERSATIONS, tasks = TASKS }: Stubs = {}) {
  const list = vi.spyOn(conversationsApi, "listConversations");
  if (conversations === "pending") list.mockReturnValue(new Promise(() => undefined));
  else if (conversations instanceof Error) list.mockRejectedValue(conversations);
  else list.mockResolvedValue(conversations);
  vi.spyOn(tasksApi, "listTasks").mockResolvedValue(tasks);
  vi.spyOn(client, "listAgents").mockResolvedValue([
    anAgentInfo({ name: "e2e-smoke", repo: "shop", alive: false, state: { kind: "error", exitCode: 1 } }),
  ]);
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo({ name: "townhall" }), aRepo({ name: "shop" })]);
  return vi.spyOn(stepsApi, "getSteps").mockImplementation(async (agent) => STEPS[agent] ?? []);
}

export function renderControl(path = "/townhall", events: FakeEvents = createFakeEvents()) {
  window.history.replaceState(null, "", path);
  return renderWithHints(
    <TestDataProvider events={events}>
      <ControlCenterPage />
    </TestDataProvider>,
  );
}

export const item = (name: RegExp | string) =>
  screen.getByRole("treeitem", { name: typeof name === "string" ? new RegExp(`^${name}\\b`) : name });

export const findItem = (name: string) => screen.findByRole("treeitem", { name: new RegExp(`^${name}\\b`) });

export const items = () => screen.queryAllByRole("treeitem").map((row) => row.dataset.row);

export const url = () => `${window.location.pathname}${window.location.search}`;
