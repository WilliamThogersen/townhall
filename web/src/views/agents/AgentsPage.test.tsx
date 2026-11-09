import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { anAgentType, anAgentTypeSummary } from "../../api/fixtures";
import * as library from "../../api/library";
import { AgentsPage } from "./AgentsPage";

const builder = anAgentTypeSummary({ name: "builder" });
const planner = anAgentTypeSummary({
  name: "planner",
  description: "plans the work",
  model: "opus",
  permissionMode: "plan",
});
const shellType = anAgentTypeSummary({ name: "dev-server", description: "runs the dev server", adapter: "shell", model: null, permissionMode: null });

const plannerFull = anAgentType({
  ...planner,
  tools: ["Read", "Grep"],
  source: 'name = "planner"\nmodel = "opus"\n',
});

beforeEach(() => {
  vi.spyOn(library, "listAgentUsage").mockResolvedValue([
    { agent: "builder", runs: 3, tokens: 12_400 },
    { agent: "planner", runs: 1, tokens: 800 },
  ]);
  vi.spyOn(library, "listAgentTypes").mockResolvedValue([builder, planner, shellType]);
  vi.spyOn(library, "getAgentType").mockImplementation((name) =>
    Promise.resolve(name === "planner" ? plannerFull : anAgentType({ name, source: `name = "${name}"\n` })),
  );
});

afterEach(() => vi.restoreAllMocks());

const rowOf = async (name: string) => (await screen.findByRole("button", { name: new RegExp(name) })) as HTMLElement;

