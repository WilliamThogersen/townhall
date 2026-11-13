import { describe, expect, it } from "vitest";
import { typesUsing, withPlugin, withPlugins } from "./agentPlugins";

const BASE = 'name = "web"\nadapter = "claude"\nmodel = "haiku"\n';

describe("withPlugins", () => {
  it("adds a plugins line after the plain keys", () => {
    expect(withPlugins(BASE, ["modern-web-guidance@claude-plugins-official"])).toBe(
      `${BASE}plugins = ["modern-web-guidance@claude-plugins-official"]\n`,
    );
  });

  it("puts the line before the first table and after a multi line prompt", () => {
    const source = `${BASE}prompt = """\n[not a table]\nbe brief\n"""\n\n[avatar]\ncolor = "info"\n`;
    expect(withPlugins(source, ["a@m"])).toBe(
      `${BASE}prompt = """\n[not a table]\nbe brief\n"""\nplugins = ["a@m"]\n\n[avatar]\ncolor = "info"\n`,
    );
  });

  it("replaces a one line list", () => {
    expect(withPlugins(`${BASE}plugins = ["old@m"]\ntools = ["Read"]\n`, ["a@m", "b@m"])).toBe(
      `${BASE}tools = ["Read"]\nplugins = ["a@m", "b@m"]\n`,
    );
  });

  it("replaces a list spread over lines", () => {
    const source = `${BASE}plugins = [\n  "x@m",\n  "y@m",\n]\ntools = ["Read"]\n`;
    expect(withPlugins(source, ["z@m"])).toBe(`${BASE}tools = ["Read"]\nplugins = ["z@m"]\n`);
  });

  it("removes the key when no plugin is chosen and leaves other tables alone", () => {
    const source = `${BASE}plugins = ["a@m"]\n\n[env]\nplugins = "kept"\n`;
    expect(withPlugins(source, [])).toBe(`${BASE}\n[env]\nplugins = "kept"\n`);
  });

  it("does not touch a plugins line inside a prompt", () => {
    const source = `${BASE}prompt = """\nplugins = ["fake"]\n"""\n`;
    expect(withPlugins(source, ["a@m"])).toBe(`${source}plugins = ["a@m"]\n`);
  });

  it("leaves a source with nothing to change as it was", () => {
    expect(withPlugins(BASE, [])).toBe(BASE);
  });

  it("quotes ids as toml strings", () => {
    expect(withPlugins(BASE, ['we"ird@m'])).toContain('plugins = ["we\\"ird@m"]');
  });
});

describe("withPlugin", () => {
  it("adds and removes one id keeping the others, once each", () => {
    const one = withPlugin(BASE, [], "a@m", true);
    const two = withPlugin(one, ["a@m"], "b@m", true);
    expect(two).toContain('plugins = ["a@m", "b@m"]');
    expect(withPlugin(two, ["a@m", "b@m"], "a@m", true)).toBe(two);
    expect(withPlugin(two, ["a@m", "b@m"], "a@m", false)).toContain('plugins = ["b@m"]');
  });
});

describe("typesUsing", () => {
  const types = [
    { name: "web", plugins: ["Web@Acme"] },
    { name: "plain", plugins: [] },
    { name: "bare" },
  ];
  it("lists the types that choose the plugin by id or by bare name, ignoring case", () => {
    expect(typesUsing(types, "web@acme")).toEqual(["web"]);
    expect(typesUsing([{ name: "x", plugins: ["web"] }], "web@acme")).toEqual(["x"]);
    expect(typesUsing(types, "other@acme")).toEqual([]);
  });
});
