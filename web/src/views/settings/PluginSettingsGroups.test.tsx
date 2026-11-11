import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { aSettingField, someContributions } from "../../api/fixtures";
import * as pluginsApi from "../../api/plugins";
import type { PluginContributions, PluginSettingField } from "../../api/types";
import { TestDataProvider } from "../../data/testing";
import { PluginSettingsGroups } from "./PluginSettingsGroups";

afterEach(() => vi.restoreAllMocks());

const ID = "lint@acme-tools";

const group = (fields: readonly PluginSettingField[]): PluginContributions =>
  someContributions({ settings: [{ plugin: ID, name: "lint", fields }] });

function renderGroups(contributions: PluginContributions) {
  vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(contributions);
  return render(
    <TestDataProvider>
      <PluginSettingsGroups />
    </TestDataProvider>,
  );
}

const verbose = aSettingField({ key: "verbose", label: "verbose", type: "boolean", default: false, value: false, min: null, max: null });
const mode = aSettingField({
  key: "mode",
  label: "mode",
  type: "enum",
  default: "fast",
  value: "fast",
  choices: ["fast", "slow"],
  min: null,
  max: null,
  description: "how hard it works",
});
const name = aSettingField({ key: "name", label: "name", type: "string", default: "", value: "", min: null, max: null });
const token = aSettingField({
  key: "token",
  label: "token",
  type: "string",
  default: "",
  value: null,
  secret: true,
  min: null,
  max: null,
});

describe("PluginSettingsGroups", () => {
  it("renders nothing while no plugin has settings", async () => {
    const { container } = renderGroups(someContributions());
    await waitFor(() => expect(pluginsApi.getContributions).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("makes one group per plugin from its schema, named after the plugin", async () => {
    renderGroups(group([aSettingField(), verbose, mode, name, token]));
    expect(await screen.findByRole("heading", { level: 2, name: "lint (plugin)" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "retries" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "verbose" })).toHaveAttribute("aria-checked", "false");
    expect(screen.getByRole("radiogroup", { name: "mode" })).toBeInTheDocument();
    expect(screen.getByText("how hard it works")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "name" })).toHaveValue("");
    expect(screen.getByLabelText("token")).toHaveAttribute("type", "password");
  });

  it("saves a switch, a stepper and an enum choice as the user changes them", async () => {
    const put = vi.spyOn(pluginsApi, "putPluginSetting").mockResolvedValue(group([aSettingField(), verbose, mode]));
    renderGroups(group([aSettingField(), verbose, mode]));
    const user = userEvent.setup();
    await user.click(await screen.findByRole("switch", { name: "verbose" }));
    expect(put).toHaveBeenLastCalledWith(ID, "verbose", true);
    await user.click(within(screen.getByRole("group", { name: "retries" })).getByRole("button", { name: "More" }));
    expect(put).toHaveBeenLastCalledWith(ID, "retries", 4);
    await user.click(within(screen.getByRole("radiogroup", { name: "mode" })).getByLabelText("slow"));
    expect(put).toHaveBeenLastCalledWith(ID, "mode", "slow");
  });

  it("saves text only when it was changed and the user presses save", async () => {
    const put = vi.spyOn(pluginsApi, "putPluginSetting").mockResolvedValue(group([{ ...name, value: "ada", set: true }]));
    renderGroups(group([name]));
    const user = userEvent.setup();
    const input = await screen.findByRole("textbox", { name: "name" });
    const save = screen.getByRole("button", { name: "save name" });
    expect(save).toBeDisabled();
    await user.type(input, "ada");
    expect(save).toBeEnabled();
    await user.click(save);
    expect(put).toHaveBeenCalledWith(ID, "name", "ada");
    await waitFor(() => expect(screen.getByRole("textbox", { name: "name" })).toHaveValue("ada"));
    expect(screen.getByRole("button", { name: "save name" })).toBeDisabled();
  });

  it("never shows a secret and clears the box once it is saved", async () => {
    const put = vi.spyOn(pluginsApi, "putPluginSetting").mockResolvedValue(group([{ ...token, set: true }]));
    renderGroups(group([token]));
    const user = userEvent.setup();
    const input = await screen.findByLabelText("token");
    expect(input).toHaveAttribute("placeholder", "not set");
    await user.type(input, "hunter2");
    await user.click(screen.getByRole("button", { name: "save token" }));
    expect(put).toHaveBeenCalledWith(ID, "token", "hunter2");
    await waitFor(() => expect(screen.getByLabelText("token")).toHaveValue(""));
    expect(screen.getByLabelText("token")).toHaveAttribute("placeholder", "saved: type to replace it");
    expect(document.body.innerHTML).not.toContain("hunter2");
  });

  it("offers reset only for a value the user saved and asks the daemon to reset it", async () => {
    const reset = vi.spyOn(pluginsApi, "resetPluginSetting").mockResolvedValue(group([aSettingField()]));
    renderGroups(group([aSettingField({ value: 5, set: true }), mode]));
    const user = userEvent.setup();
    await screen.findByRole("group", { name: "retries" });
    expect(screen.getAllByRole("button", { name: /^reset/ })).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "reset retries" }));
    expect(reset).toHaveBeenCalledWith(ID, "retries");
    await waitFor(() => expect(screen.queryByRole("button", { name: "reset retries" })).not.toBeInTheDocument());
  });

  it("shows the daemon's reason and keeps the value when a save is refused", async () => {
    vi.spyOn(pluginsApi, "putPluginSetting").mockRejectedValue(new ApiError("http", "retries: must be at most 9", 400));
    renderGroups(group([aSettingField({ value: 8, set: true })]));
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "More" }));
    expect(await screen.findByText("lint settings not saved: retries: must be at most 9")).toBeInTheDocument();
    expect(within(screen.getByRole("group", { name: "retries" })).getByRole("status")).toHaveTextContent("8");
  });
});