describe("AgentsPage", () => {
  it("lists every type with description, model, permission mode and adapter", async () => {
    render(<AgentsPage />);
    expect(screen.getByRole("heading", { level: 1, name: "agents" })).toBeInTheDocument();
    const row = within(await rowOf("planner"));
    expect(row.getByText("plans the work")).toBeInTheDocument();
    expect(row.getByText("opus")).toBeInTheDocument();
    expect(row.getByText("plan", { selector: ".th-agent-mode" })).toBeInTheDocument();
    expect(row.getByText("claude")).toBeInTheDocument();
    expect(within(await rowOf("dev-server")).getByText("shell")).toBeInTheDocument();
  });

  it("marks the default type", async () => {
    render(<AgentsPage />);
    expect(within(await rowOf("builder")).getByText("default")).toBeInTheDocument();
    expect(within(await rowOf("planner")).queryByText("default")).not.toBeInTheDocument();
  });

  it("badges a type with its pipeline step and leaves unknown types bare", async () => {
    render(<AgentsPage />);
    expect(within(await rowOf("planner")).getByText("plan", { selector: ".th-tag" })).toBeInTheDocument();
    expect(within(await rowOf("builder")).queryByText(/^(plan|code|test|review|ship)$/, { selector: ".th-tag" })).not.toBeInTheDocument();
  });

  it("asks to pick a type before one is selected", async () => {
    render(<AgentsPage />);
    await rowOf("builder");
    expect(screen.getByText("pick an agent type to see its TOML")).toBeInTheDocument();
  });

  it("shows the parsed fields and the TOML source of the selected type", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    expect(await detail.findByText("Read, Grep")).toBeInTheDocument();
    expect(detail.getByText("opus")).toBeInTheDocument();
    expect(detail.getByText("plan", { selector: "dd" })).toBeInTheDocument();
    expect(detail.getByText("claude")).toBeInTheDocument();
    const source = detail.getByText(/name = "planner"/);
    expect(source.tagName).toBe("PRE");
    expect(library.getAgentType).toHaveBeenCalledWith("planner", expect.anything());
    expect(await rowOf("planner")).toHaveAttribute("aria-pressed", "true");
  });

  it("shows no tools line value as defaults when a type lists none", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("builder"));
    expect(await screen.findByText("claude defaults")).toBeInTheDocument();
  });

  it("shows where a type may hand its task back to", async () => {
    vi.mocked(library.getAgentType).mockImplementation((name) =>
      Promise.resolve(
        anAgentType({
          name,
          canHandBackTo: name === "planner" ? ["builder"] : name === "builder" ? [] : null,
        }),
      ),
    );
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    expect(await screen.findByText("builder", { selector: "dd" })).toBeInTheDocument();
    await userEvent.click(await rowOf("builder"));
    expect(await screen.findByText("nobody", { selector: "dd" })).toBeInTheDocument();
    await userEvent.click(await rowOf("dev-server"));
    expect(await screen.findByText("any type", { selector: "dd" })).toBeInTheDocument();
  });

  it("lists the env variable names of a type but not their values", async () => {
    vi.mocked(library.getAgentType).mockImplementation((name) =>
      Promise.resolve(
        anAgentType({ name, env: name === "planner" ? { ZETA: "hunter2", ALPHA: "1" } : {}, source: 'name = "x"\n' }),
      ),
    );
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    expect(await detail.findByText("ALPHA, ZETA", { selector: "dd" })).toBeInTheDocument();
    expect(screen.queryByText(/hunter2/)).not.toBeInTheDocument();
    await userEvent.click(await rowOf("builder"));
    expect(await screen.findByText("none", { selector: "dd" })).toBeInTheDocument();
  });

  it("copies the TOML", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    await userEvent.click(await screen.findByRole("button", { name: "copy" }));
    expect(writeText).toHaveBeenCalledWith(plannerFull.source);
    expect(await screen.findByRole("button", { name: "copied" })).toBeInTheDocument();
  });

  it("renders the copy button as a ghost button that marks the copied state", async () => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: vi.fn().mockResolvedValue(undefined) }, configurable: true });
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const button = await screen.findByRole("button", { name: "copy" });
    expect(button).toHaveClass("th-btn--ghost");
    expect(button).not.toHaveAttribute("data-copied");
    await userEvent.click(button);
    expect(await screen.findByRole("button", { name: "copied" })).toHaveAttribute("data-copied", "true");
  });

  it("offers no edit or delete", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    await screen.findByRole("region", { name: "planner" });
    expect(screen.queryByRole("button", { name: /^(edit|delete|save|remove)/i })).not.toBeInTheDocument();
  });

  it("shows loading, then an empty state", async () => {
    vi.spyOn(library, "listAgentTypes").mockResolvedValue([]);
    render(<AgentsPage />);
    expect(await screen.findByText("no agent types yet")).toBeInTheDocument();
  });

  it("shows an error banner when the list fails", async () => {
    vi.spyOn(library, "listAgentTypes").mockRejectedValue(new ApiError("network", "cannot reach the daemon"));
    render(<AgentsPage />);
    expect(await screen.findByRole("alert")).toHaveTextContent("cannot reach the daemon");
  });

  it("shows an error banner when one type fails to load", async () => {
    vi.spyOn(library, "getAgentType").mockRejectedValue(new ApiError("http", "agent type gone", 404));
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("agent type gone"));
  });

  it("shows runs and compact tokens on each row, zero for a type never run", async () => {
    render(<AgentsPage />);
    const builderRow = within(await rowOf("builder"));
    expect(await builderRow.findByText("3 runs")).toBeInTheDocument();
    expect(builderRow.getByText("12.4K tokens")).toBeInTheDocument();
    expect(within(await rowOf("planner")).getByText("1 run")).toBeInTheDocument();
    expect(within(await rowOf("planner")).getByText("800 tokens")).toBeInTheDocument();
    expect(within(await rowOf("dev-server")).getByText("0 runs")).toBeInTheDocument();
    expect(within(await rowOf("dev-server")).getByText("0 tokens")).toBeInTheDocument();
  });

  it("shows runs and tokens in the detail", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("builder"));
    const detail = within(await screen.findByRole("region", { name: "builder" }));
    expect(await detail.findByText("12.4K")).toBeInTheDocument();
    expect(detail.getByText("3", { selector: "dd" })).toBeInTheDocument();
  });

  it("still lists the types when usage fails to load", async () => {
    vi.spyOn(library, "listAgentUsage").mockRejectedValue(new ApiError("network", "no usage"));
    render(<AgentsPage />);
    expect(await rowOf("builder")).toBeInTheDocument();
    expect(within(await rowOf("builder")).queryByText(/runs?$/)).not.toBeInTheDocument();
  });
});
