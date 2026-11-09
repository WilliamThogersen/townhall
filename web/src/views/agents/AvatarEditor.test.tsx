import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { anAgentType } from "../../api/fixtures";
import * as library from "../../api/library";
import { AVATAR_PARTS, defaultAvatar } from "./avatar";
import { AvatarEditor } from "./AvatarEditor";
import type { AvatarSpec } from "./avatarSpec";

const SOURCE = 'name = "planner"\nadapter = "claude"\n';

const setup = (saved: AvatarSpec | null = null, source = SOURCE) => {
  const onSaved = vi.fn();
  render(<AvatarEditor name="planner" source={source} saved={saved} onSaved={onSaved} />);
  return { onSaved };
};

const box = () => screen.getByRole("textbox", { name: "avatar spec" }) as HTMLTextAreaElement;
const openSpec = () => userEvent.click(screen.getByRole("button", { name: "edit spec" }));
const preview = () => screen.getByRole("img", { name: "planner avatar preview" });

beforeEach(() => {
  vi.spyOn(library, "saveAgentType").mockImplementation((name, source) => Promise.resolve(anAgentType({ name, source })));
});

afterEach(() => vi.restoreAllMocks());

describe("AvatarEditor", () => {
  it("previews the name default and lists the part pickers without opening the spec text", () => {
    setup();
    expect(screen.queryByRole("textbox", { name: "avatar spec" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "edit spec" })).toHaveAttribute("aria-expanded", "false");
    expect(preview()).toHaveClass(`th-avatar--${defaultAvatar("planner").color}`);
    for (const part of Object.keys(AVATAR_PARTS) as (keyof typeof AVATAR_PARTS)[]) {
      expect(screen.getByRole("group", { name: part })).toHaveTextContent(defaultAvatar("planner")[part]);
    }
  });

  it("opens and closes the spec text from the edit spec toggle", async () => {
    setup();
    await openSpec();
    expect(box()).toHaveValue("");
    expect(screen.getByRole("button", { name: "edit spec" })).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(screen.getByRole("button", { name: "edit spec" }));
    expect(screen.queryByRole("textbox", { name: "avatar spec" })).not.toBeInTheDocument();
  });

  it("gives the empty spec a neutral hint rather than something that reads like an error", async () => {
    setup();
    await openSpec();
    const hint = box().getAttribute("placeholder") ?? "";
    expect(hint).toMatch(/\[avatar\]/);
    expect(hint).not.toMatch(/\bno \[avatar\] section\b/);
  });

  it("starts with the saved spec as text", async () => {
    setup({ head: "round", color: "info" });
    await openSpec();
    expect(box()).toHaveValue('[avatar]\nhead = "round"\ncolor = "info"\n');
    expect(preview()).toHaveClass("th-avatar--info");
  });

  it("updates the preview live while you type", async () => {
    setup();
    await openSpec();
    await userEvent.click(box());
    await userEvent.paste('[avatar]\ncolor = "fg-muted"');
    expect(preview()).toHaveClass("th-avatar--fg-muted");
  });

  it("lists every problem with its line and keeps the last good preview while the text is wrong", async () => {
    setup({ color: "info" });
    await openSpec();
    await userEvent.clear(box());
    await userEvent.paste('[avatar]\nhat = "x"\nhead = "cube"');
    const errors = screen.getByRole("list", { name: "avatar spec errors" });
    expect(errors).toHaveTextContent("line 2: unknown key `hat`");
    expect(errors).toHaveTextContent("line 3: head `cube` is not one of square, round, domed, tapered");
    expect(preview()).toHaveClass("th-avatar--info");
    expect(screen.getByRole("button", { name: "save avatar" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "next head" })).toBeDisabled();
  });

  it("writes a part pick into the spec", async () => {
    setup();
    await openSpec();
    const start = defaultAvatar("planner");
    await userEvent.click(screen.getByRole("button", { name: "next eyes" }));
    const options: readonly string[] = AVATAR_PARTS.eyes;
    const next = options[(options.indexOf(start.eyes) + 1) % options.length];
    expect(box()).toHaveValue(`[avatar]\neyes = "${next}"\n`);
    await userEvent.click(screen.getByRole("button", { name: "previous eyes" }));
    expect(box()).toHaveValue(`[avatar]\neyes = "${start.eyes}"\n`);
  });

  it("keeps the keys already in the spec when a part is picked", async () => {
    setup({ head: "round", color: "info" });
    await openSpec();
    await userEvent.click(screen.getByRole("button", { name: "next mouth" }));
    expect(box().value).toMatch(/^\[avatar\]\nhead = "round"\nmouth = "\w+"\ncolor = "info"\n$/);
  });

  it("shuffles into a full valid spec", async () => {
    setup();
    await userEvent.click(screen.getByRole("button", { name: "shuffle" }));
    await openSpec();
    const lines = box().value.trim().split("\n");
    expect(lines[0]).toBe("[avatar]");
    expect(lines).toHaveLength(6);
    expect(screen.queryByRole("list", { name: "avatar spec errors" })).not.toBeInTheDocument();
  });

  it("resets to no section, which is the name default", async () => {
    setup({ color: "info" });
    expect(screen.getByRole("button", { name: "reset" })).toBeEnabled();
    await userEvent.click(screen.getByRole("button", { name: "reset" }));
    await openSpec();
    expect(box()).toHaveValue("");
    expect(preview()).toHaveClass(`th-avatar--${defaultAvatar("planner").color}`);
    expect(screen.getByRole("button", { name: "reset" })).toBeDisabled();
  });

  it("only offers save once the text differs from what is saved", async () => {
    setup({ color: "info" });
    expect(screen.getByRole("button", { name: "save avatar" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "next color" }));
    expect(screen.getByRole("button", { name: "save avatar" })).toBeEnabled();
  });

  it("saves the whole TOML with the new [avatar] section through the save path", async () => {
    const { onSaved } = setup();
    await openSpec();
    await userEvent.click(box());
    await userEvent.paste('[avatar]\ncolor = "ok"');
    await userEvent.click(screen.getByRole("button", { name: "save avatar" }));
    const expected = `${SOURCE}\n[avatar]\ncolor = "ok"\n`;
    await waitFor(() => expect(library.saveAgentType).toHaveBeenCalledWith("planner", expected));
    await waitFor(() => expect(onSaved).toHaveBeenCalledTimes(1));
    expect(onSaved.mock.calls[0]?.[0].source).toBe(expected);
  });

  it("removes the section when the spec is cleared", async () => {
    const source = `${SOURCE}\n[avatar]\ncolor = "ok"\n`;
    setup({ color: "ok" }, source);
    await userEvent.click(screen.getByRole("button", { name: "reset" }));
    await userEvent.click(screen.getByRole("button", { name: "save avatar" }));
    await waitFor(() => expect(library.saveAgentType).toHaveBeenCalledWith("planner", SOURCE));
  });

  it("shows the daemon's complaint and stays editable when the save is refused", async () => {
    vi.spyOn(library, "saveAgentType").mockRejectedValue(new ApiError("http", "avatar.grid must have 9 rows, found 1", 400));
    const { onSaved } = setup();
    await userEvent.click(screen.getByRole("button", { name: "next color" }));
    await userEvent.click(screen.getByRole("button", { name: "save avatar" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("avatar.grid must have 9 rows, found 1");
    expect(onSaved).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "save avatar" })).toBeEnabled();
  });

  it("says when a grid overrides the parts", () => {
    const grid = Array.from({ length: 9 }, () => "#########");
    setup({ grid });
    expect(screen.getByText(/grid is set: head, eyes, mouth and accessory are ignored/)).toBeInTheDocument();
  });
});
