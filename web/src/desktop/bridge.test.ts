import { afterEach, describe, expect, it, vi } from "vitest";
import { installExternalLinks, openExternal, pickFolder } from "./bridge";

const invoke = vi.hoisted(() => vi.fn());

const openUrl = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl }));

afterEach(() => {
  vi.unstubAllGlobals();
  invoke.mockReset();
  openUrl.mockReset();
});

describe("pickFolder", () => {
  it("refuses outside the desktop app", async () => {
    await expect(pickFolder()).rejects.toThrow("desktop commands are only available in the app");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("asks the app for a folder", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    invoke.mockResolvedValue("/Users/me/code/shop");
    await expect(pickFolder()).resolves.toBe("/Users/me/code/shop");
    expect(invoke).toHaveBeenCalledWith("pick_folder");
  });
});

describe("openExternal", () => {
  it("refuses outside the desktop app", async () => {
    await expect(openExternal("https://example.com")).rejects.toThrow("desktop commands are only available in the app");
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("opens http(s) and mailto urls in the system browser", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    await openExternal("https://github.com/x/y/pull/15");
    await openExternal("mailto:a@b.dk");
    expect(openUrl).toHaveBeenCalledWith("https://github.com/x/y/pull/15");
    expect(openUrl).toHaveBeenCalledWith("mailto:a@b.dk");
  });

  it("rejects other schemes", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    await expect(openExternal("file:///etc/passwd")).rejects.toThrow("refusing to open");
    await expect(openExternal("javascript:alert(1)")).rejects.toThrow("refusing to open");
    expect(openUrl).not.toHaveBeenCalled();
  });
});

describe("installExternalLinks", () => {
  const click = (a: HTMLAnchorElement) => {
    const ev = new MouseEvent("click", { bubbles: true, cancelable: true });
    a.dispatchEvent(ev);
    return ev;
  };
  const anchor = (href: string, target = "_blank") => {
    const a = document.createElement("a");
    a.href = href;
    a.target = target;
    const span = document.createElement("span");
    a.append(span);
    document.body.append(a);
    return a;
  };
  let dispose = () => {};
  afterEach(() => {
    dispose();
    document.body.replaceChildren();
  });

  it("routes external anchor clicks through the opener in the desktop app", () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    dispose = installExternalLinks(document);
    const ev = click(anchor("https://github.com/x"));
    expect(ev.defaultPrevented).toBe(true);
    return vi.waitFor(() => expect(openUrl).toHaveBeenCalledWith("https://github.com/x"));
  });

  it("handles clicks on children of the anchor", () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    dispose = installExternalLinks(document);
    const a = anchor("https://example.com/");
    const ev = new MouseEvent("click", { bubbles: true, cancelable: true });
    a.querySelector("span")?.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
  });

  it("leaves clicks alone in a plain browser", () => {
    dispose = installExternalLinks(document);
    expect(click(anchor("https://github.com/x")).defaultPrevented).toBe(false);
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("leaves same-app links alone", () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    dispose = installExternalLinks(document);
    expect(click(anchor("#section", "")).defaultPrevented).toBe(false);
    expect(openUrl).not.toHaveBeenCalled();
  });
});
