import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PluginFrame, pluginUiUrl } from "./PluginFrame";

describe("pluginUiUrl", () => {
  it("is the daemon route, encoded per segment, on the same origin in a browser", () => {
    expect(pluginUiUrl("panel@acme", "ui/panel.html", {})).toBe(
      "/api/plugins/panel%40acme/ui/ui/panel.html",
    );
  });

  it("is the fixed loopback address in the desktop shell", () => {
    expect(pluginUiUrl("panel@acme", "ui/a b.html", { __TAURI_INTERNALS__: {} })).toBe(
      "http://127.0.0.1:7777/api/plugins/panel%40acme/ui/ui/a%20b.html",
    );
  });
});

describe("PluginFrame", () => {
  it("renders a script-only sandbox without same-origin and with no powers", () => {
    const { container } = render(<PluginFrame plugin="panel@acme" entry="ui/panel.html" title="Panel" />);
    const frame = container.querySelector("iframe");
    expect(frame).not.toBeNull();
    expect(frame?.getAttribute("sandbox")).toBe("allow-scripts");
    expect(frame?.getAttribute("sandbox")?.split(" ")).not.toContain("allow-same-origin");
    expect(frame?.getAttribute("referrerpolicy")).toBe("no-referrer");
    expect(frame?.getAttribute("allow")).toBe("");
    expect(frame?.getAttribute("src")).toBe("/api/plugins/panel%40acme/ui/ui/panel.html");
    expect(frame?.getAttribute("title")).toBe("Panel");
  });

  it("keeps the sandbox when the caller passes more props", () => {
    const { container } = render(
      <PluginFrame plugin="p@m" entry="a.html" title="t" height={80} />,
    );
    const frame = container.querySelector("iframe");
    expect(frame?.getAttribute("sandbox")).toBe("allow-scripts");
    expect(frame?.style.height).toBe("80px");
  });
});
