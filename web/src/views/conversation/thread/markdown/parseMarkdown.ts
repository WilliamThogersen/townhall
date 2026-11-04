export type Inline =
  | { readonly kind: "text"; readonly text: string }
  | { readonly kind: "code"; readonly text: string }
  | { readonly kind: "strong"; readonly children: readonly Inline[] }
  | { readonly kind: "em"; readonly children: readonly Inline[] }
  | { readonly kind: "link"; readonly href: string; readonly children: readonly Inline[] };

export type Line = readonly Inline[];

export type Block =
  | { readonly kind: "paragraph"; readonly lines: readonly Line[] }
  | { readonly kind: "code"; readonly lang: string; readonly text: string }
  | {
      readonly kind: "list";
      readonly ordered: boolean;
      readonly start: number;
      readonly items: readonly (readonly Line[])[];
    };

const INLINE =
  /`([^`\n]+)`|\[([^\]\n]+)\]\(([^)\s]+)\)|\*\*(.+?)\*\*|__(.+?)__|\*([^\s*](?:[^*]*?[^\s*])?)\*|(?<![\p{L}\p{N}_])_([^\s_](?:[^_]*?[^\s_])?)_(?![\p{L}\p{N}_])/u;

const SAFE_PROTOCOL = /^(https?:|mailto:)/i;
const FENCE = /^\s*```\s*([^`\s]*)\s*$/;
const BULLET = /^\s*[-*+]\s+(.*)$/;
const NUMBERED = /^\s*(\d{1,9})[.)]\s+(.*)$/;
const HEADING = /^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$/;
const CONTINUATION = /^\s{2,}\S/;

export function safeHref(href: string): string | null {
  return SAFE_PROTOCOL.test(href.trim()) ? href.trim() : null;
}

function inlineFromMatch(match: RegExpExecArray): readonly Inline[] {
  const [, code, linkText, href, strongA, strongB, emA, emB] = match;
  if (code !== undefined) return [{ kind: "code", text: code }];
  if (linkText !== undefined && href !== undefined) {
    const children = parseInline(linkText);
    const safe = safeHref(href);
    return safe === null ? children : [{ kind: "link", href: safe, children }];
  }
  const strong = strongA ?? strongB;
  if (strong !== undefined) return [{ kind: "strong", children: parseInline(strong) }];
  return [{ kind: "em", children: parseInline(emA ?? emB ?? "") }];
}

export function parseInline(text: string): readonly Inline[] {
  const match = INLINE.exec(text);
  if (match === null) return text === "" ? [] : [{ kind: "text", text }];
  const before = text.slice(0, match.index);
  const after = text.slice(match.index + match[0].length);
  const lead: readonly Inline[] = before === "" ? [] : [{ kind: "text", text: before }];
  return [...lead, ...inlineFromMatch(match), ...parseInline(after)];
}

interface ListItemStart {
  readonly ordered: boolean;
  readonly start: number;
  readonly text: string;
}

function listItemStart(line: string): ListItemStart | null {
  const bullet = BULLET.exec(line);
  if (bullet) return { ordered: false, start: 1, text: bullet[1] ?? "" };
  const numbered = NUMBERED.exec(line);
  if (numbered) return { ordered: true, start: Number(numbered[1]), text: numbered[2] ?? "" };
  return null;
}

function headingLine(line: string): Line | null {
  const heading = HEADING.exec(line);
  return heading ? [{ kind: "strong", children: parseInline(heading[1] ?? "") }] : null;
}

interface Cursor {
  readonly lines: readonly string[];
  readonly at: number;
}

interface Taken {
  readonly block: Block;
  readonly next: number;
}

function takeFence({ lines, at }: Cursor, lang: string): Taken {
  const end = lines.findIndex((line, i) => i > at && FENCE.test(line) && line.trim() === "```");
  const stop = end === -1 ? lines.length : end;
  const text = lines.slice(at + 1, stop).join("\n");
  return { block: { kind: "code", lang, text }, next: stop + 1 };
}

function takeList({ lines, at }: Cursor, first: ListItemStart): Taken {
  let items: readonly (readonly Line[])[] = [[parseInline(first.text)]];
  let i = at + 1;
  for (; i < lines.length; i += 1) {
    const line = lines[i] ?? "";
    const item = listItemStart(line);
    if (item && item.ordered === first.ordered) {
      items = [...items, [parseInline(item.text)]];
    } else if (!item && CONTINUATION.test(line) && !FENCE.test(line)) {
      const last = items.at(-1) ?? [];
      items = [...items.slice(0, -1), [...last, parseInline(line.trim())]];
    } else {
      break;
    }
  }
  return { block: { kind: "list", ordered: first.ordered, start: first.start, items }, next: i };
}

function startsBlock(line: string): boolean {
  return line.trim() === "" || FENCE.test(line) || listItemStart(line) !== null || HEADING.test(line);
}

function takeParagraph({ lines, at }: Cursor): Taken {
  let i = at + 1;
  while (i < lines.length && !startsBlock(lines[i] ?? "")) i += 1;
  const paragraph = lines.slice(at, i).map((line) => parseInline(line.trim()));
  return { block: { kind: "paragraph", lines: paragraph }, next: i };
}

function takeBlock(cursor: Cursor): Taken | number {
  const line = cursor.lines[cursor.at] ?? "";
  if (line.trim() === "") return cursor.at + 1;
  const fence = FENCE.exec(line);
  if (fence) return takeFence(cursor, fence[1] ?? "");
  const heading = headingLine(line);
  if (heading) return { block: { kind: "paragraph", lines: [heading] }, next: cursor.at + 1 };
  const item = listItemStart(line);
  if (item) return takeList(cursor, item);
  return takeParagraph(cursor);
}

export function parseBlocks(source: string): readonly Block[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let at = 0;
  while (at < lines.length) {
    const taken = takeBlock({ lines, at });
    if (typeof taken === "number") {
      at = taken;
    } else {
      blocks.push(taken.block);
      at = taken.next;
    }
  }
  return blocks;
}
