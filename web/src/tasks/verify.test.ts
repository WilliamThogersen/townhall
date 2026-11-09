import { describe, expect, it } from "vitest";
import { aTask } from "../api/fixtures";
import type { Task, Verify } from "../api/types";
import { verifyWords } from "./verify";

const checked = (verify: Verify | null, over: Partial<Task> = {}) =>
  aTask({ state: "working", check: "npm test", maxRetries: 2, verify, ...over });

const verify = (phase: Verify["phase"], retries: number, output: string | null = null): Verify => ({
  phase,
  retries,
  output,
});

describe("verifyWords", () => {
  it("says nothing for a task without a check or before its agent first ended", () => {
    expect(verifyWords(aTask({ state: "working" }))).toBeNull();
    expect(verifyWords(checked(null))).toBeNull();
  });

  it("says verifying while the check runs, and which retry it is for", () => {
    expect(verifyWords(checked(verify("verifying", 0)))).toBe("verifying");
    expect(verifyWords(checked(verify("verifying", 1, "1 failed")))).toBe("verifying · retry 1/2");
  });

  it("counts a relaunch as retry n of the most it gets", () => {
    expect(verifyWords(checked(verify("retrying", 1, "boom"), { state: "queued" }))).toBe("retry 1/2");
    expect(verifyWords(checked(verify("retrying", 2, "boom"), { maxRetries: 3 }))).toBe("retry 2/3");
  });

  it("gives the final result: passed once the task ended, failed with the retries used", () => {
    expect(verifyWords(checked(verify("passed", 1), { state: "ended" }))).toBe("check passed");
    expect(verifyWords(checked(verify("failed", 2, "red"), { state: "needsYou" }))).toBe(
      "check failed after 2 retries",
    );
    expect(verifyWords(checked(verify("failed", 1, "red"), { state: "needsYou", maxRetries: 1 }))).toBe(
      "check failed after 1 retry",
    );
    expect(verifyWords(checked(verify("failed", 0, "red"), { state: "needsYou", maxRetries: 0 }))).toBe("check failed");
  });

  it("does not talk about a check once the task moved on to a pull request", () => {
    expect(verifyWords(checked(verify("passed", 0), { state: "inCi" }))).toBeNull();
  });
});
