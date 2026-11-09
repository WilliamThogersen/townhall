import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AVATAR_PARTS, defaultAvatar } from "./avatar";
import { AVATAR_PREFS_KEY, avatarFor } from "./avatarPrefs";
import { AvatarEditor } from "./AvatarEditor";

vi.mock("../../log/report", () => ({ logWarn: vi.fn() }));

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("AvatarEditor", () => {
  it("shows a labelled live preview and one row per part with its current option", () => {
    render(<AvatarEditor name="planner" />);
    expect(screen.getByRole("img", { name: "planner avatar preview" })).toBeInTheDocument();
    const current = defaultAvatar("planner");
    for (const part of Object.keys(AVATAR_PARTS) as (keyof typeof AVATAR_PARTS)[]) {
      expect(screen.getByRole("group", { name: part })).toHaveTextContent(current[part]);
    }
  });

  it("cycles a part forward and back, saving each step", async () => {
    render(<AvatarEditor name="planner" />);
    const start = defaultAvatar("planner");
    await userEvent.click(screen.getByRole("button", { name: "next eyes" }));
    const forward = avatarFor("planner");
    expect(forward.eyes).not.toBe(start.eyes);
    expect({ ...forward, eyes: start.eyes }).toEqual(start);
    expect(screen.getByRole("group", { name: "eyes" })).toHaveTextContent(forward.eyes);
    await userEvent.click(screen.getByRole("button", { name: "previous eyes" }));
    expect(avatarFor("planner")).toEqual(start);
  });

  it("cycles the color too", async () => {
    render(<AvatarEditor name="planner" />);
    const before = avatarFor("planner").color;
    await userEvent.click(screen.getByRole("button", { name: "next color" }));
    expect(avatarFor("planner").color).not.toBe(before);
    expect(screen.getByRole("img", { name: "planner avatar preview" })).toHaveClass(`th-avatar--${avatarFor("planner").color}`);
  });

  it("resets to the default only once something was customised", async () => {
    render(<AvatarEditor name="planner" />);
    expect(screen.getByRole("button", { name: "reset" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "next mouth" }));
    expect(localStorage.getItem(AVATAR_PREFS_KEY)).not.toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "reset" }));
    expect(avatarFor("planner")).toEqual(defaultAvatar("planner"));
    expect(screen.getByRole("button", { name: "reset" })).toBeDisabled();
  });

  it("randomizes into a valid avatar", async () => {
    render(<AvatarEditor name="planner" />);
    await userEvent.click(screen.getByRole("button", { name: "shuffle" }));
    const next = avatarFor("planner");
    for (const part of Object.keys(AVATAR_PARTS) as (keyof typeof AVATAR_PARTS)[]) {
      expect(AVATAR_PARTS[part]).toContain(next[part]);
    }
  });

  it("says so when the browser refuses to store the choice", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied");
    });
    render(<AvatarEditor name="planner" />);
    await userEvent.click(screen.getByRole("button", { name: "next eyes" }));
    expect(await screen.findByRole("status")).toHaveTextContent("not saved");
  });
});
