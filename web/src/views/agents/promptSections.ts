export interface PromptSection {
  readonly tag: string;
  readonly body: string;
  readonly closed: boolean;
}

export interface ParsedPrompt {
  readonly intro: string;
  readonly sections: readonly PromptSection[];
}

export type PromptBlock =
  | { readonly kind: "paragraph"; readonly text: string }
  | { readonly kind: "list"; readonly ordered: boolean; readonly items: readonly string[] };

export interface PromptStats {
  readonly chars: number;
  readonly tokens: number;
  readonly sections: number;
}

const OPEN = /^\s*<([A-Za-z][\w-]*)>(.*)$/;
const CLOSE = /^(.*?)\s*<\/([A-Za-z][\w-]*)>\s*$/;
const CHARS_PER_TOKEN = 4;

export function parsePrompt(text: string): ParsedPrompt {
  let intro = "";
  const sections: PromptSection[] = [];
  let loose: string[] = [];
  let open: { tag: string; lines: string[]; stack: string[] } | null = null;

  const flushLoose = () => {
    const body = loose.join("\n").trim();
    loose = [];
    if (body === "") return;
    if (sections.length === 0) intro = body;
    else sections.push({ tag: "", body, closed: true });
  };
  const finish = (closed: boolean) => {
    if (open === null) return;
    sections.push({ tag: open.tag, body: open.lines.join("\n").trim(), closed });
    open = null;
  };
  const oneLiner = (tag: string, rest: string): string | null => {
    const closing = CLOSE.exec(rest);
    return closing !== null && closing[2] === tag ? (closing[1] ?? "") : null;
  };

  for (const line of text.split("\n")) {
    if (open === null) {
      const start = OPEN.exec(line);
      if (start === null) {
        loose.push(line);
        continue;
      }
      flushLoose();
      const tag = start[1] ?? "";
      const rest = start[2] ?? "";
      const inline = oneLiner(tag, rest);
      open = { tag, lines: [inline ?? rest], stack: [tag] };
      if (inline !== null) finish(true);
      continue;
    }
    const closing = CLOSE.exec(line);
    const depth = closing === null ? -1 : open.stack.lastIndexOf(closing[2] ?? "");
    if (closing !== null && depth >= 0) {
      open.lines.push(depth === 0 ? (closing[1] ?? "") : line);
      open.stack.length = depth;
      if (open.stack.length === 0) finish(true);
      continue;
    }
    const nested = OPEN.exec(line);
    if (nested !== null && oneLiner(nested[1] ?? "", nested[2] ?? "") === null) open.stack.push(nested[1] ?? "");
    open.lines.push(line);
  }
  finish(false);
  flushLoose();
  return { intro, sections };
}

const ORDERED_ITEM = /^\s*\d+[.)]\s+(.*\S)\s*$/;
const BULLET_ITEM = /^\s*[-*]\s+(.*\S)\s*$/;

export function parseBlocks(text: string): readonly PromptBlock[] {
  const blocks: PromptBlock[] = [];
  let paragraph: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;

  const endParagraph = () => {
    if (paragraph.length > 0) blocks.push({ kind: "paragraph", text: paragraph.join("\n") });
    paragraph = [];
  };
  const endList = () => {
    if (list !== null) blocks.push({ kind: "list", ordered: list.ordered, items: list.items });
    list = null;
  };

  for (const line of text.split("\n")) {
    if (line.trim() === "") {
      endParagraph();
      endList();
      continue;
    }
    const ordered = ORDERED_ITEM.exec(line);
    const bullet = ordered === null ? BULLET_ITEM.exec(line) : null;
    const item = ordered?.[1] ?? bullet?.[1];
    if (item !== undefined) {
      endParagraph();
      const isOrdered = ordered !== null;
      if (list !== null && list.ordered !== isOrdered) endList();
      list ??= { ordered: isOrdered, items: [] };
      list.items.push(item);
      continue;
    }
    if (list !== null && /^\s/.test(line)) {
      list.items[list.items.length - 1] += ` ${line.trim()}`;
      continue;
    }
    endList();
    paragraph.push(line);
  }
  endParagraph();
  endList();
  return blocks;
}

export function promptStats(text: string): PromptStats {
  return {
    chars: text.length,
    tokens: Math.ceil(text.length / CHARS_PER_TOKEN),
    sections: parsePrompt(text).sections.length,
  };
}
