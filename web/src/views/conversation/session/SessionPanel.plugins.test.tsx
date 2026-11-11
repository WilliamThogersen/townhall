import { act, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as artifactsApi from "../../../api/artifacts";
import { aConversation, aDetail, aTask } from "../../../api/fixtures";
import * as pluginsApi from "../../../api/plugins";
import type { PluginUiEntry } from "../../../api/types";
import { forgetHostSession } from "../../../plugins/hostSession";
import { KeyHintsProvider } from "../../../shell/keyHints";
import { ToastProvider } from "../../../ui/Toasts";
import { SessionPanel } from "./index";

const section = (slot: string, over: Partial<PluginUiEntry> = {}): PluginUiEntry => ({
  plugin: "panel@acme",
  slot,
  title: slot === "task.section" ? "Task notes" : "Board",
  entry: `ui/${slot}.html`,
  height: 90,
  ...over,
});

const detail = aDetail({
  conversation: aConversation({ id: "login-loop" }),
  tasks: [aTask({ name: "auth-tests", state: "working" }), aTask({ name: "fix-login", state: "needsYou" })],
});

beforeEach(() => {
  forgetHostSession("panel@acme");
  vi.spyOn(artifactsApi, "listArtifacts").mockResolvedValue([]);
  vi.spyOn(pluginsApi, "mintHostToken").mockResolvedValue({
    plugin: "panel@acme",
    version: "1.0.0",
    token: "cd".repeat(32),
    scopes: [],
  });
});
afterEach(() => vi.restoreAllMocks());

function renderPanel(openTask: string | null = null) {
  return render(
    <ToastProvider>
      <KeyHintsProvider>
        <SessionPanel detail={detail} openTask={openTask} onOpenTask={vi.fn()} />
      </KeyHintsProvider>
    </ToastProvider>,
  );
}

const panel = () => screen.getByRole("complementary", { name: "Session" });
const frames = () => [...panel().querySelectorAll("iframe")];

async function initOf(frame: HTMLIFrameElement) {
  const spy = vi.spyOn(frame.contentWindow as Window, "postMessage");
  act(() => {
    window.dispatchEvent(
      new MessageEvent("message", { data: { th: 1, type: "hello", apiVersion: 1 }, source: frame.contentWindow }),
    );
  });
  await waitFor(() => expect(spy).toHaveBeenCalled());
  return spy.mock.calls[0]?.[0] as { type: string; slot: string; context: unknown };
}

describe("SessionPanel plugin slots", () => {
  it("renders a session.section entry as the last section with the conversation id", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([section("session.section")]);
    renderPanel();
    await waitFor(() => expect(frames()).toHaveLength(1));
    const heading = within(panel()).getByRole("heading", { name: "Board" });
    expect(heading).toHaveClass("th-section");
    const context = within(panel()).getByRole("heading", { name: "context" });
    expect(context.compareDocumentPosition(heading) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(heading.nextElementSibling).toBe(frames()[0]);
    expect(frames()[0]?.style.height).toBe("90px");
    expect(await initOf(frames()[0] as HTMLIFrameElement)).toMatchObject({
      type: "init",
      slot: "session.section",
      context: { conversationId: "login-loop" },
    });
  });

  it("renders a task.section entry for the open task, with the task name", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([section("task.section")]);
    renderPanel("auth-tests");
    await waitFor(() => expect(frames()).toHaveLength(1));
    expect(within(panel()).getByRole("heading", { name: "Task notes" })).toBeInTheDocument();
    expect(await initOf(frames()[0] as HTMLIFrameElement)).toMatchObject({
      slot: "task.section",
      context: { conversationId: "login-loop", task: "auth-tests" },
    });
  });

  it("shows no task.section while no task is open", async () => {
    const list = vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([section("task.section")]);
    renderPanel(null);
    await waitFor(() => expect(list).toHaveBeenCalled());
    await act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
    expect(frames()).toHaveLength(0);
  });

  it("renders nothing when the plugin is disabled", async () => {
    const list = vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([]);
    renderPanel("auth-tests");
    await waitFor(() => expect(list).toHaveBeenCalled());
    expect(frames()).toHaveLength(0);
    expect(within(panel()).queryByRole("heading", { name: "Board" })).toBeNull();
  });
});
