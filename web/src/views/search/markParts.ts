export interface MarkPart {
  readonly text: string;
  readonly marked: boolean;
}

const PATTERN_CHARS = /[.*+?^${}()|[\]\\/]/g;

export function matchPattern(query: string): RegExp | null {
  const words = query.trim().split(/\s+/).filter((word) => word !== "");
  if (words.length === 0) return null;
  return new RegExp(words.map((word) => word.replace(PATTERN_CHARS, "\\$&")).join("\\s+"), "giu");
}

export function markParts(text: string, query: string): readonly MarkPart[] {
  const pattern = matchPattern(query);
  const found = pattern ? [...text.matchAll(pattern)] : [];
  if (found.length === 0) return [{ text, marked: false }];
  const endOf = (match: RegExpExecArray) => match.index + match[0].length;
  const parts = found.flatMap((match, i) => {
    const previous = found[i - 1];
    const from = previous ? endOf(previous) : 0;
    return [
      { text: text.slice(from, match.index), marked: false },
      { text: match[0], marked: true },
    ];
  });
  const last = found.at(-1);
  const tail = { text: last ? text.slice(endOf(last)) : "", marked: false };
  return [...parts, tail].filter((part) => part.text !== "");
}
