import { describe, expect, it } from "vitest";
import { composeAvatar, defaultAvatar, type Avatar } from "./avatar";
import {
  effectiveSpec,
  formatAvatarSpec,
  parseAvatarSpec,
  resolveAvatar,
  withAvatarSection,
  type AvatarSpec,
} from "./avatarSpec";

const GRID = [
  "....#....",
  "....#....",
  ".#######.",
  "#########",
  "##o###o##",
  "#########",
  "##oooo###",
  ".#######.",
  "..#####..",
];

const full: AvatarSpec = { head: "round", eyes: "visor", mouth: "smile", accessory: "antenna", color: "fg-muted" };

const parsed = (text: string) => {
  const result = parseAvatarSpec(text);
  if (!result.ok) throw new Error(result.errors.join("; "));
  return result.spec;
};

const errorsOf = (text: string) => {
  const result = parseAvatarSpec(text);
  if (result.ok) throw new Error("expected errors");
  return result.errors;
};

describe("parseAvatarSpec", () => {
  it("reads every part key", () => {
    const text = [
      "[avatar]",
      'head = "round"',
      'eyes = "visor"',
      'mouth = "smile"',
      'accessory = "antenna"',
      'color = "fg-muted"',
    ].join("\n");
    expect(parsed(text)).toEqual(full);
  });

  it("allows any subset of keys, an empty section and empty text", () => {
    expect(parsed('[avatar]\ncolor = "ok"')).toEqual({ color: "ok" });
    expect(parsed("[avatar]")).toEqual({});
    expect(parsed("")).toEqual({});
    expect(parsed("  \n\n")).toEqual({});
  });

  it("ignores comments, blank lines, extra space and single quotes", () => {
    const text = "# my face\n[avatar]  # table\n\n  head   =   'domed'   # top\neyes = \"dots\"\n";
    expect(parsed(text)).toEqual({ head: "domed", eyes: "dots" });
  });

  it("reads a multi line grid", () => {
    const text = `[avatar]\ncolor = "info"\ngrid = [\n${GRID.map((row) => `  "${row}",`).join("\n")}\n]\n`;
    expect(parsed(text)).toEqual({ color: "info", grid: GRID });
  });

  it("reads a grid on one line without a trailing comma", () => {
    const text = `[avatar]\ngrid = [${GRID.map((row) => `"${row}"`).join(", ")}]`;
    expect(parsed(text).grid).toEqual(GRID);
  });

  it("names an unknown key and its line", () => {
    expect(errorsOf('[avatar]\nhat = "top"')).toEqual([
      "line 2: unknown key `hat`, use head, eyes, mouth, accessory, color or grid",
    ]);
  });

  it("names an unknown value and lists the options", () => {
    expect(errorsOf('[avatar]\nhead = "cube"')).toEqual([
      "line 2: head `cube` is not one of square, round, domed, tapered",
    ]);
    expect(errorsOf('[avatar]\ncolor = "pink"')[0]).toContain("fg-muted");
  });

  it("wants a quoted string for a part", () => {
    expect(errorsOf("[avatar]\nhead = round")).toEqual(['line 2: head must be a quoted string like "square"']);
    expect(errorsOf("[avatar]\nhead = 3")[0]).toContain("quoted string");
  });

  it("refuses a key given twice", () => {
    expect(errorsOf('[avatar]\nhead = "round"\nhead = "square"')).toEqual(["line 3: head is set twice"]);
  });

  it("refuses a line that is not key = value", () => {
    expect(errorsOf("[avatar]\nround")).toEqual(["line 2: expected key = value"]);
  });

  it("wants the [avatar] header first and refuses other tables", () => {
    expect(errorsOf('head = "round"')).toEqual(["line 1: start with the [avatar] header"]);
    expect(errorsOf("[avatar]\n[other]")).toEqual(["line 2: only the [avatar] table belongs here"]);
    expect(errorsOf("[avatar]\n[avatar]")).toEqual(["line 2: only the [avatar] table belongs here"]);
  });

  it("checks the grid size and cells like the daemon does", () => {
    expect(errorsOf('[avatar]\ngrid = ["#########"]')).toEqual(["line 2: grid must have 9 rows, found 1"]);
    const narrow = GRID.map((row, i) => (i === 3 ? row.slice(1) : row));
    expect(errorsOf(`[avatar]\ngrid = [${narrow.map((r) => `"${r}"`).join(",")}]`)).toEqual([
      "line 2: grid row 4 must have 9 cells, found 8",
    ]);
    const stray = GRID.map((row, i) => (i === 0 ? "....x...." : row));
    expect(errorsOf(`[avatar]\ngrid = [${stray.map((r) => `"${r}"`).join(",")}]`)).toEqual([
      "line 2: grid row 1 has `x`, use only . (empty), # (body) and o (face)",
    ]);
  });

  it("refuses a grid that is not a list of strings or is never closed", () => {
    expect(errorsOf('[avatar]\ngrid = "........."')[0]).toContain("grid must be a list");
    expect(errorsOf('[avatar]\ngrid = ["#########",')[0]).toContain("grid must be a list");
  });

  it("reports every problem, not just the first", () => {
    expect(errorsOf('[avatar]\nhat = "x"\nhead = "cube"')).toHaveLength(2);
  });
});

