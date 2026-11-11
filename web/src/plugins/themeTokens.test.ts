import { afterEach, describe, expect, it } from "vitest";
import { readThemePayload, THEME_TOKEN_NAMES } from "./themeTokens";

afterEach(() => {
  document.documentElement.removeAttribute("style");
  document.documentElement.removeAttribute("data-theme");
});

describe("themeTokens", () => {
  it("lists the --th- tokens of the stylesheet once each", () => {
    expect(THEME_TOKEN_NAMES.length).toBeGreaterThan(5);
    expect(THEME_TOKEN_NAMES.every((name) => name.startsWith("--th-"))).toBe(true);
    expect(new Set(THEME_TOKEN_NAMES).size).toBe(THEME_TOKEN_NAMES.length);
  });

  it("reads the resolved theme and the values that are set", () => {
    document.documentElement.setAttribute("data-theme", "light");
    const name = THEME_TOKEN_NAMES[0] ?? "";
    document.documentElement.style.setProperty(name, " #123456 ");
    const payload = readThemePayload();
    expect(payload.resolved).toBe("light");
    expect(payload.tokens[name]).toBe("#123456");
  });

  it("leaves out a token with no value", () => {
    const payload = readThemePayload();
    for (const value of Object.values(payload.tokens)) expect(value).not.toBe("");
  });
});
