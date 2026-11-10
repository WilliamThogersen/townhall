import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { aPreview } from "../../api/fixtures";
import { PermissionList, PermissionPrompt } from "./PermissionPrompt";

const preview = aPreview({
  permissions: [
    "api:tasks:read",
    "claude.hooks:PreToolUse",
    "env:GITHUB_TOKEN",
    "runs.hook:taskEnded:./notify.sh",
    "ui:sidebar",
  ],
});

function renderPrompt(over: Partial<Parameters<typeof PermissionPrompt>[0]> = {}) {
  const onInstall = vi.fn();
  const onCancel = vi.fn();
  render(
    <PermissionPrompt
      id="lint@acme-tools"
      official={false}
      preview={preview}
      error={null}
      busy={false}
      onInstall={onInstall}
      onCancel={onCancel}
      {...over}
    />,
  );
  return { onInstall, onCancel };
}

describe("PermissionPrompt", () => {
  it("shows what the plugin is and every permission grouped by what it can do", () => {
    renderPrompt();
    const dialog = screen.getByRole("dialog", { name: "install lint" });
    expect(within(dialog).getByText("lint@acme-tools")).toBeInTheDocument();
    expect(within(dialog).getByText("1111111")).toBeInTheDocument();
    const groups = within(dialog).getAllByRole("group");
    expect(groups.map((g) => g.getAttribute("aria-label"))).toEqual([
      "runs programs on this machine, as you",
      "changes what agents do",
      "reads or writes townhall data",
      "shows UI in these places",
      "reads these environment variables",
    ]);
    expect(within(groups[0]!).getByText("hook taskEnded runs ./notify.sh")).toBeInTheDocument();
    expect(within(groups[4]!).getByText("GITHUB_TOKEN")).toBeInTheDocument();
  });

  it("flags PreToolUse", () => {
    renderPrompt();
    expect(screen.getByText("can block or change any tool an agent calls")).toBeInTheDocument();
  });

  it("badges an official plugin and still shows the list", () => {
    renderPrompt({ official: true });
    expect(screen.getByText("official")).toBeInTheDocument();
    expect(screen.getByText("sidebar")).toBeInTheDocument();
  });

  it("installs not enabled, or installs and enables", async () => {
    const { onInstall } = renderPrompt();
    await userEvent.click(screen.getByRole("button", { name: "install" }));
    expect(onInstall).toHaveBeenLastCalledWith(false);
    await userEvent.click(screen.getByRole("button", { name: "install and enable" }));
    expect(onInstall).toHaveBeenLastCalledWith(true);
  });

  it("cancels with the button and with escape", async () => {
    const { onCancel } = renderPrompt();
    await userEvent.click(screen.getByRole("button", { name: "cancel" }));
    await userEvent.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(2);
  });

  it("says so when the plugin asks for nothing", () => {
    renderPrompt({ preview: aPreview({ permissions: [] }) });
    expect(screen.getByText("asks for no permissions")).toBeInTheDocument();
  });

  it("waits for the preview and cannot install before it", async () => {
    renderPrompt({ preview: null });
    expect(await screen.findByText(/loading permissions/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "install" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "install and enable" })).toBeDisabled();
  });

  it("shows why the preview failed and cannot install", () => {
    renderPrompt({ preview: null, error: "could not clone" });
    expect(screen.getByRole("alert")).toHaveTextContent("could not clone");
    expect(screen.getByRole("button", { name: "install" })).toBeDisabled();
  });

  it("disables both buttons while installing", () => {
    renderPrompt({ busy: true });
    expect(screen.getByRole("button", { name: "install" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "installing" })).toBeDisabled();
  });
});

describe("PermissionList", () => {
  it("lists the groups of any permission list", () => {
    render(<PermissionList permissions={["api:tasks:create"]} />);
    expect(screen.getByRole("group", { name: "reads or writes townhall data" })).toBeInTheDocument();
  });
});
