import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aMarketplace, anAvailablePlugin, aPreview } from "../../api/fixtures";
import * as marketsApi from "../../api/marketplaces";
import * as pluginsApi from "../../api/plugins";
import { TestDataProvider } from "../../data/testing";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { PluginsPage } from "./PluginsPage";

function Bar() {
  const hints = useCurrentKeyHints();
  return <p data-testid="bar">{hints.map((h) => `${h.keys} ${h.label}`).join(" | ")}</p>;
}

afterEach(() => vi.restoreAllMocks());

describe("plugins key hints", () => {
  it("shows tab on the page and esc while the install prompt is open", async () => {
    vi.spyOn(marketsApi, "listMarketplaces").mockResolvedValue([aMarketplace()]);
    vi.spyOn(pluginsApi, "listPlugins").mockResolvedValue({ installed: [], available: [anAvailablePlugin()] });
    vi.spyOn(pluginsApi, "previewPlugin").mockResolvedValue(aPreview());
    render(
      <TestDataProvider>
        <KeyHintsProvider>
          <Bar />
          <PluginsPage />
        </KeyHintsProvider>
      </TestDataProvider>,
    );
    expect(screen.getByTestId("bar")).toHaveTextContent("tab next control");
    await userEvent.click(await screen.findByRole("button", { name: "install lint" }));
    await screen.findByRole("dialog");
    expect(screen.getByTestId("bar")).toHaveTextContent("esc cancel install");
  });
});
