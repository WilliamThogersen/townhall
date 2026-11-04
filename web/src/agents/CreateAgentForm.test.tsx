import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import { ApiError } from "../api/errors";
import type { Agent } from "../api/types";
import { CreateAgentForm } from "./CreateAgentForm";

const agent = (name: string): Agent => ({
  name,
  kind: "shell",
  branch: `townhall/${name}`,
  worktree: `/w/${name}`,
  alive: true,
});

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

afterEach(() => vi.restoreAllMocks());

function show(onCreated = vi.fn()) {
  const user = userEvent.setup();
  render(<CreateAgentForm id="new-agent" onCreated={onCreated} />);
  return { user, onCreated };
}

const start = () => screen.getByRole("button", { name: "start" });

describe("CreateAgentForm", () => {
  it("renders the fields in the v2 style with the name focused", () => {
    show();
    const form = screen.getByRole("form", { name: "new agent" });
    expect(form).toHaveClass("th-new-agent");
    expect(form).toHaveAttribute("id", "new-agent");
    const name = screen.getByLabelText("name");
    expect(name).toHaveClass("th-input");
    expect(name).toHaveFocus();
    expect(screen.getByLabelText("base branch")).toHaveClass("th-input");
    expect(screen.getByLabelText("prompt")).toHaveClass("th-input");
    expect(screen.queryByLabelText("command")).toBeNull();
    const adapter = screen.getByRole("radiogroup", { name: "adapter" });
    expect(adapter).toHaveClass("th-segmented");
    expect(screen.getByRole("radio", { name: "claude" })).toBeChecked();
    expect(start()).toHaveClass("th-btn", "th-btn--primary");
    expect(start()).toHaveAttribute("type", "submit");
  });

  it("swaps the prompt for a command when shell is chosen", async () => {
    const { user } = show();
    await user.click(screen.getByRole("radio", { name: "shell" }));
    expect(screen.getByLabelText("command")).toHaveClass("th-input");
    expect(screen.queryByLabelText("prompt")).toBeNull();
  });

  it("shows field errors only after a submit and keeps them live", async () => {
    const create = vi.spyOn(client, "createAgent");
    const { user } = show();
    expect(screen.queryByText(/^error:/)).toBeNull();
    await user.click(screen.getByRole("radio", { name: "shell" }));
    await user.type(screen.getByLabelText("base branch"), "-x");
    await user.click(start());
    expect(create).not.toHaveBeenCalled();
    const name = screen.getByLabelText("name");
    expect(name).toHaveAttribute("aria-invalid", "true");
    expect(name).toHaveAccessibleDescription("error: name is required");
    expect(screen.getByText("error: name is required")).toHaveClass("th-field-error");
    expect(screen.getByText("error: a shell agent needs a command")).toBeInTheDocument();
    expect(screen.getByText("error: base branch must not start with -")).toBeInTheDocument();
    await user.type(name, "bad name");
    expect(screen.getByText("error: use letters, digits, - and _ only")).toBeInTheDocument();
    await user.clear(name);
    await user.type(name, "ok");
    expect(name).toHaveAttribute("aria-invalid", "false");
    expect(screen.queryByText(/^error: use letters/)).toBeNull();
  });

  it("starts a shell agent, shows starting… while busy, then reports the name and clears", async () => {
    const pending = deferred<Agent>();
    const create = vi.spyOn(client, "createAgent").mockReturnValue(pending.promise);
    const { user, onCreated } = show();
    await user.type(screen.getByLabelText("name"), "u15-a");
    await user.click(screen.getByRole("radio", { name: "shell" }));
    await user.type(screen.getByLabelText("command"), " cat ");
    await user.type(screen.getByLabelText("base branch"), "main");
    await user.click(start());
    expect(create).toHaveBeenCalledWith({ name: "u15-a", adapter: "shell", command: "cat", base: "main" });
    const busy = screen.getByRole("button", { name: /starting…/ });
    expect(busy).toBeDisabled();
    pending.resolve(agent("u15-a"));
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith("u15-a"));
    expect(start()).toBeEnabled();
    expect(screen.getByLabelText("name")).toHaveValue("");
  });

  it("sends a trimmed prompt for claude", async () => {
    const create = vi.spyOn(client, "createAgent").mockResolvedValue(agent("x"));
    const { user } = show();
    await user.type(screen.getByLabelText("name"), "x");
    await user.type(screen.getByLabelText("prompt"), "  fix it ");
    await user.click(start());
    expect(create).toHaveBeenCalledWith({ name: "x", adapter: "claude", prompt: "fix it" });
  });

  it("shows the daemon's error and keeps the values", async () => {
    vi.spyOn(client, "createAgent").mockRejectedValue(new ApiError("http", "agent u15-a already exists", 409));
    const { user, onCreated } = show();
    await user.type(screen.getByLabelText("name"), "u15-a");
    await user.click(start());
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("error: agent u15-a already exists");
    expect(alert).toHaveClass("th-field-error");
    expect(onCreated).not.toHaveBeenCalled();
    expect(screen.getByLabelText("name")).toHaveValue("u15-a");
    expect(start()).toBeEnabled();
  });
});
