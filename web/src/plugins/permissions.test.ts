import { describe, expect, it } from "vitest";
import { describePermission, groupPermissions } from "./permissions";

describe("groupPermissions", () => {
  it("groups the list as the design asks the user to approve it, in a fixed order", () => {
    const groups = groupPermissions([
      "env:GITHUB_TOKEN",
      "ui:sidebar",
      "api:tasks:read",
      "claude.api:tasks:create",
      "claude.hooks:PreToolUse",
      "claude.mcp:db:*",
      "claude.planner",
      "prompt:brief",
      "hook:taskEnded",
      "runs.hook:taskEnded:./notify.sh",
      "runs.mcp:db:node server.js",
      "connects.mcp:web:https://example.com/mcp",
    ]);
    expect(groups.map((g) => g.id)).toEqual(["runs", "agents", "api", "ui", "env"]);
    expect(groups.map((g) => g.title)).toEqual([
      "runs programs on this machine, as you",
      "changes what agents do",
      "reads or writes townhall data",
      "shows UI in these places",
      "reads these environment variables",
    ]);
    expect(groups[0]?.items.map((i) => i.raw)).toEqual([
      "hook:taskEnded",
      "runs.hook:taskEnded:./notify.sh",
      "runs.mcp:db:node server.js",
      "connects.mcp:web:https://example.com/mcp",
    ]);
    expect(groups[1]?.items.map((i) => i.raw)).toEqual([
      "claude.hooks:PreToolUse",
      "claude.mcp:db:*",
      "claude.planner",
      "prompt:brief",
    ]);
    expect(groups[2]?.items.map((i) => i.raw)).toEqual(["api:tasks:read", "claude.api:tasks:create"]);
  });

  it("leaves out groups with nothing in them", () => {
    expect(groupPermissions(["ui:sidebar"]).map((g) => g.id)).toEqual(["ui"]);
    expect(groupPermissions([])).toEqual([]);
  });

  it("keeps an item it does not know in an other group instead of hiding it", () => {
    const groups = groupPermissions(["future:thing"]);
    expect(groups).toHaveLength(1);
    expect(groups[0]).toMatchObject({ id: "other", items: [{ raw: "future:thing" }] });
  });
});

describe("describePermission", () => {
  it("says what each kind does, flagging PreToolUse and agent-held api scopes", () => {
    expect(describePermission("claude.hooks:PreToolUse")).toMatchObject({ flag: "can block or change any tool an agent calls" });
    expect(describePermission("claude.hooks:Stop").flag).toBeNull();
    expect(describePermission("claude.api:tasks:create")).toMatchObject({
      text: "tasks:create, for agents",
      flag: "an agent that is prompt injected could use this",
    });
    expect(describePermission("runs.hook:taskEnded:./notify.sh").text).toBe("hook taskEnded runs ./notify.sh");
    expect(describePermission("runs.mcp:db:node server.js").text).toBe("mcp server db runs node server.js");
    expect(describePermission("connects.mcp:web:https://x/mcp").text).toBe("mcp server web connects to https://x/mcp");
    expect(describePermission("claude.mcp:db:*").text).toBe("mcp server db, all tools");
    expect(describePermission("claude.mcp.read-only:db:query").text).toBe("mcp server db, tool query (read only)");
    expect(describePermission("claude.hooks.agent-type:tester").text).toBe("hooks for agent type tester");
    expect(describePermission("api:tasks:read").text).toBe("tasks:read");
    expect(describePermission("env:HOME").text).toBe("HOME");
    expect(describePermission("ui:sidebar").text).toBe("sidebar");
    expect(describePermission("prompt:brief").text).toBe("adds to the brief");
    expect(describePermission("claude.planner").text).toBe("changes townhall's planner");
    expect(describePermission("hook:taskEnded").text).toBe("hook on taskEnded");
  });
});
