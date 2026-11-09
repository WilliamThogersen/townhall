import { describe, expect, it } from "vitest";
import { aConversation, anAgentInfo, aRepo, aTask } from "../api/fixtures";
import {
  agentGlyph,
  agentsLabel,
  conversationCounts,
  conversationReposOf,
  MAX_AGENT_GLYPHS,
  summarizeRepos,
} from "./repoSummary";

const HOME = "/Users/me";

describe("agentGlyph", () => {
  it("maps the daemon state to a glyph", () => {
    expect(agentGlyph(anAgentInfo({ state: { kind: "working" } }))).toBe("working");
    expect(agentGlyph(anAgentInfo({ state: { kind: "waiting", question: "ok?" } }))).toBe("waiting");
    expect(agentGlyph(anAgentInfo({ state: { kind: "error", exitCode: 1 } }))).toBe("error");
  });

  it("tells an idle live agent from a dead one", () => {
    expect(agentGlyph(anAgentInfo({ alive: true, state: { kind: "ended" } }))).toBe("alive");
    expect(agentGlyph(anAgentInfo({ alive: false, state: { kind: "ended" } }))).toBe("dead");
  });
});

describe("agentsLabel", () => {
  it("counts agents in words", () => {
    expect(agentsLabel(0)).toBe("no agents");
    expect(agentsLabel(1)).toBe("1 agent");
    expect(agentsLabel(3)).toBe("3 agents");
  });
});

describe("conversationReposOf", () => {
  const tasks = [
    aTask({ name: "a", conversationId: "c1", repo: "shop" }),
    aTask({ name: "b", conversationId: "c1", repo: "shop" }),
    aTask({ name: "c", conversationId: "c1", repo: "townhall" }),
    aTask({ name: "d", conversationId: "c2", repo: "shop" }),
    aTask({ name: "e", conversationId: "c2", repo: "data-hub" }),
  ];

  it("is the linked repo when one is set", () => {
    expect(conversationReposOf(aConversation({ id: "c1", linkedRepo: "data-hub" }), tasks)).toEqual(["data-hub"]);
  });

  it("is the most common repo among its tasks", () => {
    expect(conversationReposOf(aConversation({ id: "c1" }), tasks)).toEqual(["shop"]);
  });

  it("is every tied repo on a tie", () => {
    expect(conversationReposOf(aConversation({ id: "c2" }), tasks)).toEqual(["shop", "data-hub"]);
  });

  it("is none without a link or tasks", () => {
    expect(conversationReposOf(aConversation({ id: "c3" }), tasks)).toEqual([]);
  });

  it("matches repo names ignoring case", () => {
    const mixed = [aTask({ name: "a", conversationId: "c1", repo: "Shop" }), aTask({ name: "b", conversationId: "c1", repo: "shop" })];
    expect(conversationReposOf(aConversation({ id: "c1" }), mixed)).toEqual(["Shop"]);
  });
});

describe("conversationCounts", () => {
  it("counts each conversation once for each of its repos, ignoring case", () => {
    const conversations = [
      aConversation({ id: "c1", linkedRepo: "Shop" }),
      aConversation({ id: "c2" }),
      aConversation({ id: "c3" }),
      aConversation({ id: "c4", ended: true }),
    ];
    const tasks = [
      aTask({ name: "a", conversationId: "c2", repo: "shop" }),
      aTask({ name: "b", conversationId: "c2", repo: "townhall" }),
      aTask({ name: "c", conversationId: "c4", repo: "townhall" }),
    ];
    const counts = conversationCounts(conversations, tasks);
    expect(counts.get("shop")).toBe(2);
    expect(counts.get("townhall")).toBe(2);
    expect(counts.get("data-hub")).toBeUndefined();
  });
});

describe("summarizeRepos", () => {
  const repos = [
    aRepo({ name: "townhall", path: "/Users/me/Developer/townhall", baseBranch: "main" }),
    aRepo({ name: "shop", path: "/Users/me/Developer/acme/work/clients/shop", baseBranch: "develop" }),
  ];

  it("gives each repo its short path, counts and glyphs", () => {
    const [townhall, shop] = summarizeRepos(repos, {
      home: HOME,
      conversations: [aConversation({ id: "c1", linkedRepo: "shop" })],
      tasks: [],
      agents: [
        anAgentInfo({ name: "a", repo: "shop", alive: true, state: { kind: "ended" } }),
        anAgentInfo({ name: "b", repo: "SHOP", state: { kind: "waiting", question: "?" } }),
        anAgentInfo({ name: "c", repo: "townhall", state: { kind: "working" } }),
      ],
    });
    expect(townhall).toMatchObject({
      repo: repos[0],
      shortPath: "~/Developer/townhall",
      conversations: "no conversations",
      glyphs: ["working"],
      agents: "1 agent",
      spokenAgents: "1 working",
    });
    expect(shop).toMatchObject({
      shortPath: "~/Developer/…/clients/shop",
      conversations: "1 conversation",
      glyphs: ["waiting", "alive"],
      agents: "2 agents",
      spokenAgents: "1 waiting, 1 running",
    });
  });

  it("shows the most urgent glyphs first and caps them", () => {
    const agents = [
      ...Array.from({ length: 6 }, (_, i) => anAgentInfo({ name: `idle-${i}`, repo: "shop", state: { kind: "ended" } })),
      anAgentInfo({ name: "broken", repo: "shop", state: { kind: "error", exitCode: 1 } }),
    ];
    const [, shop] = summarizeRepos(repos, { home: HOME, conversations: [], tasks: [], agents });
    expect(shop?.glyphs).toHaveLength(MAX_AGENT_GLYPHS);
    expect(shop?.glyphs[0]).toBe("error");
    expect(shop?.agents).toBe("7 agents");
  });

  it("says no agents for a repo without any", () => {
    const [townhall] = summarizeRepos(repos, { home: null, conversations: [], tasks: [], agents: [] });
    expect(townhall).toMatchObject({ glyphs: [], agents: "no agents", spokenAgents: "", shortPath: "/Users/me/Developer/townhall" });
  });
});
