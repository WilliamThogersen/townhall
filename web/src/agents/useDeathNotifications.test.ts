import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Agent } from "../api/types";
import * as notify from "../desktop/notify";
import { findDeaths, useDeathNotifications } from "./useDeathNotifications";

const mk = (name: string, alive: boolean): Agent => ({
  name, kind: null, branch: `townhall/${name}`, worktree: `/w/${name}`, alive,
});

afterEach(() => vi.restoreAllMocks());

describe("findDeaths", () => {
  it("reports only alive to dead transitions", () => {
    const prev = [mk("a", true), mk("b", true), mk("c", false), mk("d", true)];
    const next = [mk("a", false), mk("b", true), mk("c", false)];
    expect(findDeaths(prev, next)).toEqual(["a"]);
  });
});

describe("useDeathNotifications", () => {
  it("notifies on a transition but not on the first load", () => {
    const spy = vi.spyOn(notify, "notifyAgentDied").mockResolvedValue();
    const { rerender } = renderHook(
      ({ agents, loading }) => useDeathNotifications(agents, loading),
      { initialProps: { agents: [] as Agent[], loading: true } },
    );
    rerender({ agents: [mk("a", false)], loading: false });
    expect(spy).not.toHaveBeenCalled();
    rerender({ agents: [mk("a", true)], loading: false });
    rerender({ agents: [mk("a", false)], loading: false });
    expect(spy).toHaveBeenCalledExactlyOnceWith("a");
  });

  it("is a no-op in a plain browser", async () => {
    vi.restoreAllMocks();
    await expect(notify.notifyAgentDied("a")).resolves.toBeUndefined();
  });
});
