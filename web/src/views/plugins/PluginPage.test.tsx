import { act, render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { PluginUiEntry } from "../../api/types";
import type { HostSession } from "../../plugins/bridge";
import type { SlotServices } from "../../plugins/PluginSlot";
import { ToastProvider } from "../../ui/Toasts";
import { PluginPage } from "./PluginPage";

const page = (over: Partial<PluginUiEntry> = {}): PluginUiEntry => ({
  plugin: "panel@acme",
  slot: "page",
  title: "Board",
  entry: "ui/board.html",
  height: null,
  ...over,
});

function services(entries: readonly PluginUiEntry[]): Partial<SlotServices> {
  return {
    listUi: async () => entries,
    session: async (id): Promise<HostSession> => ({ plugin: id, version: "1.0.0", token: "ab".repeat(32), scopes: [] }),
  };
}

function mount(entries: readonly PluginUiEntry[], plugin = "panel@acme", name = "ui/board.html") {
  return render(
    <ToastProvider>
      <PluginPage plugin={plugin} page={name} services={services(entries)} />
    </ToastProvider>,
  );
}

describe("PluginPage", () => {
  it("renders the page entry of the plugin as a full-size frame", async () => {
    const { container } = mount([page({ height: 200 })]);
    await waitFor(() => expect(container.querySelector("iframe")).not.toBeNull());
    const frame = container.querySelector("iframe") as HTMLIFrameElement;
    expect(frame.getAttribute("src")).toContain("/plugins/panel%40acme/ui/ui/board.html");
    expect(frame).toHaveClass("th-plugin-frame--page");
    expect(frame.style.height).toBe("");
  });

  it("gives the frame its page as context", async () => {
    const { container } = mount([page()]);
    await waitFor(() => expect(container.querySelector("iframe")).not.toBeNull());
    const frame = container.querySelector("iframe") as HTMLIFrameElement;
    const spy = vi.spyOn(frame.contentWindow as Window, "postMessage");
    act(() => {
      window.dispatchEvent(
        new MessageEvent("message", { data: { th: 1, type: "hello", apiVersion: 1 }, source: frame.contentWindow }),
      );
    });
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const init = spy.mock.calls[0]?.[0] as { type: string; slot: string; context: unknown };
    expect(init).toMatchObject({ type: "init", slot: "page", context: { page: "ui/board.html" } });
  });

  it("renders nothing for a page the plugin does not declare", async () => {
    const { container } = mount([page({ entry: "ui/other.html" })]);
    await act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
    expect(container.querySelector("iframe")).toBeNull();
  });

  it("renders nothing for another plugin's page or a different slot", async () => {
    const { container } = mount([page({ plugin: "other@acme" }), page({ slot: "session.section" })]);
    await act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
    expect(container.querySelector("iframe")).toBeNull();
  });

  it("renders nothing once the plugin is disabled", async () => {
    const { container } = mount([]);
    await act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
    expect(container.querySelector("iframe")).toBeNull();
  });
});
