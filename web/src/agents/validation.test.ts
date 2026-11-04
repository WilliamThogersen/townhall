import { describe, expect, it } from "vitest";
import { EMPTY_FORM, validateForm, validateName } from "./validation";

describe("validateName", () => {
  it.each(["a", "fix-login", "A_1", "x-Y_9"])("accepts %s", (n) => {
    expect(validateName(n)).toBeNull();
  });
  it.each(["has space", "a/b", "dot.name", "ünï", "a;b"])("rejects %j", (n) => {
    expect(validateName(n)).toBe("use letters, digits, - and _ only");
  });
  it("asks for a name when it is empty", () => {
    expect(validateName("")).toBe("name is required");
  });
});

describe("validateForm", () => {
  it("builds a claude input with trimmed prompt and omits empty base", () => {
    const r = validateForm({ ...EMPTY_FORM, name: "a", prompt: " do it " });
    expect(r.input).toEqual({ name: "a", adapter: "claude", prompt: "do it" });
  });
  it("omits an empty prompt", () => {
    expect(validateForm({ ...EMPTY_FORM, name: "a", prompt: "  " }).input).toEqual({
      name: "a",
      adapter: "claude",
    });
  });
  it("requires a command for shell", () => {
    const r = validateForm({ ...EMPTY_FORM, name: "a", adapter: "shell" });
    expect(r.input).toBeNull();
    expect(r.errors.command).toBe("a shell agent needs a command");
  });
  it("rejects a base starting with -", () => {
    expect(validateForm({ ...EMPTY_FORM, name: "a", base: "-x" }).errors.base).toBe(
      "base branch must not start with -",
    );
  });
  it("includes base and command for shell", () => {
    const r = validateForm({ ...EMPTY_FORM, name: "a", adapter: "shell", command: "cat", base: "main" });
    expect(r.input).toEqual({ name: "a", adapter: "shell", command: "cat", base: "main" });
  });
  it("reports every broken field at once", () => {
    const r = validateForm({ ...EMPTY_FORM, adapter: "shell", base: "-x" });
    expect(Object.keys(r.errors).sort()).toEqual(["base", "command", "name"]);
  });
});
