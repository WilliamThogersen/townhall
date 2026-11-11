import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as pluginsApi from "../../api/plugins";
import type { PluginUiEntry } from "../../api/types";
import { forgetHostSession } from "../../plugins/hostSession";
import { ToastProvider } from "../../ui/Toasts";
import { PluginSettingsFrames } from "./PluginSettingsFrames";

const entry = (over: Partial<PluginUiEntry> = {}): PluginUiEntry => ({
  plugin: "notes@acme",
  slot: "settings.group",
  title: "release notes",
  entry: "ui/settings.html",
  height: 120,
  ...over,
});

beforeEach(() => {
  forgetHostSession("notes@acme");
  vi.spyOn(pluginsApi, "mintHostToken").mockResolvedValue({
    plugin: "notes@acme",
    version: "1.0.0",
    token: "ab".repeat(32),
    scopes: [],
  });
});
afterEach(() => vi.restoreAllMocks());

const renderFrames = () =>
  render(
    <ToastProvider>
      <PluginSettingsFrames />
    </ToastProvider>,
  );

describe("PluginSettingsFrames", () => {
  it("renders a settings.group entry as a titled group holding the frame", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([entry(), entry({ slot: "page", title: "Board" })]);
    const { container } = renderFrames();
    expect(await screen.findByRole("heading", { level: 2, name: "release notes (plugin)" })).toBeInTheDocument();
    const frames = container.querySelectorAll("section.th-setting-group iframe");
    expect(frames).toHaveLength(1);
    expect((frames[0] as HTMLIFrameElement).style.height).toBe("120px");
  });

  it("answers hello with the settings.group slot", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([entry()]);
    const { container } = renderFrames();
    await waitFor(() => expect(container.querySelector("iframe")).not.toBeNull());
    const frame = container.querySelector("iframe") as HTMLIFrameElement;
    const spy = vi.spyOn(frame.contentWindow as Window, "postMessage");
    act(() => {
      window.dispatchEvent(
        new MessageEvent("message", { data: { th: 1, type: "hello", apiVersion: 1 }, source: frame.contentWindow }),
      );
    });
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0]?.[0]).toMatchObject({ type: "init", slot: "settings.group", context: {} });
  });

  it("renders nothing when the plugin is disabled", async () => {
    const list = vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([]);
    const { container } = renderFrames();
    await waitFor(() => expect(list).toHaveBeenCalled());
    expect(container.querySelector("iframe")).toBeNull();
    expect(screen.queryByRole("heading")).toBeNull();
  });
});
