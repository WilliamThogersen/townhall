import { AVATAR_PARTS, defaultAvatar, GRID, type Avatar, type AvatarPart } from "./avatar";

export type AvatarSpec = { readonly [P in AvatarPart]?: Avatar[P] } & { readonly grid?: readonly string[] };

export type SpecResult =
  | { readonly ok: true; readonly spec: AvatarSpec }
  | { readonly ok: false; readonly errors: readonly string[] };

export interface ResolvedAvatar {
  readonly avatar: Avatar;
  readonly grid: readonly string[] | null;
}

const PARTS = Object.keys(AVATAR_PARTS) as readonly AvatarPart[];
const KEYS = [...PARTS, "grid"] as const;
const HEADER = "[avatar]";
const GRID_CELLS = ".#o";
const KEY_LINE = /^([A-Za-z_][\w-]*)\s*=\s*(.*)$/;
const QUOTED = /^(["'])(.*)\1$/;
const QUOTED_ANYWHERE = /"[^"]*"|'[^']*'/g;

function stripComment(line: string): string {
  let quote: string | null = null;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (quote !== null) {
      if (ch === quote) quote = null;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
    } else if (ch === "#") {
      return line.slice(0, i);
    }
  }
  return line;
}

function gridProblems(rows: readonly string[]): readonly string[] {
  const height = rows.length === GRID.height ? [] : [`grid must have ${GRID.height} rows, found ${rows.length}`];
  const cells = rows.flatMap((row, at) => {
    const cellsOfRow = [...row];
    if (cellsOfRow.length !== GRID.width) {
      return [`grid row ${at + 1} must have ${GRID.width} cells, found ${cellsOfRow.length}`];
    }
    const stray = cellsOfRow.find((cell) => !GRID_CELLS.includes(cell));
    return stray === undefined ? [] : [`grid row ${at + 1} has \`${stray}\`, use only . (empty), # (body) and o (face)`];
  });
  return [...height, ...cells];
}

const GRID_LIST_ERROR = `grid must be a list of ${GRID.height} quoted rows`;

function readGrid(text: string): { readonly rows: readonly string[] } | { readonly errors: readonly string[] } {
  if (!text.startsWith("[") || !text.endsWith("]")) return { errors: [GRID_LIST_ERROR] };
  const inner = text.slice(1, -1);
  if (inner.replace(QUOTED_ANYWHERE, "").replace(/[\s,]/g, "") !== "") return { errors: [GRID_LIST_ERROR] };
  const rows = Array.from(inner.matchAll(QUOTED_ANYWHERE), (match) => match[0].slice(1, -1));
  const problems = gridProblems(rows);
  return problems.length > 0 ? { errors: problems } : { rows };
}

function readPart(part: AvatarPart, value: string): { readonly value: string } | { readonly error: string } {
  const options: readonly string[] = AVATAR_PARTS[part];
  const quoted = QUOTED.exec(value);
  if (quoted === null) return { error: `${part} must be a quoted string like "${options[0]}"` };
  const text = quoted[2] ?? "";
  return options.includes(text)
    ? { value: text }
    : { error: `${part} \`${text}\` is not one of ${options.join(", ")}` };
}

export function parseAvatarSpec(text: string): SpecResult {
  const lines = text.split("\n");
  const errors: string[] = [];
  const spec: Record<string, unknown> = {};
  let headerSeen = false;
  for (let i = 0; i < lines.length; i++) {
    const at = `line ${i + 1}: `;
    const raw = stripComment(lines[i] ?? "").trim();
    if (raw === "") continue;
    if (raw.startsWith("[")) {
      if (raw === HEADER && !headerSeen) headerSeen = true;
      else errors.push(`${at}only the ${HEADER} table belongs here`);
      continue;
    }
    const line = KEY_LINE.exec(raw);
    if (line === null) {
      errors.push(`${at}expected key = value`);
      continue;
    }
    if (!headerSeen) {
      errors.push(`${at}start with the ${HEADER} header`);
      headerSeen = true;
    }
    const key = line[1] ?? "";
    let value = line[2] ?? "";
    if (!(KEYS as readonly string[]).includes(key)) {
      errors.push(`${at}unknown key \`${key}\`, use ${KEYS.slice(0, -1).join(", ")} or grid`);
      continue;
    }
    if (key in spec) {
      errors.push(`${at}${key} is set twice`);
      continue;
    }
    if (key === "grid") {
      while (!value.endsWith("]") && i + 1 < lines.length) {
        i += 1;
        value = `${value} ${stripComment(lines[i] ?? "").trim()}`.trim();
      }
      const grid = readGrid(value);
      if ("rows" in grid) spec.grid = grid.rows;
      else errors.push(...grid.errors.map((message) => `${at}${message}`));
      continue;
    }
    const part = readPart(key as AvatarPart, value);
    if ("value" in part) spec[key] = part.value;
    else errors.push(`${at}${part.error}`);
  }
  return errors.length > 0 ? { ok: false, errors } : { ok: true, spec: spec as AvatarSpec };
}

export function formatAvatarSpec(spec: AvatarSpec): string {
  const parts = PARTS.flatMap((part) => (spec[part] === undefined ? [] : [`${part} = "${spec[part]}"`]));
  const grid = spec.grid === undefined ? [] : [`grid = [\n${spec.grid.map((row) => `  "${row}",`).join("\n")}\n]`];
  return [HEADER, ...parts, ...grid].join("\n") + "\n";
}

export function resolveAvatar(name: string, spec: AvatarSpec | null): ResolvedAvatar {
  const base = defaultAvatar(name);
  const avatar = Object.fromEntries(PARTS.map((part) => [part, spec?.[part] ?? base[part]])) as unknown as Avatar;
  return { avatar, grid: spec?.grid ?? null };
}

export function effectiveSpec(name: string, spec: AvatarSpec | null): AvatarSpec {
  const { avatar, grid } = resolveAvatar(name, spec);
  return grid === null ? avatar : { ...avatar, grid };
}

function isHeaderLine(line: string): boolean {
  return stripComment(line).trim() === HEADER;
}

function isTableLine(line: string): boolean {
  return stripComment(line).trim().startsWith("[");
}

function opensOrClosesString(line: string): boolean {
  const triples = (line.match(/"""|'''/g) ?? []).length;
  return triples % 2 === 1;
}

function tableRange(lines: readonly string[]): { readonly start: number; readonly end: number } | null {
  let inString = false;
  let start = -1;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (!inString && start < 0 && isHeaderLine(line)) start = i;
    else if (!inString && start >= 0 && isTableLine(line)) return { start, end: i };
    if (opensOrClosesString(line)) inString = !inString;
  }
  return start < 0 ? null : { start, end: lines.length };
}

export function withAvatarSection(source: string, section: string): string {
  const lines = source.split("\n");
  const range = tableRange(lines);
  const before = range === null ? lines : lines.slice(0, range.start);
  const after = range === null ? [] : lines.slice(range.end);
  const kept = [...before, ...after].join("\n").replace(/\s+$/, "");
  const body = kept === "" ? "" : `${kept}\n`;
  const text = section.trim();
  return text === "" ? body : `${body}\n${text}\n`;
}
