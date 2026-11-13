const PLUGINS_KEY = /^\s*plugins\s*=/;

interface Span {
  readonly start: number;
  readonly stop: number;
}

interface Layout {
  readonly end: number;
  readonly key: Span | null;
}

function opensOrClosesString(line: string): boolean {
  return ((line.match(/"""|'''/g) ?? []).length) % 2 === 1;
}

function bracketDelta(line: string): number {
  return (line.match(/\[/g) ?? []).length - (line.match(/\]/g) ?? []).length;
}

function keySpan(lines: readonly string[], start: number): Span {
  let stop = start + 1;
  let depth = bracketDelta(lines[start] ?? "");
  while (depth > 0 && stop < lines.length) {
    depth += bracketDelta(lines[stop] ?? "");
    stop += 1;
  }
  return { start, stop };
}

function layoutOf(lines: readonly string[]): Layout {
  let inString = false;
  let key: Span | null = null;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (!inString) {
      if (line.trim().startsWith("[")) return { end: i, key };
      if (key === null && PLUGINS_KEY.test(line)) {
        key = keySpan(lines, i);
        i = key.stop - 1;
        continue;
      }
    }
    if (opensOrClosesString(line)) inString = !inString;
  }
  return { end: lines.length, key };
}

function listLine(ids: readonly string[]): string {
  return `plugins = [${ids.map((id) => JSON.stringify(id)).join(", ")}]`;
}

export function withPlugins(source: string, ids: readonly string[]): string {
  const lines = source.split("\n");
  const { end, key } = layoutOf(lines);
  if (key === null && ids.length === 0) return source;
  const kept = key === null ? lines : [...lines.slice(0, key.start), ...lines.slice(key.stop)];
  if (ids.length === 0) return kept.join("\n");
  const topEnd = end - (key === null ? 0 : key.stop - key.start);
  let at = topEnd;
  while (at > 0 && (kept[at - 1] ?? "").trim() === "") at -= 1;
  return [...kept.slice(0, at), listLine(ids), ...kept.slice(at)].join("\n");
}

export function withPlugin(source: string, current: readonly string[], id: string, on: boolean): string {
  if (!on) return withPlugins(source, current.filter((item) => !chooses(item, id)));
  return withPlugins(source, isChosen(current, id) ? current : [...current, id]);
}

function chooses(chosen: string, id: string): boolean {
  const [name = ""] = id.split("@");
  const wanted = chosen.toLowerCase();
  return wanted === id.toLowerCase() || wanted === name.toLowerCase();
}

export function typesUsing(
  types: readonly { readonly name: string; readonly plugins?: readonly string[] }[],
  id: string,
): readonly string[] {
  return types.filter((type) => (type.plugins ?? []).some((chosen) => chooses(chosen, id))).map((type) => type.name);
}

export interface Shipped {
  readonly skills: number;
  readonly commands: number;
}

export function shipped(plugin: { readonly permissions: readonly string[] }): Shipped {
  const count = (prefix: string) => plugin.permissions.filter((p) => p.startsWith(prefix)).length;
  return { skills: count("claude.skill:"), commands: count("claude.command:") };
}

export function shippedText({ skills, commands }: Shipped): string {
  const part = (n: number, one: string) => (n === 0 ? [] : [`${n} ${one}${n === 1 ? "" : "s"}`]);
  return [...part(skills, "skill"), ...part(commands, "command")].join(", ");
}

export function pluginsOf(type: { readonly plugins?: readonly string[] }): readonly string[] {
  return type.plugins ?? [];
}

export function isChosen(chosen: readonly string[], id: string): boolean {
  return chosen.some((item) => chooses(item, id));
}
