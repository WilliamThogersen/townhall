import { render, screen, within } from "@testing-library/react";
import { vi } from "vitest";
import { AgentsProvider } from "../agents/AgentsProvider";
import * as client from "../api/client";
import * as conversationsApi from "../api/conversations";
import { aConversation, aRepo, aTask } from "../api/fixtures";
import * as reposApi from "../api/repos";
import * as tasksApi from "../api/tasks";
import type { Conversation, Task } from "../api/types";
import { createFakeEvents, TestDataProvider, type FakeEvents } from "../data/testing";
import { KeyHintsProvider, useCurrentKeyHints } from "../shell/keyHints";
import { Sidebar } from "../shell/Sidebar";

export const minutesAgo = (m: number) => new Date(Date.now() - m * 60_000).toISOString();

export const CONVERSATIONS: readonly Conversation[] = [
  aConversation({ id: "vite", title: "upgrade to vite 8", status: "thinking", updatedAt: minutesAgo(2) }),
  aConversation({ id: "ci", title: "flaky ci on main", updatedAt: minutesAgo(9) }),
  aConversation({ id: "login", title: "login redirect loop", updatedAt: minutesAgo(30) }),
  aConversation({ id: "i18n", title: "i18n keys cleanup", linkedRepo: "acme-access", updatedAt: minutesAgo(60) }),
  aConversation({ id: "coupon", title: "checkout coupon rounding", ended: true, updatedAt: minutesAgo(2 * 24 * 60) }),
];

export const TASKS: readonly Task[] = [
  aTask({ name: "fix-login", conversationId: "login", repo: "townhall", state: "needsYou" }),
  aTask({ name: "e2e-smoke", conversationId: "ci", repo: "shop", state: "error" }),
  aTask({ name: "ci-cache", conversationId: "ci", repo: "shop", state: "inCi" }),
  aTask({ name: "i18n-keys", conversationId: "i18n", repo: "acme-access", state: "working" }),
  aTask({ name: "coupon-rounding", conversationId: "coupon", repo: "shop", state: "merged" }),
];

export const REPOS = [
  aRepo({ name: "shop", baseBranch: "develop" }),
  aRepo({ name: "townhall", baseBranch: "main" }),
  aRepo({ name: "acme-access", baseBranch: "staging" }),
];

export function stubDaemon(conversations = CONVERSATIONS, tasks = TASKS) {
  vi.spyOn(client, "listAgents").mockResolvedValue([]);
  vi.spyOn(reposApi, "listRepos").mockResolvedValue(REPOS);
  return {
    list: vi.spyOn(conversationsApi, "listConversations").mockResolvedValue(conversations),
    tasks: vi.spyOn(tasksApi, "listTasks").mockResolvedValue(tasks),
  };
}

function Hints() {
  return <output aria-label="key hints">{useCurrentKeyHints().map((h) => h.keys).join(" ")}</output>;
}

export function renderSidebar(events: FakeEvents = createFakeEvents()) {
  const view = render(
    <TestDataProvider events={events}>
      <AgentsProvider intervalMs={60_000}>
        <KeyHintsProvider>
          <Sidebar />
          <Hints />
        </KeyHintsProvider>
      </AgentsProvider>
    </TestDataProvider>,
  );
  return { ...view, events };
}

export const scrollArea = () => document.querySelector<HTMLElement>(".th-sidebar-scroll") as HTMLElement;

export const rowButton = (title: string) => within(scrollArea()).getByRole("button", { name: new RegExp(`^${title}`) });

export const rowTitles = () =>
  [...scrollArea().querySelectorAll(".th-convo-title")].map((title) => title.textContent);

export const shownKeyHints = () => screen.getByRole("status", { name: "key hints" }).textContent;
