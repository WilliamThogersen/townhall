import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { anAgentType, anAgentTypeSummary } from "../../api/fixtures";
import * as conversations from "../../api/conversations";
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
  vi.spyOn(conversations, "listConversations").mockResolvedValue([]);
  vi.spyOn(library, "getAgentUsageSeries").mockImplementation((name, range) =>
    Promise.resolve({
      agent: name,
      range,
      bucketMs: 3_600_000,
      since: "2026-11-05T00:00:00.000Z",
      until: "2026-11-05T02:00:00.000Z",
      runs: 3,
      tokens: 12_400,
      buckets: [
        { start: "2026-11-05T00:00:00.000Z", runs: 1, tokens: 400 },
        { start: "2026-11-05T01:00:00.000Z", runs: 2, tokens: 12_000 },
      ],
      byTask: [],
      byRepo: [],
      byConversation: [],
    }),
  );
  vi.spyOn(library, "listAgentUsage").mockResolvedValue([
    { agent: "builder", runs: 3, tokens: 12_400 },
    { agent: "planner", runs: 1, tokens: 800 },
  ]);
  vi.spyOn(library, "listAgentRuns").mockResolvedValue([]);
  vi.spyOn(library, "listAgentTypes").mockResolvedValue([builder, planner, shellType]);
  vi.spyOn(library, "getAgentType").mockImplementation((name) =>
    Promise.resolve(name === "planner" ? plannerFull : anAgentType({ name, source: `name = "${name}"\n` })),
  );
});

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

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

  it("shows effort in the row and the setup only when the type sets it", async () => {
    const lowPlanner = { ...planner, effort: "low" as const };
    vi.spyOn(library, "listAgentTypes").mockResolvedValue([builder, lowPlanner]);
    vi.mocked(library.getAgentType).mockImplementation((name) =>
      Promise.resolve(name === "planner" ? anAgentType({ ...lowPlanner }) : anAgentType({ name })),
    );
    render(<AgentsPage />);
    expect(within(await rowOf("planner")).getByText("low")).toBeInTheDocument();
    expect(within(await rowOf("builder")).queryByText(/^(low|medium|high|xhigh|max)$/)).not.toBeInTheDocument();
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    expect(await detail.findByText("effort")).toBeInTheDocument();
    expect(detail.getByText("low", { selector: "dd" })).toBeInTheDocument();
    await userEvent.click(await rowOf("builder"));
    const builderDetail = within(await screen.findByRole("region", { name: "builder" }));
    await builderDetail.findByText("permission mode");
    expect(builderDetail.queryByText("effort")).not.toBeInTheDocument();
  });

  it("shows skip all as bypassPermissions and a type with no mode as acceptEdits", async () => {
    const skipper = anAgentTypeSummary({ name: "skipper", permissionMode: "bypassPermissions" });
    vi.spyOn(library, "listAgentTypes").mockResolvedValue([builder, skipper, shellType]);
    vi.mocked(library.getAgentType).mockImplementation((name) =>
      Promise.resolve(
        anAgentType({ ...(name === "skipper" ? skipper : shellType), source: `name = "${name}"\n` }),
      ),
    );
    render(<AgentsPage />);
    expect(within(await rowOf("skipper")).getByText("bypassPermissions", { selector: ".th-agent-mode" })).toBeInTheDocument();
    await userEvent.click(await rowOf("skipper"));
    const detail = within(await screen.findByRole("region", { name: "skipper" }));
    expect(await detail.findByText(/bypassPermissions \(skip all/, { selector: "dd" })).toBeInTheDocument();
    await userEvent.click(await rowOf("dev-server"));
    const plain = within(await screen.findByRole("region", { name: "dev-server" }));
    expect(await plain.findByText("acceptEdits", { selector: "dd" })).toBeInTheDocument();
  });

  it("filters by effort", async () => {
    vi.spyOn(library, "listAgentTypes").mockResolvedValue([builder, { ...planner, effort: "xhigh" }]);
    render(<AgentsPage />);
    await rowOf("builder");
    await userEvent.type(screen.getByRole("searchbox", { name: "filter agent types" }), "xhigh");
    expect(screen.getByRole("button", { name: /planner/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /builder/ })).not.toBeInTheDocument();
  });

  it("marks the type tasks default to", async () => {
    render(<AgentsPage />);
    expect(within(await rowOf("builder")).getByText("task default")).toBeInTheDocument();
    expect(within(await rowOf("planner")).queryByText("task default")).not.toBeInTheDocument();
  });

  describe("the conversation default", () => {
    const conversationType = anAgentTypeSummary({ name: "default", description: "the conversation agent", model: null });

    beforeEach(() => {
      vi.spyOn(library, "listAgentTypes").mockResolvedValue([conversationType, builder, planner]);
    });

    const conversationRow = async () => {
      const name = await screen.findByText("default", { selector: "b" });
      return within(name.closest("button") as HTMLElement);
    };

    it("is listed and labelled as the conversation default, not the task default", async () => {
      render(<AgentsPage />);
      const row = await conversationRow();
      expect(row.getByText("conversation default")).toBeInTheDocument();
      expect(row.queryByText("task default")).not.toBeInTheDocument();
      expect(within(await rowOf("builder")).getByText("task default")).toBeInTheDocument();
    });

    it("lists the task default before it", async () => {
      render(<AgentsPage />);
      await conversationRow();
      const names = screen.getAllByRole("button", { pressed: false }).map((row) => row.querySelector("b")?.textContent);
      expect(names[0]).toBe("builder");
    });

    it("says in its detail that it runs the conversations and cannot be deleted", async () => {
      render(<AgentsPage />);
      await userEvent.click((await conversationRow()) && (await screen.findByText("default", { selector: "b" })));
      expect(await screen.findByText(/runs your conversations with townhall/)).toBeInTheDocument();
      expect(screen.getByText(/cannot be deleted/)).toBeInTheDocument();
    });

    it("gives other types no such note", async () => {
      render(<AgentsPage />);
      await userEvent.click(await rowOf("planner"));
      await screen.findByLabelText("planner");
      expect(screen.queryByText(/runs your conversations/)).not.toBeInTheDocument();
    });
  });

  describe("the adviser", () => {
    const adviser = anAgentTypeSummary({
      name: "adviser",
      description: "a senior sparring partner other agents consult; read-only, answers questions, never edits code",
      model: "claude-fable-5-2",
      effort: "high",
      permissionMode: "plan",
    });

    beforeEach(() => {
      vi.spyOn(library, "listAgentTypes").mockResolvedValue([adviser, builder, planner]);
      vi.mocked(library.getAgentType).mockImplementation((name) =>
        Promise.resolve(anAgentType({ ...(name === "adviser" ? adviser : builder), name, tools: ["Read", "Grep"], source: `name = "${name}"\n` })),
      );
    });

    it("is listed like the other built-ins, with its model, effort and mode, and is not the task default", async () => {
      render(<AgentsPage />);
      const row = within(await rowOf("adviser"));
      expect(row.getByText("claude-fable-5-2")).toBeInTheDocument();
      expect(row.getByText("high")).toBeInTheDocument();
      expect(row.getByText("plan", { selector: ".th-agent-mode" })).toBeInTheDocument();
      expect(row.getByText("consulted by agents")).toBeInTheDocument();
      expect(row.queryByText("task default")).not.toBeInTheDocument();
      expect(within(await rowOf("builder")).getByText("task default")).toBeInTheDocument();
    });

    it("says in its detail that agents ask it with ask_adviser and that it only reads", async () => {
      render(<AgentsPage />);
      await userEvent.click(await rowOf("adviser"));
      expect(await screen.findByText(/other agents ask it with the ask_adviser tool/)).toBeInTheDocument();
      expect(screen.getByText(/only reads and answers/)).toBeInTheDocument();
    });

    it("lists its consult runs with their model and tokens", async () => {
      vi.spyOn(library, "listAgentRuns").mockResolvedValue([
        { kind: "consult", conversationId: "c1", task: "fix-login", repo: "alpha", state: "done", model: "claude-fable-5-2", tokens: 1034, createdAt: "2026-11-09T10:00:00Z", endedAt: "2026-11-09T10:00:40Z" },
      ]);
      render(<AgentsPage />);
      await userEvent.click(await rowOf("adviser"));
      const detail = within(await screen.findByRole("region", { name: "adviser" }));
      expect(await detail.findByText("fix-login")).toBeInTheDocument();
      expect(detail.getAllByText("claude-fable-5-2").length).toBeGreaterThan(0);
    });

    it("gives other types no such note", async () => {
      render(<AgentsPage />);
      await userEvent.click(await rowOf("planner"));
      await screen.findByLabelText("planner");
      expect(screen.queryByText(/ask_adviser/)).not.toBeInTheDocument();
    });
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
    expect(await detail.findByText("Read", { selector: ".th-tag" })).toBeInTheDocument();
    expect(detail.getByText("Grep", { selector: ".th-tag" })).toBeInTheDocument();
    expect(detail.getByText("opus")).toBeInTheDocument();
    expect(detail.getByText("plan", { selector: "dd" })).toBeInTheDocument();
    expect(detail.getByText("claude")).toBeInTheDocument();
    const source = detail.getByText(/name = "planner"/);
    expect(source.tagName).toBe("PRE");
    expect(library.getAgentType).toHaveBeenCalledWith("planner", expect.anything());
    expect(await rowOf("planner")).toHaveAttribute("aria-pressed", "true");
  });

  it("shows the prompt of the selected type as sections above the TOML", async () => {
    vi.mocked(library.getAgentType).mockResolvedValue(
      anAgentType({ ...planner, prompt: "Intro text.\n<role>\nPlan the work.\n</role>", source: 'name = "planner"\n' }),
    );
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    const prompt = within(await detail.findByRole("region", { name: "prompt" }));
    expect(prompt.getByText("Intro text.")).toBeInTheDocument();
    expect(prompt.getByRole("button", { name: "role" })).toHaveAttribute("aria-expanded", "true");
    expect(prompt.getByText("Plan the work.")).toBeInTheDocument();
  });

  it("says no prompt when the selected type has none", async () => {
    vi.mocked(library.getAgentType).mockResolvedValue(anAgentType({ ...planner, prompt: null }));
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    expect(await detail.findByText("no prompt")).toBeInTheDocument();
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
    expect(await screen.findByText("builder", { selector: "dd .th-tag" })).toBeInTheDocument();
    await userEvent.click(await rowOf("builder"));
    expect(await screen.findByText("nobody", { selector: "dd" })).toBeInTheDocument();
    await userEvent.click(await rowOf("dev-server"));
    expect(await screen.findByText("any type", { selector: "dd" })).toBeInTheDocument();
  });

  it("badges a type that comes from a plugin with the plugin's id, and no other", async () => {
    vi.mocked(library.getAgentType).mockImplementation((name) =>
      Promise.resolve(
        anAgentType({
          name,
          source: 'name = "x"\n',
          ...(name === "planner" ? { origin: "plugin" as const, plugin: "lint@acme-tools" } : { origin: "user" as const, plugin: null }),
        }),
      ),
    );
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    expect(await detail.findByText("plugin lint@acme-tools")).toBeInTheDocument();
    await userEvent.click(await rowOf("builder"));
    const other = within(await screen.findByRole("region", { name: "builder" }));
    expect(other.queryByText(/^plugin /)).toBeNull();
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

  it("compares the recent runs of a type by model and tokens", async () => {
    vi.mocked(library.listAgentRuns).mockResolvedValue([
      { kind: "task", conversationId: "c1", task: "cheap-one", repo: "alpha", state: "ended", model: "claude-haiku", tokens: 12_400, createdAt: "2026-11-09T10:00:00Z", endedAt: null },
      { kind: "task", conversationId: "c1", task: "old-one", repo: "alpha", state: "failed", model: null, tokens: 3, createdAt: "2026-11-08T09:00:00Z", endedAt: null },
    ]);
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const runs = within(await screen.findByRole("region", { name: "recent runs" }));
    expect(library.listAgentRuns).toHaveBeenCalledWith("planner", expect.anything());
    const cheap = within(await runs.findByRole("row", { name: /cheap-one/ }));
    expect(cheap.getByText("claude-haiku")).toBeInTheDocument();
    expect(cheap.getByText("12.4K")).toBeInTheDocument();
    expect(cheap.getByText("ended")).toBeInTheDocument();
    expect(cheap.getByText("2026-11-09")).toBeInTheDocument();
    expect(runs.getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["task", "model", "tokens", "state", "started"]);
  });

  it("falls back to the type's model for a run that recorded none, and never says unknown", async () => {
    vi.mocked(library.listAgentRuns).mockResolvedValue([
      { kind: "task", conversationId: "c1", task: "old-one", repo: "alpha", state: "failed", model: null, tokens: 3, createdAt: "2026-11-08T09:00:00Z", endedAt: null },
    ]);
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const runs = within(await screen.findByRole("region", { name: "recent runs" }));
    const cell = within(await runs.findByRole("row", { name: /old-one/ })).getByText("opus");
    expect(cell).toHaveAttribute("title", "the agent type's model; the run did not record one");
    expect(screen.queryByText("unknown")).not.toBeInTheDocument();
  });

  it("hides the model column when no run and not the type knows a model", async () => {
    vi.mocked(library.getAgentType).mockImplementation((name) =>
      Promise.resolve(anAgentType({ name, model: null, source: `name = "${name}"\n` })),
    );
    vi.mocked(library.listAgentRuns).mockResolvedValue([
      { kind: "task", conversationId: "c1", task: "old-one", repo: "alpha", state: "failed", model: null, tokens: 3, createdAt: "2026-11-08T09:00:00Z", endedAt: null },
    ]);
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const runs = within(await screen.findByRole("region", { name: "recent runs" }));
    await runs.findByRole("row", { name: /old-one/ });
    expect(runs.getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["task", "tokens", "state", "started"]);
  });

  it("lists only the first eight runs until show more is pressed", async () => {
    vi.mocked(library.listAgentRuns).mockResolvedValue(
      Array.from({ length: 12 }, (_, i) => ({
        kind: "task",
        conversationId: "c1",
        task: `run-${i}`,
        repo: "alpha",
        state: "ended",
        model: "claude-haiku",
        tokens: i,
        createdAt: "2026-11-09T10:00:00Z",
        endedAt: null,
      })),
    );
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const runs = within(await screen.findByRole("region", { name: "recent runs" }));
    await runs.findByRole("row", { name: /run-0/ });
    expect(runs.getAllByRole("row")).toHaveLength(1 + 8);
    expect(runs.queryByRole("row", { name: /run-8/ })).not.toBeInTheDocument();
    await userEvent.click(runs.getByRole("button", { name: "show 4 more" }));
    expect(runs.getAllByRole("row")).toHaveLength(1 + 12);
    await userEvent.click(runs.getByRole("button", { name: "show fewer" }));
    expect(runs.getAllByRole("row")).toHaveLength(1 + 8);
  });

  it("links a conversation turn to its conversation and leaves a task as text", async () => {
    vi.mocked(library.listAgentRuns).mockResolvedValue([
      { kind: "turn", conversationId: "c9", task: "plan the login page", repo: "alpha", state: "done", model: null, tokens: 5040, createdAt: "2026-11-09T10:00:00Z", endedAt: "2026-11-09T10:00:05Z" },
      { kind: "task", conversationId: "c9", task: "a-task", repo: "alpha", state: "ended", model: null, tokens: 1, createdAt: "2026-11-09T09:00:00Z", endedAt: null },
    ]);
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const runs = within(await screen.findByRole("region", { name: "recent runs" }));
    const link = await runs.findByRole("link", { name: "plan the login page" });
    expect(link).toHaveAttribute("href", "/c/c9");
    expect(runs.queryByRole("link", { name: "a-task" })).not.toBeInTheDocument();
  });

  it("offers no show more when the runs fit", async () => {
    vi.mocked(library.listAgentRuns).mockResolvedValue([
      { kind: "task", conversationId: "c1", task: "only", repo: "alpha", state: "ended", model: "claude-haiku", tokens: 1, createdAt: "2026-11-09T10:00:00Z", endedAt: null },
    ]);
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const runs = within(await screen.findByRole("region", { name: "recent runs" }));
    await runs.findByRole("row", { name: /only/ });
    expect(runs.queryByRole("button", { name: /show/ })).not.toBeInTheDocument();
  });

  it("says when a type has no runs yet", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    expect(await screen.findByText("no runs yet")).toBeInTheDocument();
  });

  it("keeps the type's fields when its runs do not load", async () => {
    vi.mocked(library.listAgentRuns).mockRejectedValue(new ApiError("network", "no runs"));
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    expect(await screen.findByText(/runs not loaded: no runs/)).toBeInTheDocument();
    const detail = within(screen.getByRole("region", { name: "planner" }));
    expect(detail.getByText("env")).toBeInTheDocument();
    expect(detail.getByText(/model = "opus"/)).toBeInTheDocument();
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

  it("offers no edit or delete of the type itself, only a save for its avatar", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    await screen.findByRole("region", { name: "planner" });
    expect(screen.queryByRole("button", { name: /^(edit(?! spec)|delete|remove)/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^save(?! avatar)/i })).not.toBeInTheDocument();
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

  it("shows a compact run count on each row and no token counts anywhere in the list", async () => {
    render(<AgentsPage />);
    const builderRow = within(await rowOf("builder"));
    expect(await builderRow.findByText("3 runs")).toBeInTheDocument();
    expect(within(await rowOf("planner")).getByText("1 run")).toBeInTheDocument();
    expect(within(await rowOf("dev-server")).getByText("0 runs")).toBeInTheDocument();
    expect(screen.queryByText(/tokens/)).not.toBeInTheDocument();
  });

  it("shows usage in the detail as a graph for the selected type", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("builder"));
    const detail = within(await screen.findByRole("region", { name: "builder" }));
    expect(await detail.findByRole("group", { name: "usage range" })).toBeInTheDocument();
    expect(await detail.findByRole("group", { name: "tokens over time" })).toBeInTheDocument();
    expect(detail.getByRole("button", { name: "runs" })).toBeInTheDocument();
    expect(library.getAgentUsageSeries).toHaveBeenCalledWith("builder", "7d", expect.anything());
    expect(detail.queryByText("12.4K")).not.toBeInTheDocument();
  });

  it("lists the default first, then the busiest types", async () => {
    vi.spyOn(library, "listAgentTypes").mockResolvedValue([shellType, planner, builder]);
    render(<AgentsPage />);
    await rowOf("builder");
    const names = screen.getAllByRole("button", { name: /builder|planner|dev-server/ }).filter((b) => b.classList.contains("th-agent-row"));
    expect(names.map((b) => b.querySelector("b")?.textContent)).toEqual(["builder", "planner", "dev-server"]);
  });

  it("shows an avatar on every row", async () => {
    render(<AgentsPage />);
    for (const name of ["builder", "planner", "dev-server"]) {
      expect((await rowOf(name)).querySelector("svg.th-avatar")).not.toBeNull();
    }
  });

  it("filters the list as you type and says when nothing matches", async () => {
    render(<AgentsPage />);
    await rowOf("builder");
    const filter = screen.getByRole("searchbox", { name: "filter agent types" });
    await userEvent.type(filter, "plans");
    expect(screen.getByRole("button", { name: /planner/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /builder/ })).not.toBeInTheDocument();
    await userEvent.clear(filter);
    await userEvent.type(filter, "zzz");
    expect(screen.getByText('no agent types match "zzz"')).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(filter).toHaveValue("");
    expect(await rowOf("builder")).toBeInTheDocument();
  });

  it("moves the selection with the arrow keys from a row", async () => {
    render(<AgentsPage />);
    const first = await rowOf("builder");
    first.focus();
    await userEvent.keyboard("{ArrowDown}");
    expect(await rowOf("planner")).toHaveAttribute("aria-pressed", "true");
    expect(await rowOf("planner")).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    expect(await rowOf("dev-server")).toHaveAttribute("aria-pressed", "true");
    await userEvent.keyboard("{ArrowUp}");
    expect(await rowOf("planner")).toHaveAttribute("aria-pressed", "true");
  });

  it("moves from the filter box into the list with the down arrow", async () => {
    render(<AgentsPage />);
    await rowOf("builder");
    await userEvent.click(screen.getByRole("searchbox", { name: "filter agent types" }));
    await userEvent.keyboard("{ArrowDown}");
    expect(await rowOf("builder")).toHaveFocus();
  });

  it("shows tools and hand-back targets as chips and keeps the TOML collapsible", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    expect(await detail.findByText("Read", { selector: ".th-tag" })).toBeInTheDocument();
    expect(detail.getByText("Grep", { selector: ".th-tag" })).toBeInTheDocument();
    const toggle = detail.getByText("toml");
    const fold = toggle.closest("details");
    expect(fold).not.toBeNull();
    expect(fold).not.toHaveAttribute("open");
    await userEvent.click(toggle);
    expect(fold).toHaveAttribute("open");
    expect(detail.getByRole("button", { name: "copy" })).toBeInTheDocument();
  });

  it("draws the avatar a type's TOML defines, in the row and the detail", async () => {
    vi.spyOn(library, "listAgentTypes").mockResolvedValue([builder, { ...planner, avatar: { color: "fg-muted" } }]);
    vi.spyOn(library, "getAgentType").mockResolvedValue({ ...plannerFull, avatar: { color: "fg-muted" } });
    render(<AgentsPage />);
    expect((await rowOf("planner")).querySelector("svg")).toHaveClass("th-avatar--fg-muted");
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    expect(await detail.findByRole("img", { name: "planner avatar preview" })).toHaveClass("th-avatar--fg-muted");
    await userEvent.click(detail.getByRole("button", { name: "edit spec" }));
    expect(detail.getByRole("textbox", { name: "avatar spec" })).toHaveValue('[avatar]\ncolor = "fg-muted"\n');
  });

  it("puts the avatar editor at the top of the detail, before the setup and usage sections", async () => {
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    const editor = await detail.findByRole("img", { name: "planner avatar preview" });
    for (const name of ["setup", "usage", "recent runs"]) {
      const section = await detail.findByRole("region", { name });
      expect(editor.compareDocumentPosition(section) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    }
  });

  it("saves an avatar through the type's TOML and shows it in the list row", async () => {
    const saved = { ...plannerFull, avatar: { color: "fg-muted" as const }, source: `${plannerFull.source}\n[avatar]\ncolor = "fg-muted"\n` };
    vi.spyOn(library, "saveAgentType").mockResolvedValue(saved);
    render(<AgentsPage />);
    await userEvent.click(await rowOf("planner"));
    const detail = within(await screen.findByRole("region", { name: "planner" }));
    await userEvent.click(await detail.findByRole("button", { name: "edit spec" }));
    const box = await detail.findByRole("textbox", { name: "avatar spec" });
    await userEvent.click(box);
    await userEvent.paste('[avatar]\ncolor = "fg-muted"');
    vi.mocked(library.listAgentTypes).mockResolvedValue([builder, { ...planner, avatar: { color: "fg-muted" } }, shellType]);
    vi.mocked(library.getAgentType).mockResolvedValue(saved);
    await userEvent.click(detail.getByRole("button", { name: "save avatar" }));
    expect(library.saveAgentType).toHaveBeenCalledWith("planner", saved.source);
    await waitFor(async () => expect((await rowOf("planner")).querySelector("svg")).toHaveClass("th-avatar--fg-muted"));
    await waitFor(() => expect(detail.queryByRole("textbox", { name: "avatar spec" })).not.toBeInTheDocument());
    await userEvent.click(detail.getByRole("button", { name: "edit spec" }));
    expect(detail.getByRole("textbox", { name: "avatar spec" })).toHaveValue('[avatar]\ncolor = "fg-muted"\n');
  });

  it("removes the browser-only avatar choices an older version stored", async () => {
    localStorage.setItem("th-agent-avatars", "{}");
    render(<AgentsPage />);
    await rowOf("builder");
    expect(localStorage.getItem("th-agent-avatars")).toBeNull();
  });

  it("still lists the types when usage fails to load", async () => {
    vi.spyOn(library, "listAgentUsage").mockRejectedValue(new ApiError("network", "no usage"));
    render(<AgentsPage />);
    expect(await rowOf("builder")).toBeInTheDocument();
    expect(within(await rowOf("builder")).queryByText(/runs?$/)).not.toBeInTheDocument();
  });
});
