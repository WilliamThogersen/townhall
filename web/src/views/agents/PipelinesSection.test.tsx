import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { aPipeline } from "../../api/fixtures";
import * as pipelines from "../../api/pipelines";
import type { DefinitionError } from "../../api/types";
import { PipelinesSection } from "./PipelinesSection";

const standard = aPipeline({
  name: "standard",
  description: "plan, build, test",
  default: true,
  source: 'name = "standard"\ndescription = "plan, build, test"\n',
});
const shipped = aPipeline({
  name: "flows-ship",
  description: "from a plugin",
  origin: "plugin",
  plugin: "flows@acme",
  source: 'name = "ship"\n',
});

const refusal = (errors: readonly DefinitionError[]) => new ApiError("http", "refused", 400, errors);
const problem = (over: Partial<DefinitionError>): DefinitionError => ({
  kind: "workflow",
  name: "standard",
  step: null,
  field: null,
  message: "bad",
  line: null,
  ...over,
});

beforeEach(() => {
  vi.spyOn(pipelines, "listPipelines").mockResolvedValue([standard, shipped]);
  vi.spyOn(pipelines, "getPipeline").mockImplementation((name) =>
    Promise.resolve([standard, shipped].find((p) => p.name === name) ?? standard),
  );
});

afterEach(() => vi.restoreAllMocks());

const open = async (name: string) => {
  const user = userEvent.setup();
  render(<PipelinesSection />);
  await user.click(await screen.findByRole("button", { name: new RegExp(name) }));
  return user;
};

