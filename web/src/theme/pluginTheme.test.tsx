import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as pluginsApi from "../api/plugins";
import { someContributions } from "../api/fixtures";
import { TestDataProvider } from "../data/testing";
import { applyPluginTheme, PLUGIN_THEME_ATTRIBUTE, PluginTheme } from "./pluginTheme";

const styles = () => document.head.querySelectorAll(`style[${PLUGIN_THEME_ATTRIBUTE}]`);

afterEach(() => {
  applyPluginTheme(null);
  document.head.replaceChildren();
  vi.restoreAllMocks();
});

describe("applyPluginTheme", () => {
  it("adds one style element after what the head already holds and updates it in place", () => {
    const builtIn = document.createElement("style");
    builtIn.textContent = ":root { --th-bg: #111; }";
    document.head.append(builtIn);
    applyPluginTheme(":root {\n  --th-bg: #222;\n}\n");
    expect(styles()).toHaveLength(1);
    expect(document.head.lastElementChild).toBe(styles()[0]);
    const first = styles()[0];
    applyPluginTheme(":root {\n  --th-bg: #333;\n}\n");
    expect(styles()).toHaveLength(1);
    expect(styles()[0]).toBe(first);
    expect(first?.textContent).toBe(":root {\n  --th-bg: #333;\n}\n");
  });

  it("removes the style when there is no theme and again is harmless", () => {
    applyPluginTheme(":root { --th-bg: #222; }");
    applyPluginTheme(null);
    expect(styles()).toHaveLength(0);
    applyPluginTheme(null);
    expect(styles()).toHaveLength(0);
  });

  it("keeps the text as text, never as markup", () => {
    const text = "/* </style><script>window.pwned = 1</script> */";
    applyPluginTheme(text);
    expect(document.head.querySelector("script")).toBeNull();
    expect(styles()[0]?.textContent).toBe(text);
  });
});

describe("PluginTheme", () => {
  it("applies the active theme of the daemon and removes it when it goes", async () => {
    const css = ":root {\n  --th-accent: #ff00aa;\n}\n";
    vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(
      someContributions({ theme: { plugin: "theme-slate@official", css } }),
    );
    const { unmount } = render(
      <TestDataProvider>
        <PluginTheme />
      </TestDataProvider>,
    );
    await vi.waitFor(() => expect(styles()[0]?.textContent).toBe(css));
    unmount();
    expect(styles()).toHaveLength(0);
  });

  it("adds nothing when no plugin chose a theme", async () => {
    const load = vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(someContributions());
    render(
      <TestDataProvider>
        <PluginTheme />
      </TestDataProvider>,
    );
    await vi.waitFor(() => expect(load).toHaveBeenCalled());
    expect(styles()).toHaveLength(0);
  });

  it("adds nothing when the daemon cannot be asked", async () => {
    const load = vi.spyOn(pluginsApi, "getContributions").mockRejectedValue(new Error("down"));
    render(
      <TestDataProvider>
        <PluginTheme />
      </TestDataProvider>,
    );
    await vi.waitFor(() => expect(load).toHaveBeenCalled());
    expect(styles()).toHaveLength(0);
  });
});
