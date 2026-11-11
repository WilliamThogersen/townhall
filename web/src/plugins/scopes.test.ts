import { describe, expect, it } from "vitest";
import { eventScopes, resolveCall } from "./scopes";

const refused = (method: string, path: string) => {
  const found = resolveCall(method, path);
  expect(found.ok).toBe(false);
  return found.ok ? "" : found.reason;
};

describe("resolveCall", () => {
  it("maps a plugin api call to its scope and daemon path", () => {
    expect(resolveCall("GET", "/api/plugin/v1/tasks")).toEqual({
      ok: true,
      scope: "tasks:read",
      path: "/plugin/v1/tasks",
    });
    expect(resolveCall("get", "/api/plugin/v1/tasks/one/artifacts?x=1")).toEqual({
      ok: true,
      scope: "tasks:read",
      path: "/plugin/v1/tasks/one/artifacts?x=1",
    });
    expect(resolveCall("POST", "/api/plugin/v1/conversations/c1/messages")).toMatchObject({
      scope: "conversations:write",
    });
    expect(resolveCall("POST", "/api/plugin/v1/conversations/c1/tasks")).toMatchObject({
      scope: "tasks:create",
    });
    expect(resolveCall("PUT", "/api/plugin/v1/library/agents/a")).toMatchObject({ scope: "library:write" });
    expect(resolveCall("GET", "/api/plugin/v1/library/agents/a")).toMatchObject({ scope: "library:read" });
    expect(resolveCall("DELETE", "/api/plugin/v1/storage/k")).toMatchObject({ scope: "storage" });
    expect(resolveCall("GET", "/api/plugin/v1/settings")).toMatchObject({ scope: "settings:read" });
    expect(resolveCall("GET", "/api/plugin/v1/profile")).toMatchObject({ scope: "profile:read" });
    expect(resolveCall("GET", "/api/plugin/v1/search?q=a")).toMatchObject({ scope: "conversations:read" });
  });

  it("needs no scope for info", () => {
    expect(resolveCall("GET", "/api/plugin/v1/info")).toEqual({ ok: true, scope: null, path: "/plugin/v1/info" });
  });

  it("refuses paths outside /api/plugin/v1/", () => {
    expect(refused("GET", "/api/tasks")).toContain("/api/plugin/v1/");
    expect(refused("GET", "/api/settings")).toContain("/api/plugin/v1/");
    expect(refused("GET", "api/plugin/v1/tasks")).toContain("/api/plugin/v1/");
    expect(refused("GET", "https://evil.example/api/plugin/v1/tasks")).toContain("/api/plugin/v1/");
    expect(refused("GET", "//evil.example/api/plugin/v1/tasks")).toContain("/api/plugin/v1/");
    expect(refused("GET", "/api/plugin/v1")).toContain("/api/plugin/v1/");
    expect(refused("GET", "/api/plugin/v10/tasks")).toContain("/api/plugin/v1/");
  });

  it("refuses a path that climbs out or hides a dot segment", () => {
    refused("GET", "/api/plugin/v1/../tasks");
    refused("GET", "/api/plugin/v1/%2e%2e/tasks");
    refused("GET", "/api/plugin/v1/tasks/./one");
    refused("GET", "/api/plugin/v1/tasks//one");
    refused("GET", "/api/plugin/v1/tasks\\one");
    refused("GET", "/api/plugin/v1/tasks/%2fone");
    refused("GET", "/api/plugin/v1/tasks/one\u0000");
    refused("GET", "/api/plugin/v1/tasks#frag");
  });

  it("refuses a route the plugin api does not have, and a method it does not allow there", () => {
    expect(refused("GET", "/api/plugin/v1/repos")).toContain("no such plugin api route");
    refused("DELETE", "/api/plugin/v1/tasks/one");
    refused("PATCH", "/api/plugin/v1/tasks");
    refused("POST", "/api/plugin/v1/tasks");
    expect(refused("TRACE", "/api/plugin/v1/tasks")).toContain("method");
  });
});

describe("eventScopes", () => {
  it("names the scopes a topic needs", () => {
    expect(eventScopes("taskUpdated")).toEqual(["events", "tasks:read"]);
    expect(eventScopes("agentUpdated")).toEqual(["events", "tasks:read"]);
    expect(eventScopes("entryAppended")).toEqual(["events", "conversations:read"]);
    expect(eventScopes("conversationUpdated")).toEqual(["events", "conversations:read"]);
  });

  it("is null for a topic that cannot be subscribed to", () => {
    expect(eventScopes("resync")).toBeNull();
    expect(eventScopes("nope")).toBeNull();
  });
});
