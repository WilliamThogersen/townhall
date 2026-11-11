import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { PluginUiEntry } from "../api/types";
import { navigate } from "../router";
import { PluginNavItems } from "./PluginNavItems";

const nav = (over: Partial<PluginUiEntry> = {}): PluginUiEntry => ({
  plugin: "panel@acme",
  slot: "sidebar.nav",
  title: "Board",
  entry: "ui/board.html",
  height: null,
  ...over,
});

beforeEach(() => {
  window.history.replaceState(null, "", "/");
});

describe("PluginNavItems", () => {
  it("renders one link per sidebar.nav entry that opens the plugin page", async () => {
    const listUi = async () => [nav(), nav({ slot: "page" }), nav({ plugin: "b@acme", title: "Metrics", entry: "m.html" })];
    render(<PluginNavItems listUi={listUi} />);
    const board = await screen.findByRole("link", { name: /Board/ });
    expect(board).toHaveAttribute("href", "/p/panel%40acme/ui%2Fboard.html");
    expect(board).toHaveClass("th-nav-item");
    expect(screen.getByRole("link", { name: /Metrics/ })).toHaveAttribute("href", "/p/b%40acme/m.html");
    expect(screen.getAllByRole("link")).toHaveLength(2);
  });

  it("marks the link of the open page as current", async () => {
    window.history.replaceState(null, "", "/p/panel%40acme/ui%2Fboard.html");
    render(<PluginNavItems listUi={async () => [nav()]} />);
    const link = await screen.findByRole("link", { name: /Board/ });
    await waitFor(() => expect(link).toHaveAttribute("aria-current", "page"));
    act(() => navigate("/"));
    await waitFor(() => expect(link).not.toHaveAttribute("aria-current"));
  });

  it("renders nothing without enabled plugins", async () => {
    const { container } = render(<PluginNavItems listUi={async () => []} />);
    await act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
    expect(container).toBeEmptyDOMElement();
  });
});
