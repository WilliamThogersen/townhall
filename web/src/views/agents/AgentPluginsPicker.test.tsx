import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { anAgentType, aPlugin } from "../../api/fixtures";
import * as libraryApi from "../../api/library";
import * as pluginsApi from "../../api/plugins";
import type { AgentType, Plugin } from "../../api/types";
import { AgentPluginsPicker } from "./AgentPluginsPicker";

const WEB = aPlugin({
  id: "modern-web-guidance@claude-plugins-official",
  marketplace: "claude-plugins-official",
  name: "modern-web-guidance",
  enabled: true,
  permissions: ["claude.skill:chrome-extensions", "claude.skill:modern-web-guidance"],
});
const LINT = aPlugin({ permissions: ["prompt:brief", "claude.command:fix"] });

const TYPE = anAgentType({
  name: "coder",
  source: 'name = "coder"\nadapter = "claude"\nplugins = ["lint@acme-tools"]\n',
  plugins: ["lint@acme-tools"],
});

function renderPicker(installed: Plugin[], type: AgentType = TYPE, onSaved = vi.fn()) {
  vi.spyOn(pluginsApi, "listPlugins").mockResolvedValue({ available: [], installed });
  render(<AgentPluginsPicker type={type} onSaved={onSaved} />);
  return onSaved;
}

afterEach(() => vi.restoreAllMocks());

describe("AgentPluginsPicker", () => {
  it("lists the installed plugins with what they add and ticks the ones the type uses", async () => {
    renderPicker([WEB, LINT]);
    const web = await screen.findByRole("checkbox", { name: "modern-web-guidance" });
    expect(web).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "lint" })).toBeChecked();
    const row = web.closest("li") as HTMLElement;
    expect(within(row).getByText("2 skills")).toBeInTheDocument();
    expect(within(row).getByText("claude-plugins-official")).toBeInTheDocument();
    expect(within(screen.getByRole("checkbox", { name: "lint" }).closest("li") as HTMLElement).getByText("1 command")).toBeInTheDocument();
    expect(within(screen.getByRole("checkbox", { name: "lint" }).closest("li") as HTMLElement).getByText("disabled")).toBeInTheDocument();
  });

  it("saves the toml with the plugin added and tells the page", async () => {
    const save = vi.spyOn(libraryApi, "saveAgentType").mockResolvedValue(TYPE);
    const onSaved = renderPicker([WEB, LINT]);
    await userEvent.click(await screen.findByRole("checkbox", { name: "modern-web-guidance" }));
    expect(save).toHaveBeenCalledWith(
      "coder",
      'name = "coder"\nadapter = "claude"\nplugins = ["lint@acme-tools", "modern-web-guidance@claude-plugins-official"]\n',
    );
    await vi.waitFor(() => expect(onSaved).toHaveBeenCalled());
  });

  it("saves the toml without the plugin when it is unticked", async () => {
    const save = vi.spyOn(libraryApi, "saveAgentType").mockResolvedValue(TYPE);
    renderPicker([WEB, LINT]);
    await userEvent.click(await screen.findByRole("checkbox", { name: "lint" }));
    expect(save).toHaveBeenCalledWith("coder", 'name = "coder"\nadapter = "claude"\n');
  });

  it("shows the daemon's reason when the save is refused and does not tell the page", async () => {
    vi.spyOn(libraryApi, "saveAgentType").mockRejectedValue(new ApiError("http", "owned by plugin x@y", 409));
    const onSaved = renderPicker([WEB, LINT]);
    await userEvent.click(await screen.findByRole("checkbox", { name: "modern-web-guidance" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("owned by plugin x@y");
    expect(onSaved).not.toHaveBeenCalled();
  });

  it("offers a way to drop a chosen plugin that is no longer installed", async () => {
    const save = vi.spyOn(libraryApi, "saveAgentType").mockResolvedValue(TYPE);
    renderPicker([WEB]);
    const gone = await screen.findByRole("checkbox", { name: "lint@acme-tools" });
    expect(gone).toBeChecked();
    expect(within(gone.closest("li") as HTMLElement).getByText("not installed")).toBeInTheDocument();
    await userEvent.click(gone);
    expect(save).toHaveBeenCalledWith("coder", 'name = "coder"\nadapter = "claude"\n');
  });

  it("points to the plugins page when nothing is installed", async () => {
    renderPicker([], anAgentType({ name: "coder" }));
    expect(await screen.findByText(/no plugins installed/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "plugins" })).toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).toBeNull();
  });

  it("is read only for a type that comes from a plugin", async () => {
    renderPicker([WEB, LINT], anAgentType({ name: "notes-writer", origin: "plugin", plugin: "notes@acme" }));
    expect(await screen.findByText(/comes from a plugin/)).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "lint" })).toBeDisabled();
  });

  it("says townhall's own conversation type never loads plugin skills and offers no choice", async () => {
    renderPicker([WEB, LINT], anAgentType({ name: "default" }));
    expect(await screen.findByText(/never loads plugin skills/)).toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).toBeNull();
  });
});
