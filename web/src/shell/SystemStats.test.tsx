import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import { aSystem } from "../api/fixtures";
import * as systemApi from "../api/system";
import { formatSystem, SystemStats } from "./SystemStats";

const GB = 1024 ** 3;

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
});

async function flush() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

describe("formatSystem", () => {
  it("writes cpu, ram and disk compactly in lowercase", () => {
    expect(formatSystem(aSystem())).toEqual({
      cpu: "cpu 23%",
      ram: "ram 11.2/32 GB",
      disk: "disk 410/994 GB",
    });
  });

  it("drops a trailing .0 and keeps one decimal below 100 GB", () => {
    const s = formatSystem(aSystem({ memoryUsedBytes: 8 * GB, memoryTotalBytes: 16 * GB }));
    expect(s.ram).toBe("ram 8/16 GB");
  });
});

describe("SystemStats", () => {
  it("shows the stats once they are read", async () => {
    vi.spyOn(systemApi, "getSystem").mockResolvedValue(aSystem());
    const { container } = render(<SystemStats />);
    expect(container).toBeEmptyDOMElement();
    await flush();
    expect(screen.getByText("cpu 23%")).toBeInTheDocument();
    expect(screen.getByText("ram 11.2/32 GB")).toBeInTheDocument();
    expect(screen.getByText("disk 410/994 GB")).toBeInTheDocument();
  });

  it("polls every few seconds", async () => {
    const get = vi.spyOn(systemApi, "getSystem").mockResolvedValue(aSystem());
    render(<SystemStats />);
    await flush();
    expect(get).toHaveBeenCalledTimes(1);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });
    expect(get).toHaveBeenCalledTimes(2);
  });

  it("pauses while the tab is hidden and reads again when it shows", async () => {
    const get = vi.spyOn(systemApi, "getSystem").mockResolvedValue(aSystem());
    render(<SystemStats />);
    await flush();
    Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      await vi.advanceTimersByTimeAsync(20_000);
    });
    expect(get).toHaveBeenCalledTimes(1);
    Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(get).toHaveBeenCalledTimes(2);
  });

  it("shows nothing when the daemon cannot be asked", async () => {
    vi.spyOn(systemApi, "getSystem").mockRejectedValue(new ApiError("network", "cannot reach the daemon: down"));
    const { container } = render(<SystemStats />);
    await flush();
    expect(container).toBeEmptyDOMElement();
  });

  it("hides stats again when a later read fails", async () => {
    const get = vi.spyOn(systemApi, "getSystem").mockResolvedValueOnce(aSystem());
    const { container } = render(<SystemStats />);
    await flush();
    expect(screen.getByText("cpu 23%")).toBeInTheDocument();
    get.mockRejectedValue(new ApiError("network", "down"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });
    expect(container).toBeEmptyDOMElement();
  });

  it("tints a value amber above 85% and red above 95%", async () => {
    vi.spyOn(systemApi, "getSystem").mockResolvedValue(
      aSystem({ cpuPercent: 90, memoryUsedBytes: 31 * GB, memoryTotalBytes: 32 * GB, diskUsedBytes: 50 * GB }),
    );
    render(<SystemStats />);
    await flush();
    expect(screen.getByText("cpu 90%")).toHaveAttribute("data-level", "warn");
    expect(screen.getByText("ram 31/32 GB")).toHaveAttribute("data-level", "hot");
    expect(screen.getByText("disk 50/994 GB")).not.toHaveAttribute("data-level");
  });
});
