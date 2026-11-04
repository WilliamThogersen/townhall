import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "../../settings/settings";
import { getSettings, updateSettings } from "../../settings/settingsStore";
import { saveThemeChoice } from "../../theme/theme";
import { SettingsPage } from "./SettingsPage";

const group = (name: string) => within(screen.getByRole("radiogroup", { name }));
const permissionHelp = () => screen.getByText(/^agents|^edits go through/, { selector: ".th-setting-help" });

beforeEach(() => {
  updateSettings(DEFAULT_SETTINGS);
  saveThemeChoice("dark");
  localStorage.clear();
});

describe("SettingsPage", () => {
  it("shows the three groups and how changes are saved", () => {
    render(<SettingsPage />);
    expect(screen.getByRole("heading", { level: 1, name: "settings" })).toBeInTheDocument();
    expect(screen.getByText("saved as you change them")).toBeInTheDocument();
    ["agents", "conversations", "appearance and notifications"].forEach((name) => {
      expect(screen.getByRole("heading", { level: 2, name })).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: /save/ })).not.toBeInTheDocument();
  });

  it("starts from the stored settings", () => {
    updateSettings({ permissionMode: "plan", agentsAtOnce: 9, repoUnclear: "guess", notifyWhen: "failed" });
    render(<SettingsPage />);
    expect(group("Permissions").getByRole("radio", { name: "plan" })).toBeChecked();
    expect(screen.getByRole("group", { name: "Agents at once" })).toHaveTextContent("9");
    expect(group("When the repo is unclear").getByRole("radio", { name: "guess" })).toBeChecked();
    expect(group("Notify me when").getByRole("radio", { name: "+ failed" })).toBeChecked();
  });

  it("changes the permission mode and its help line", async () => {
    render(<SettingsPage />);
    expect(group("Permissions").getByRole("radio", { name: "accept edits" })).toBeChecked();
    expect(permissionHelp()).toHaveTextContent("edits go through, commands ask first.");
    const cases = [
      ["ask", "default", "agents ask before every edit or command."],
      ["plan", "plan", "agents plan first and change nothing."],
      ["auto", "auto", "agents decide what needs your approval."],
    ] as const;
    for (const [label, mode, help] of cases) {
      await userEvent.click(group("Permissions").getByRole("radio", { name: label }));
      expect(getSettings().permissionMode).toBe(mode);
      expect(permissionHelp()).toHaveTextContent(help);
      expect(permissionHelp()).not.toHaveAttribute("data-tone");
    }
    expect(JSON.parse(localStorage.getItem("th-settings") ?? "{}")).toMatchObject({ permissionMode: "auto" });
  });

  it("marks skip all as dangerous and explains it on demand", async () => {
    render(<SettingsPage />);
    const skip = group("Permissions").getByRole("radio", { name: "skip all" });
    expect(skip.closest("label")).toHaveAttribute("data-tone", "danger");
    await userEvent.click(skip);
    expect(getSettings().permissionMode).toBe("bypassPermissions");
    expect(permissionHelp()).toHaveAttribute("data-tone", "danger");
    expect(permissionHelp()).toHaveTextContent("agents never ask. only use on repos you trust.");
    const more = screen.getByRole("button", { name: "more" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    const details = document.getElementById(more.getAttribute("aria-controls") ?? "");
    expect(details).not.toBeVisible();
    await userEvent.click(more);
    expect(details).toBeVisible();
    expect(details).toHaveTextContent("--dangerously-skip-permissions");
    expect(screen.getByRole("button", { name: "less" })).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(screen.getByRole("button", { name: "less" }));
    expect(details).not.toBeVisible();
  });

  it("drops the danger help when leaving skip all", async () => {
    render(<SettingsPage />);
    await userEvent.click(group("Permissions").getByRole("radio", { name: "skip all" }));
    await userEvent.click(screen.getByRole("button", { name: "more" }));
    await userEvent.click(group("Permissions").getByRole("radio", { name: "ask" }));
    expect(screen.queryByRole("button", { name: /more|less/ })).not.toBeInTheDocument();
    expect(screen.queryByText(/--dangerously-skip-permissions/)).not.toBeInTheDocument();
    await userEvent.click(group("Permissions").getByRole("radio", { name: "skip all" }));
    expect(screen.getByRole("button", { name: "more" })).toHaveAttribute("aria-expanded", "false");
  });

  it("switches work without asking", async () => {
    render(<SettingsPage />);
    const toggle = screen.getByRole("switch", { name: "Work without asking" });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await userEvent.click(toggle);
    expect(getSettings().workWithoutAsking).toBe(true);
    expect(toggle).toHaveAttribute("aria-checked", "true");
  });

  it("steps agents at once", async () => {
    render(<SettingsPage />);
    const stepper = within(screen.getByRole("group", { name: "Agents at once" }));
    await userEvent.click(stepper.getByRole("button", { name: "More" }));
    expect(getSettings().agentsAtOnce).toBe(7);
    await userEvent.click(stepper.getByRole("button", { name: "Fewer" }));
    await userEvent.click(stepper.getByRole("button", { name: "Fewer" }));
    expect(getSettings().agentsAtOnce).toBe(5);
    expect(stepper.getByText("5")).toBeInTheDocument();
  });

  it("chooses what happens when the repo is unclear", async () => {
    render(<SettingsPage />);
    expect(group("When the repo is unclear").getByRole("radio", { name: "ask" })).toBeChecked();
    await userEvent.click(group("When the repo is unclear").getByRole("radio", { name: "guess" }));
    expect(getSettings().repoUnclear).toBe("guess");
  });

  it("switches the session panel default", async () => {
    render(<SettingsPage />);
    const toggle = screen.getByRole("switch", { name: "Session panel open by default" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await userEvent.click(toggle);
    expect(getSettings().sessionPanelOpen).toBe(false);
  });

  it("changes the theme of the app at once", async () => {
    render(<SettingsPage />);
    expect(group("Theme").getByRole("radio", { name: "dark" })).toBeChecked();
    await userEvent.click(group("Theme").getByRole("radio", { name: "light" }));
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
    expect(localStorage.getItem("th-theme")).toBe("light");
    expect(group("Theme").getByRole("radio", { name: "light" })).toBeChecked();
    expect(JSON.parse(localStorage.getItem("th-settings") ?? "{}")).not.toHaveProperty("theme");
  });

  it("chooses when to notify", async () => {
    render(<SettingsPage />);
    expect(group("Notify me when").getByRole("radio", { name: "needs you" })).toBeChecked();
    await userEvent.click(group("Notify me when").getByRole("radio", { name: "everything" }));
    expect(getSettings().notifyWhen).toBe("everything");
    await userEvent.click(group("Notify me when").getByRole("radio", { name: "+ failed" }));
    expect(getSettings().notifyWhen).toBe("failed");
  });
});
