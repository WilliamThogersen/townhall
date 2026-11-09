import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { AgentPrompt } from "./AgentPrompt";

const PROMPT =
  "You are the BUILD step.\n\n<role>\nImplement new behaviour.\n</role>\n\n<process>\n1. Read the brief\n2. Write a test\n</process>\n\n<constraints>\n- Stay inside the task\n- Never skip a test\n</constraints>";

const header = (name: RegExp | string) => screen.getByRole("button", { name });

describe("AgentPrompt", () => {
  it("says there is no prompt for null and blank prompts", () => {
    const { rerender } = render(<AgentPrompt prompt={null} />);
    expect(screen.getByText("no prompt")).toBeInTheDocument();
    rerender(<AgentPrompt prompt={"  \n"} />);
    expect(screen.getByText("no prompt")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "copy prompt" })).not.toBeInTheDocument();
  });

  it("falls back to wrapped plain text when there are no tags", () => {
    render(<AgentPrompt prompt={"You implement the task.\nBe brief."} />);
    expect(screen.getByText(/You implement the task\./)).toBeInTheDocument();
    expect(screen.getByText(/0 sections/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /role/ })).not.toBeInTheDocument();
  });

  it("summarises characters, tokens and sections", () => {
    render(<AgentPrompt prompt={PROMPT} />);
    expect(screen.getByText(`${PROMPT.length} chars · ~${Math.ceil(PROMPT.length / 4)} tokens · 3 sections`)).toBeInTheDocument();
  });

  it("shows the intro, expands the first section and collapses the rest", () => {
    render(<AgentPrompt prompt={PROMPT} />);
    expect(screen.getByText("You are the BUILD step.")).toBeInTheDocument();
    expect(header("role")).toHaveAttribute("aria-expanded", "true");
    expect(header("process")).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("Implement new behaviour.")).toBeInTheDocument();
    expect(screen.queryByText("Read the brief")).not.toBeInTheDocument();
  });

  it("toggles one section", async () => {
    const user = userEvent.setup();
    render(<AgentPrompt prompt={PROMPT} />);
    await user.click(header("process"));
    expect(screen.getByText("Read the brief")).toBeInTheDocument();
    await user.click(header("process"));
    expect(screen.queryByText("Read the brief")).not.toBeInTheDocument();
  });

  it("expands all and collapses all", async () => {
    const user = userEvent.setup();
    render(<AgentPrompt prompt={PROMPT} />);
    await user.click(screen.getByRole("button", { name: "expand all" }));
    for (const tag of ["role", "process", "constraints"]) expect(header(tag)).toHaveAttribute("aria-expanded", "true");
    await user.click(screen.getByRole("button", { name: "collapse all" }));
    for (const tag of ["role", "process", "constraints"]) expect(header(tag)).toHaveAttribute("aria-expanded", "false");
  });

  it("renders numbered and dash lists as lists", async () => {
    const user = userEvent.setup();
    render(<AgentPrompt prompt={PROMPT} />);
    await user.click(screen.getByRole("button", { name: "expand all" }));
    const numbered = screen.getByText("Read the brief").closest("ol");
    expect(numbered).not.toBeNull();
    expect(within(numbered as HTMLElement).getAllByRole("listitem")).toHaveLength(2);
    const dashed = screen.getByText("Stay inside the task").closest("ul");
    expect(within(dashed as HTMLElement).getAllByRole("listitem")).toHaveLength(2);
  });

  it("flags a section whose closing tag is missing", () => {
    render(<AgentPrompt prompt={"<role>\nImplement."} />);
    expect(screen.getByText("not closed")).toBeInTheDocument();
  });

  it("copies the whole prompt", async () => {
    const user = userEvent.setup();
    render(<AgentPrompt prompt={PROMPT} />);
    await user.click(screen.getByRole("button", { name: "copy prompt" }));
    expect(await navigator.clipboard.readText()).toBe(PROMPT);
  });

  it("says the prompt is read-only and edited in the toml", () => {
    render(<AgentPrompt prompt={PROMPT} />);
    expect(screen.getByText(/read-only/)).toBeInTheDocument();
  });

  it("resets which sections are open when the prompt changes", async () => {
    const user = userEvent.setup();
    const { rerender } = render(<AgentPrompt prompt={PROMPT} />);
    await user.click(header("process"));
    rerender(<AgentPrompt prompt={PROMPT.replace("Write a test", "Write two tests")} />);
    expect(header("process")).toHaveAttribute("aria-expanded", "false");
  });
});
