import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as pluginsApi from "../api/plugins";
import type { PluginUiEntry } from "../api/types";
import { forgetHostSession } from "../plugins/hostSession";
import { ToastProvider } from "../ui/Toasts";
import { Composer } from "./Composer";

const action: PluginUiEntry = {
  plugin: "snippets@acme",
  slot: "composer.action",
  title: "Snippets",
  entry: "ui/snippets.html",
  height: 160,
};

beforeEach(() => {
  forgetHostSession("snippets@acme");
  vi.spyOn(pluginsApi, "mintHostToken").mockResolvedValue({
    plugin: "snippets@acme",
    version: "1.0.0",
    token: "12".repeat(32),
    scopes: [],
  });
});
afterEach(() => vi.restoreAllMocks());

function renderComposer(onSubmit = vi.fn()) {
  return render(
    <ToastProvider>
      <Composer conversationId="login-loop" onSubmit={onSubmit} />
    </ToastProvider>,
  );
}

const frame = () => document.querySelector("iframe");

async function say(target: HTMLIFrameElement, data: unknown) {
  act(() => {
    window.dispatchEvent(new MessageEvent("message", { data, source: target.contentWindow }));
  });
  await act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
}

async function openAction() {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: "Snippets" }));
  await waitFor(() => expect(frame()).not.toBeNull());
  return { user, target: frame() as HTMLIFrameElement };
}

describe("Composer with a plugin composer.action", () => {
  it("shows a button with the manifest title in the tools row, before the help text", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([action]);
    renderComposer();
    const button = await screen.findByRole("button", { name: "Snippets" });
    expect(button.closest(".th-composer-tools")).not.toBeNull();
    const help = document.querySelector(".th-composer-tools .th-help") as HTMLElement;
    expect(button.compareDocumentPosition(help) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(button).toHaveAttribute("type", "button");
    expect(frame()).toBeNull();
  });

  it("opens the plugin's popover frame on click and closes it on a second click", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([action]);
    renderComposer();
    const { user } = await openAction();
    expect(frame()?.style.height).toBe("160px");
    await user.click(screen.getByRole("button", { name: "Snippets" }));
    expect(frame()).toBeNull();
  });

  it("hands the frame the composer.action slot and the conversation id", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([action]);
    renderComposer();
    const { target } = await openAction();
    const spy = vi.spyOn(target.contentWindow as Window, "postMessage");
    await say(target, { th: 1, type: "hello", apiVersion: 1 });
    expect(spy.mock.calls.map((c) => c[0])).toMatchObject([
      { type: "init", slot: "composer.action", context: { conversationId: "login-loop" } },
      { type: "composer.action" },
    ]);
  });

  it("puts composer.insert text in the draft without sending it", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([action]);
    const onSubmit = vi.fn();
    renderComposer(onSubmit);
    const { user, target } = await openAction();
    await user.type(screen.getByRole("textbox"), "look at ");
    await say(target, { th: 1, type: "hello", apiVersion: 1 });
    await say(target, { th: 1, id: 1, type: "composer.insert", text: "src/main.rs" });
    expect(screen.getByRole("textbox")).toHaveValue("look at src/main.rs");
    await say(target, { th: 1, id: 2, type: "composer.insert", text: " now" });
    expect(screen.getByRole("textbox")).toHaveValue("look at src/main.rs now");
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("shows no button when the plugin is disabled", async () => {
    const list = vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([]);
    renderComposer();
    await waitFor(() => expect(list).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: "Snippets" })).toBeNull();
  });
});
