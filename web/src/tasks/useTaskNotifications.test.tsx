import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";
import { aTask } from "../api/fixtures";
import type { Task } from "../api/types";
import * as notify from "../desktop/notify";
import { DEFAULT_SETTINGS, type Settings } from "../settings/settings";
import { createSettingsStore } from "../settings/settingsStore";
import { SettingsStoreContext } from "../settings/useSettings";
import { BATCH_MS, OPEN_WINDOW_MS, useTaskNotifications } from "./useTaskNotifications";

function wrapperFor(settings: Partial<Settings>) {
  const store = createSettingsStore({
    load: async () => ({ ...DEFAULT_SETTINGS, ...settings }),
    put: async () => ({ ...DEFAULT_SETTINGS, ...settings }),
    storage: () => localStorage,
  });
  return function Wrapper({ children }: { readonly children: ReactNode }) {
    return <SettingsStoreContext value={store}>{children}</SettingsStoreContext>;
  };
}

async function mount(settings: Partial<Settings> = {}) {
  const hook = renderHook(
    ({ tasks }: { tasks: readonly Task[] }) => useTaskNotifications(tasks, false),
    { wrapper: wrapperFor(settings), initialProps: { tasks: [] as readonly Task[] } },
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  return hook;
}

let send: MockInstance<typeof notify.notifyDesktop>;

beforeEach(() => {
  vi.useFakeTimers();
  localStorage.clear();
  send = vi.spyOn(notify, "notifyDesktop").mockResolvedValue(true);
  window.history.replaceState(null, "", "/");
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const working = (name: string, conversationId = "c1") => aTask({ name, state: "working", conversationId });
const merged = (name: string, conversationId = "c1") => aTask({ name, title: name, state: "merged", conversationId });

describe("useTaskNotifications", () => {
  it("stays quiet on the first load", async () => {
    const hook = await mount();
    hook.rerender({ tasks: [merged("a")] });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BATCH_MS);
    });
    expect(send).not.toHaveBeenCalled();
  });

  it("sends one notification for events that arrive within the batch window", async () => {
    const hook = await mount();
    hook.rerender({ tasks: [working("a"), working("b")] });
    hook.rerender({ tasks: [merged("a"), working("b")] });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BATCH_MS - 1000);
    });
    hook.rerender({ tasks: [merged("a"), aTask({ name: "b", state: "needsYou", title: "b" })] });
    expect(send).not.toHaveBeenCalled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BATCH_MS);
    });
    expect(send).toHaveBeenCalledExactlyOnceWith({
      title: "townhall: 1 landed, 1 needs you",
      body: "open the conversation.",
      conversationId: "c1",
    });
  });

  it("sends nothing when notifications are off", async () => {
    const hook = await mount({ notifyWhen: "off" });
    hook.rerender({ tasks: [working("a")] });
    hook.rerender({ tasks: [merged("a")] });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BATCH_MS * 2);
    });
    expect(send).not.toHaveBeenCalled();
  });

  it("leaves out the kinds the setting does not cover", async () => {
    const hook = await mount({ notifyWhen: "needsYou" });
    hook.rerender({ tasks: [working("a")] });
    hook.rerender({ tasks: [merged("a")] });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BATCH_MS * 2);
    });
    expect(send).not.toHaveBeenCalled();
  });

  it("opens the conversation when the window gets focus soon after a notification", async () => {
    const hook = await mount();
    hook.rerender({ tasks: [working("a", "c9")] });
    hook.rerender({ tasks: [merged("a", "c9")] });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BATCH_MS);
    });
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(window.location.pathname).toBe("/c/c9");
  });

  it("does not open it when focus comes late", async () => {
    const hook = await mount();
    hook.rerender({ tasks: [working("a", "c9")] });
    hook.rerender({ tasks: [merged("a", "c9")] });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BATCH_MS + OPEN_WINDOW_MS + 1);
    });
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(window.location.pathname).toBe("/");
  });
});
