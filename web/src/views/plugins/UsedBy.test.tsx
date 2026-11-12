import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { anAgentType, anAgentTypeSummary, aPlugin } from "../../api/fixtures";
import * as library from "../../api/library";
import { UsedBy } from "./UsedBy";

const WEB = aPlugin({
  id: "modern-web-guidance@claude-plugins-official",
  name: "modern-web-guidance",
  marketplace: "claude-plugins-official",
});

const CODER = anAgentTypeSummary({ name: "coder", plugins: [WEB.id] });
const TESTER = anAgentTypeSummary({ name: "tester", plugins: [] });
const CHAT = anAgentTypeSummary({ name: "default" });

function stub(types = [CODER, TESTER, CHAT]) {
  vi.spyOn(library, "listAgentTypes").mockResolvedValue(types);
  vi.spyOn(library, "getAgentType").mockImplementation((name) =>
    Promise.resolve(
      anAgentType({
        name,
        source: `name = "${name}"\n${name === "coder" ? `plugins = ["${WEB.id}"]\n` : ""}`,
        plugins: name === "coder" ? [WEB.id] : [],
      }),
    ),
  );
}

async function open() {
  render(<UsedBy plugin={WEB} />);
  await userEvent.click(await screen.findByText(/used by/));
}

afterEach(() => vi.restoreAllMocks());

describe("UsedBy", () => {
  it("counts the types that use the plugin and lists every type but townhall's own session", async () => {
    stub();
    await open();
    expect(screen.getByText("used by 1 agent type")).toBeInTheDocument();
    expect(await screen.findByRole("checkbox", { name: "coder" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "tester" })).not.toBeChecked();
    expect(screen.queryByRole("checkbox", { name: "default" })).toBeNull();
  });

  it("adds the plugin to a type by saving its toml", async () => {
    stub();
    const save = vi.spyOn(library, "saveAgentType").mockResolvedValue(anAgentType());
    await open();
    await userEvent.click(await screen.findByRole("checkbox", { name: "tester" }));
    expect(save).toHaveBeenCalledWith("tester", `name = "tester"\nplugins = ["${WEB.id}"]\n`);
    await vi.waitFor(() => expect(library.listAgentTypes).toHaveBeenCalledTimes(2));
  });

  it("removes the plugin from a type", async () => {
    stub();
    const save = vi.spyOn(library, "saveAgentType").mockResolvedValue(anAgentType());
    await open();
    await userEvent.click(await screen.findByRole("checkbox", { name: "coder" }));
    expect(save).toHaveBeenCalledWith("coder", 'name = "coder"\n');
  });

  it("shows the daemon's reason when a type cannot be saved", async () => {
    stub();
    vi.spyOn(library, "saveAgentType").mockRejectedValue(new ApiError("http", "owned by plugin x@y", 409));
    await open();
    await userEvent.click(await screen.findByRole("checkbox", { name: "tester" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("owned by plugin x@y");
  });

  it("says when no type uses the plugin", async () => {
    stub([TESTER]);
    render(<UsedBy plugin={WEB} />);
    expect(await screen.findByText("used by no agent type")).toBeInTheDocument();
  });
});
