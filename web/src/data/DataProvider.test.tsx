import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as conversationsApi from "../api/conversations";
import * as eventsApi from "../api/events";
import { aConversation } from "../api/fixtures";
import { DataProvider, useConnection, useConversations } from ".";
import { createFakeEvents, TestDataProvider, type FakeEvents } from "./testing";

afterEach(() => vi.restoreAllMocks());

const wrapWith = (events: FakeEvents) =>
  function Wrapper({ children }: { readonly children: ReactNode }) {
    return <TestDataProvider events={events}>{children}</TestDataProvider>;
  };

describe("DataProvider", () => {
  it("reports the events socket status", () => {
    const events = createFakeEvents("connecting");
    const { result } = renderHook(() => useConnection(), { wrapper: wrapWith(events) });
    expect(result.current).toBe("connecting");
    act(() => events.setStatus("open"));
    expect(result.current).toBe("open");
    act(() => events.setStatus("closed"));
    expect(result.current).toBe("closed");
  });

  it("opens one socket with the real connector by default and closes it on unmount", () => {
    const close = vi.fn();
    const connect = vi.spyOn(eventsApi, "connectEvents").mockReturnValue({ close });
    const { unmount } = renderHook(() => useConnection(), {
      wrapper: ({ children }) => <DataProvider>{children}</DataProvider>,
    });
    expect(connect).toHaveBeenCalledOnce();
    unmount();
    expect(close).toHaveBeenCalledOnce();
  });

  it("refetches what is loaded on resync and on a reconnect", async () => {
    const list = vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([aConversation()]);
    const events = createFakeEvents("open");
    const { result } = renderHook(() => useConversations(), { wrapper: wrapWith(events) });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(list).toHaveBeenCalledOnce();
    act(() => events.emit({ type: "resync" }));
    await waitFor(() => expect(list).toHaveBeenCalledTimes(2));
    act(() => {
      events.setStatus("closed");
      events.setStatus("connecting");
      events.setStatus("open");
    });
    await waitFor(() => expect(list).toHaveBeenCalledTimes(3));
    expect(result.current.loading).toBe(false);
  });

  it("refetches on the first open only what settled before it", async () => {
    const list = vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([aConversation()]);
    const events = createFakeEvents("connecting");
    const { result } = renderHook(() => useConversations(), { wrapper: wrapWith(events) });
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => events.setStatus("open"));
    await waitFor(() => expect(list).toHaveBeenCalledTimes(2));
  });

  it("disconnects the fake on unmount", () => {
    const events = createFakeEvents();
    const { unmount } = renderHook(() => useConnection(), { wrapper: wrapWith(events) });
    expect(events.isConnected()).toBe(true);
    unmount();
    expect(events.isConnected()).toBe(false);
  });

  it("gives a test provider its own fake when none is passed", () => {
    const { result } = renderHook(() => useConnection(), {
      wrapper: ({ children }) => <TestDataProvider>{children}</TestDataProvider>,
    });
    expect(result.current).toBe("open");
  });

  it("throws a clear error outside the provider", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    expect(() => renderHook(() => useConnection())).toThrow("data hooks need a DataProvider");
  });
});