describe("formatAvatarSpec", () => {
  it("writes the header and the keys in a fixed order", () => {
    expect(formatAvatarSpec(full)).toBe(
      '[avatar]\nhead = "round"\neyes = "visor"\nmouth = "smile"\naccessory = "antenna"\ncolor = "fg-muted"\n',
    );
    expect(formatAvatarSpec({ color: "ok", head: "square" })).toBe('[avatar]\nhead = "square"\ncolor = "ok"\n');
  });

  it("writes a grid as one quoted row per line", () => {
    const text = formatAvatarSpec({ color: "info", grid: GRID });
    expect(text).toContain('grid = [\n  "....#....",\n');
    expect(text.endsWith('  "..#####..",\n]\n')).toBe(true);
  });

  it("round trips through the parser", () => {
    for (const spec of [{}, { color: "ok" }, full, { ...full, grid: GRID }] as const) {
      expect(parsed(formatAvatarSpec(spec))).toEqual(spec);
    }
  });
});

describe("resolveAvatar", () => {
  it("is the name hash default when there is no spec", () => {
    expect(resolveAvatar("planner", null)).toEqual({ avatar: defaultAvatar("planner"), grid: null });
    expect(resolveAvatar("planner", {})).toEqual({ avatar: defaultAvatar("planner"), grid: null });
  });

  it("lets the spec override only the keys it sets", () => {
    const base = defaultAvatar("planner");
    const { avatar } = resolveAvatar("planner", { eyes: "cyclops" });
    expect(avatar).toEqual({ ...base, eyes: "cyclops" });
  });

  it("carries the grid", () => {
    expect(resolveAvatar("planner", { grid: GRID }).grid).toEqual(GRID);
  });
});

describe("effectiveSpec", () => {
  it("spells out every part, from the default for a missing spec", () => {
    expect(effectiveSpec("planner", null)).toEqual(defaultAvatar("planner"));
    expect(effectiveSpec("planner", { color: "ok" })).toEqual({ ...defaultAvatar("planner"), color: "ok" });
    expect(effectiveSpec("planner", { grid: GRID }).grid).toEqual(GRID);
  });
});

describe("composeAvatar with a grid", () => {
  const avatar: Avatar = defaultAvatar("planner");

  it("draws the grid as written, # as body and o as face", () => {
    const cells = composeAvatar(avatar, GRID);
    expect(cells.filter((c) => c.tone === "body")).toHaveLength(GRID.join("").replace(/[.o]/g, "").length);
    expect(cells.filter((c) => c.tone === "face")).toHaveLength(GRID.join("").replace(/[.#]/g, "").length);
    expect(cells.find((c) => c.x === 4 && c.y === 0)).toEqual({ x: 4, y: 0, tone: "body" });
    expect(cells.find((c) => c.x === 2 && c.y === 4)).toEqual({ x: 2, y: 4, tone: "face" });
    expect(cells.find((c) => c.x === 0 && c.y === 0)).toBeUndefined();
  });

  it("ignores the parts when a grid is given", () => {
    expect(composeAvatar({ ...avatar, head: "round" }, GRID)).toEqual(composeAvatar({ ...avatar, head: "square" }, GRID));
  });
});

describe("withAvatarSection", () => {
  const source = 'name = "planner"\nadapter = "claude"\n';
  const section = '[avatar]\ncolor = "ok"\n';

  it("appends the section after a blank line", () => {
    expect(withAvatarSection(source, section)).toBe(`${source}\n${section}`);
  });

  it("adds a missing final newline first", () => {
    expect(withAvatarSection(source.trimEnd(), section)).toBe(`${source}\n${section}`);
  });

  it("replaces an existing section and keeps everything else as written", () => {
    const old = `${source}prompt = "hi"\n\n[avatar]\nhead = "round"\n`;
    expect(withAvatarSection(old, section)).toBe(`${source}prompt = "hi"\n\n${section}`);
  });

  it("removes the section when the text is blank", () => {
    const old = `${source}\n[avatar]\nhead = "round"\n`;
    expect(withAvatarSection(old, "")).toBe(source);
    expect(withAvatarSection(old, "  \n")).toBe(source);
    expect(withAvatarSection(source, "")).toBe(source);
  });

  it("does not mistake an [avatar] line inside a multi line prompt for the table", () => {
    const withPrompt = `name = "a"\nprompt = """\n[avatar]\nnot a table\n"""\n`;
    expect(withAvatarSection(withPrompt, section)).toBe(`${withPrompt}\n${section}`);
  });

  it("is stable: writing the same section twice changes nothing more", () => {
    const once = withAvatarSection(source, section);
    expect(withAvatarSection(once, section)).toBe(once);
  });
});
