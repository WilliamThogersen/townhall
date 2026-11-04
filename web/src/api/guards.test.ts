import { describe, expect, it } from "vitest";
import { anAgentInfo } from "./fixtures";
import { parseAgentInfo, parseAgentState, parseEnvelope, parseInfo, parseRepo } from "./guards";

describe("guards", () => {
  it("reads the envelope with defaults for missing data and error", () => {
    expect(parseEnvelope({ success: true })).toEqual({ success: true, data: null, error: null });
    expect(() => parseEnvelope([])).toThrow("envelope is not an object");
    expect(() => parseEnvelope({ success: "yes" })).toThrow("envelope.success");
    expect(() => parseEnvelope({ success: false, error: 3 })).toThrow("envelope.error");
  });

  it.each([
    [{ kind: "working" }],
    [{ kind: "waiting", question: "run migrations?" }],
    [{ kind: "error", exitCode: 130 }],
    [{ kind: "error", exitCode: null }],
    [{ kind: "ended" }],
  ])("reads the agent state %j", (state) => {
    expect(parseAgentState(state)).toEqual(state);
  });

  it("refuses broken agent states", () => {
    expect(() => parseAgentState({ kind: "sleeping" })).toThrow("agent.state.kind");
    expect(() => parseAgentState({ kind: "waiting" })).toThrow("agent.state.question");
    expect(() => parseAgentState({ kind: "error", exitCode: "1" })).toThrow("agent.state.exitCode");
  });

  it("reads an agent with its repo and state", () => {
    expect(parseAgentInfo(anAgentInfo())).toEqual(anAgentInfo());
    expect(parseAgentInfo({ ...anAgentInfo(), kind: undefined }).kind).toBeNull();
    expect(() => parseAgentInfo({ ...anAgentInfo(), repo: null })).toThrow("agent.repo");
  });

  it("reads info and repos", () => {
    expect(parseInfo({ home: "/h", repo: "/r", repoKey: "k", userName: "Ada" }).userName).toBe("Ada");
    expect(() => parseInfo({ home: "/h", repo: "/r" })).toThrow("info.repoKey");
    expect(parseRepo({ name: "shop", path: "/s", baseBranch: "main" }).baseBranch).toBe("main");
    expect(() => parseRepo("shop")).toThrow("repo is not an object");
  });
});