describe("PipelinesSection", () => {
  it("lists pipelines with description, step count and the default tag", async () => {
    render(<PipelinesSection />);
    const row = within(await screen.findByRole("button", { name: /standard/ }));
    expect(row.getByText("plan, build, test")).toBeInTheDocument();
    expect(row.getByText("2 steps")).toBeInTheDocument();
    expect(row.getByText("default")).toBeInTheDocument();
  });

  it("badges a plugin pipeline the way plugin agent types are", async () => {
    await open("flows-ship");
    const detail = within(await screen.findByRole("region", { name: "flows-ship" }));
    expect(detail.getByText("plugin flows@acme")).toHaveAttribute("title", "this pipeline comes from a plugin and is read only");
    expect(within(screen.getByRole("button", { name: /flows-ship/ })).getByText("plugin flows@acme")).toBeInTheDocument();
  });

  it("shows a plugin pipeline read only, with no save or delete", async () => {
    await open("flows-ship");
    const editor = await screen.findByRole("textbox", { name: "pipeline toml" });
    expect(editor).toHaveAttribute("readonly");
    expect(editor).toHaveValue('name = "ship"\n');
    expect(screen.queryByRole("button", { name: "save" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "delete" })).not.toBeInTheDocument();
  });

  it("saves an edited pipeline under its own name and reloads the list", async () => {
    const save = vi.spyOn(pipelines, "savePipeline").mockResolvedValue(standard);
    const user = await open("standard");
    const editor = await screen.findByRole("textbox", { name: "pipeline toml" });
    expect(screen.getByRole("button", { name: "save" })).toBeDisabled();
    await user.type(editor, "x");
    await user.click(screen.getByRole("button", { name: "save" }));
    expect(save).toHaveBeenCalledWith("standard", `${standard.source}x`);
    await waitFor(() => expect(pipelines.listPipelines).toHaveBeenCalledTimes(2));
  });

  it("shows each field-level error of a refused save", async () => {
    vi.spyOn(pipelines, "savePipeline").mockRejectedValue(
      refusal([
        problem({ step: "build", field: "agent", message: 'unknown agent "ghost"' }),
        problem({ field: "work", message: 'no step "ship"' }),
        problem({ message: "unexpected token", line: 4 }),
      ]),
    );
    const user = await open("standard");
    await user.type(await screen.findByRole("textbox", { name: "pipeline toml" }), "x");
    await user.click(screen.getByRole("button", { name: "save" }));
    const errors = within(await screen.findByRole("list", { name: "pipeline errors" }));
    expect(errors.getAllByRole("listitem").map((li) => li.textContent)).toEqual([
      'step build · agent: unknown agent "ghost"',
      'work: no step "ship"',
      "line 4: unexpected token",
    ]);
    expect(screen.getByRole("textbox", { name: "pipeline toml" })).toHaveAttribute("aria-invalid", "true");
  });

  it("shows a refusal that has no field errors as a banner", async () => {
    vi.spyOn(pipelines, "savePipeline").mockRejectedValue(new ApiError("http", "cannot reach the daemon", null));
    const user = await open("standard");
    await user.type(await screen.findByRole("textbox", { name: "pipeline toml" }), "x");
    await user.click(screen.getByRole("button", { name: "save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("cannot reach the daemon");
  });

  it("clears the errors when the text changes", async () => {
    vi.spyOn(pipelines, "savePipeline").mockRejectedValue(refusal([problem({ field: "work", message: "bad" })]));
    const user = await open("standard");
    const editor = await screen.findByRole("textbox", { name: "pipeline toml" });
    await user.type(editor, "x");
    await user.click(screen.getByRole("button", { name: "save" }));
    await screen.findByRole("list", { name: "pipeline errors" });
    await user.type(editor, "y");
    expect(screen.queryByRole("list", { name: "pipeline errors" })).not.toBeInTheDocument();
  });

  it("creates a pipeline from a template and opens it after the save", async () => {
    const save = vi.spyOn(pipelines, "savePipeline").mockResolvedValue(aPipeline({ name: "mine" }));
    const user = userEvent.setup();
    render(<PipelinesSection />);
    await user.click(await screen.findByRole("button", { name: "new pipeline" }));
    const editor = await screen.findByRole("textbox", { name: "pipeline toml" });
    await user.clear(editor);
    await user.click(editor);
    await user.paste('name = "mine"\n');
    await user.click(screen.getByRole("button", { name: "save" }));
    expect(save).toHaveBeenCalledWith("mine", 'name = "mine"\n');
    await waitFor(() => expect(pipelines.listPipelines).toHaveBeenCalledTimes(2));
  });

  it("starts a new pipeline from a template that names a step and an agent", async () => {
    const user = userEvent.setup();
    render(<PipelinesSection />);
    await user.click(await screen.findByRole("button", { name: "new pipeline" }));
    const editor = await screen.findByRole("textbox", { name: "pipeline toml" });
    expect((editor as HTMLTextAreaElement).value).toMatch(/name = "[^"]+"[\s\S]*\[\[step\]\]/);
  });

  it("refuses to create under a name that is taken, without calling the daemon", async () => {
    const save = vi.spyOn(pipelines, "savePipeline").mockResolvedValue(standard);
    const user = userEvent.setup();
    render(<PipelinesSection />);
    await user.click(await screen.findByRole("button", { name: "new pipeline" }));
    const editor = await screen.findByRole("textbox", { name: "pipeline toml" });
    await user.clear(editor);
    await user.click(editor);
    await user.paste('name = "standard"\n');
    await user.click(screen.getByRole("button", { name: "save" }));
    expect(save).not.toHaveBeenCalled();
    expect(await screen.findByRole("list", { name: "pipeline errors" })).toHaveTextContent(
      'name: a pipeline called "standard" exists: open it to edit',
    );
  });

  it("asks for a name when the text has none, without calling the daemon", async () => {
    const save = vi.spyOn(pipelines, "savePipeline").mockResolvedValue(standard);
    const user = userEvent.setup();
    render(<PipelinesSection />);
    await user.click(await screen.findByRole("button", { name: "new pipeline" }));
    const editor = await screen.findByRole("textbox", { name: "pipeline toml" });
    await user.clear(editor);
    await user.click(editor);
    await user.paste("description = 'x'\n");
    await user.click(screen.getByRole("button", { name: "save" }));
    expect(save).not.toHaveBeenCalled();
    expect(await screen.findByRole("list", { name: "pipeline errors" })).toHaveTextContent('name: add name = "…" on its own line');
  });

  it("deletes after a confirm that names the pipeline", async () => {
    const remove = vi.spyOn(pipelines, "deletePipeline").mockResolvedValue("standard");
    const user = await open("standard");
    await user.click(await screen.findByRole("button", { name: "delete" }));
    expect(remove).not.toHaveBeenCalled();
    expect(screen.getByText(/delete standard\?/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "cancel" }));
    expect(remove).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "delete" }));
    await user.click(screen.getByRole("button", { name: "confirm delete" }));
    expect(remove).toHaveBeenCalledWith("standard");
    await waitFor(() => expect(pipelines.listPipelines).toHaveBeenCalledTimes(2));
  });

  it("says so when there are no pipelines", async () => {
    vi.spyOn(pipelines, "listPipelines").mockResolvedValue([]);
    render(<PipelinesSection />);
    expect(await screen.findByText("no pipelines yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "new pipeline" })).toBeInTheDocument();
  });

  it("shows a load failure", async () => {
    vi.spyOn(pipelines, "listPipelines").mockRejectedValue(new ApiError("http", "boom", 500));
    render(<PipelinesSection />);
    expect(await screen.findByRole("alert")).toHaveTextContent("pipelines not loaded: boom");
  });
});
