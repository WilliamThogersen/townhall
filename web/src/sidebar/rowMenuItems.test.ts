import { describe, expect, it, vi } from "vitest";
import { aConversation, aRepo } from "../api/fixtures";
import type { Conversation } from "../api/types";
import { rowMenuItems } from "./rowMenuItems";
import type { SidebarRow } from "./sidebarRows";

const row = (over: Partial<Conversation> = {}): SidebarRow => ({
  conversation: aConversation({ id: "c1", ...over }),
  summary: { glyph: "dead", state: null, status: "your turn", needsYou: false },
  repo: { kind: "repo", name: "shop" },
  autoRepo: { kind: "repo", name: "shop" },
});

const repos = [aRepo({ name: "shop", baseBranch: "develop" }), aRepo({ name: "townhall", baseBranch: "main" })];

function setup(over: Partial<Conversation> = {}) {
  const actions = { end: vi.fn(), link: vi.fn() };
  return { items: rowMenuItems(row(over), repos, actions), actions };
}

describe("rowMenuItems", () => {
  it("offers end, repo auto with the inferred repo, and one item per repo", () => {
    const { items } = setup();
    expect(items.map((i) => [i.label, i.detail ?? "", i.checked ?? null])).toEqual([
      ["end conversation", "", null],
      ["repo: auto", "shop", true],
      ["repo: shop", "develop", false],
      ["repo: townhall", "main", false],
    ]);
  });

  it("ends and links through the actions", () => {
    const { items, actions } = setup();
    items[0]?.onSelect();
    items[3]?.onSelect();
    expect(actions.end).toHaveBeenCalledWith("c1");
    expect(actions.link).toHaveBeenCalledWith("c1", "townhall");
  });

  it("checks the linked repo ignoring case and sets auto back", () => {
    const { items, actions } = setup({ linkedRepo: "Shop" });
    expect(items.filter((i) => i.checked).map((i) => i.label)).toEqual(["repo: shop"]);
    items[2]?.onSelect();
    expect(actions.link).not.toHaveBeenCalled();
    items[1]?.onSelect();
    expect(actions.link).toHaveBeenCalledWith("c1", null);
  });

  it("leaves end out of an ended conversation", () => {
    const { items } = setup({ ended: true });
    expect(items[0]?.label).toBe("repo: auto");
  });
});
