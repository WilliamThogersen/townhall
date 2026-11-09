import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import { aRepo } from "../api/fixtures";
import * as reposApi from "../api/repos";
import { useRepos } from "../data";
import { TestDataProvider } from "../data/testing";
import * as bridge from "../desktop/bridge";
import { useAddRepo } from "./useAddRepo";

vi.mock("../desktop/bridge", () => ({ pickFolder: vi.fn() }));

const pickFolder = vi.mocked(bridge.pickFolder);
const wrapper = ({ children }: { readonly children: ReactNode }) => <TestDataProvider>{children}</TestDataProvider>;

function renderFlow() {
  return renderHook(() => ({ flow: useAddRepo(), repos: useRepos().repos }), { wrapper });
}

beforeEach(() => {
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([]);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  pickFolder.mockReset();
});

describe("useAddRepo in the browser", () => {
  it("opens the form and closes it once the repo is added", async () => {
    const added = aRepo({ name: "shop" });
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
    const { result } = renderFlow();
    await waitFor(() => expect(result.current.repos).toEqual([]));
    expect(result.current.flow.desktop).toBe(false);
    act(() => result.current.flow.begin());
    expect(result.current.flow.formOpen).toBe(true);
    await act(() => result.current.flow.submit({ path: "/code/shop" }));
    expect(add).toHaveBeenCalledWith({ path: "/code/shop" });
    expect(result.current.flow.formOpen).toBe(false);
    expect(result.current.repos).toEqual([added]);
    expect(pickFolder).not.toHaveBeenCalled();
  });

  it("keeps the form open with the daemon's message on a failure", async () => {
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(new ApiError("http", "/tmp is not the top folder of a git repo", 400));
    const { result } = renderFlow();
    act(() => result.current.flow.begin());
    await act(() => result.current.flow.submit({ path: "/tmp" }));
    expect(result.current.flow.formOpen).toBe(true);
    expect(result.current.flow.busy).toBe(false);
    expect(result.current.flow.error).toBe("/tmp is not the top folder of a git repo");
    act(() => result.current.flow.dismissError());
    expect(result.current.flow.error).toBeNull();
  });

  it("forgets the error when the form is cancelled", async () => {
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(new ApiError("http", "path is required", 400));
    const { result } = renderFlow();
    act(() => result.current.flow.begin());
    await act(() => result.current.flow.submit({ path: " " }));
    act(() => result.current.flow.cancel());
    expect(result.current.flow).toMatchObject({ formOpen: false, error: null });
  });

  it("asks the form to focus each time add is chosen", () => {
    const { result } = renderFlow();
    act(() => result.current.flow.begin());
    const first = result.current.flow.focusToken;
    act(() => result.current.flow.begin());
    expect(result.current.flow.focusToken).toBe(first + 1);
  });
});

describe("useAddRepo in the desktop app", () => {
  beforeEach(() => vi.stubGlobal("__TAURI_INTERNALS__", {}));

  it("opens the form instead of the native dialog", async () => {
    const { result } = renderFlow();
    await waitFor(() => expect(result.current.repos).toEqual([]));
    expect(result.current.flow.desktop).toBe(true);
    act(() => result.current.flow.begin());
    expect(result.current.flow.formOpen).toBe(true);
    expect(pickFolder).not.toHaveBeenCalled();
  });

  it("browses for a folder and hands the path back without adding it", async () => {
    const add = vi.spyOn(reposApi, "addRepo");
    pickFolder.mockResolvedValue("/Users/me/code/shop");
    const { result } = renderFlow();
    act(() => result.current.flow.begin());
    let path: string | null = null;
    await act(async () => {
      path = await result.current.flow.browse();
    });
    expect(path).toBe("/Users/me/code/shop");
    expect(add).not.toHaveBeenCalled();
    expect(result.current.flow).toMatchObject({ formOpen: true, error: null });
  });

  it("returns null when the dialog is cancelled", async () => {
    pickFolder.mockResolvedValue(null);
    const { result } = renderFlow();
    act(() => result.current.flow.begin());
    let path: string | null = "x";
    await act(async () => {
      path = await result.current.flow.browse();
    });
    expect(path).toBeNull();
    expect(result.current.flow.error).toBeNull();
  });

  it("reports a dialog that fails and keeps the form open", async () => {
    pickFolder.mockRejectedValue(new Error("dialog unavailable"));
    const { result } = renderFlow();
    act(() => result.current.flow.begin());
    await act(async () => {
      await result.current.flow.browse();
    });
    expect(result.current.flow).toMatchObject({ formOpen: true, error: "dialog unavailable" });
  });

  it("adds the submitted folder and closes the form", async () => {
    const added = aRepo({ name: "shop", path: "/Users/me/code/shop" });
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
    const { result } = renderFlow();
    act(() => result.current.flow.begin());
    await act(() => result.current.flow.submit({ path: "/Users/me/code/shop" }));
    expect(add).toHaveBeenCalledWith({ path: "/Users/me/code/shop" });
    expect(result.current.flow).toMatchObject({ formOpen: false, busy: false, error: null });
  });

  it("reports a refused folder in the open form", async () => {
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(new ApiError("http", "path /Users/me is registered already", 409));
    const { result } = renderFlow();
    act(() => result.current.flow.begin());
    await act(() => result.current.flow.submit({ path: "/Users/me" }));
    expect(result.current.flow).toMatchObject({ formOpen: true, busy: false, error: "path /Users/me is registered already" });
  });
});
