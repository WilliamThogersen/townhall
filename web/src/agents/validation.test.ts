import { describe, expect, it } from "vitest";
import { EMPTY_FORM, validateForm, validateName } from "./validation";

describe("validateName", () => {
  it.each(["a", "fix-login", "A_1", "x-Y_9"])("accepts %s", (n) => {
    expect(validateName(n)).toBeNull();
  });
  it.each(["", "has space", "a/b", "dot.name", "ünï", "a;b"])("rejects %j", (n) => {
    expect(validateName(n)).not.toBeNull();
  });
});

describe("validateForm", () => {
  it("builds a claude input with trimmed prompt and omits empty base", () => {
    const r = validateForm({ ...EMPTY_FORM, name: "a", prompt: " do it " });
    expect(r.input).toEqual({ name: "a", adapter: "claude", prompt: "do it" });
  });
  it("requires a command for shell", () => {
    const r = validateForm({ ...EMPTY_FORM, name: "a", adapter: "shell" });
    expect(r.input).toBeNull();
    expect(r.errors.command).toBeDefined();
  });
  it("rejects a base starting with -", () => {
    expect(validateForm({ ...EMPTY_FORM, name: "a", base: "-x" }).errors.base).toBeDefined();
  });
  it("includes base and command for shell", () => {
    const r = validateForm({ ...EMPTY_FORM, name: "a", adapter: "shell", command: "cat", base: "main" });
    expect(r.input).toEqual({ name: "a", adapter: "shell", command: "cat", base: "main" });
  });
});
