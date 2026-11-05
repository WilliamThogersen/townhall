/**
 * An agent's question split for display: what it is about to run (`context`), the question
 * itself (`prompt`) and its numbered options. The daemon sends the question as one text, the
 * way the agent's screen showed it (`agent/state/question.rs`).
 */

export interface AskOption {
  readonly number: string;
  readonly label: string;
  /** The part after ` · ` in Claude's own labels, shown faint. */
  readonly hint: string | null;
}

export interface AskView {
  readonly context: readonly string[];
  readonly prompt: string;
  readonly options: readonly AskOption[];
}

/** `1. label` or `2) label`, after an optional selection cursor. */
const NUMBERED = /^\s*(?:[❯›>▸►]\s+)?(\d{1,2})[.)]\s+(.+)$/;
const HINT_SEPARATOR = " · ";

interface Numbered {
  readonly index: number;
  readonly number: string;
  readonly text: string;
}

function numbered(line: string, index: number): Numbered | null {
  const match = NUMBERED.exec(line);
  if (match === null) return null;
  return { index, number: String(Number(match[1])), text: (match[2] ?? "").trim() };
}

/** The last run of lines numbered 1, 2, 3 ..., at least two long, whose numbers are choices. */
function optionRun(lines: readonly string[], choices: readonly string[]): readonly Numbered[] {
  let best: readonly Numbered[] = [];
  let run: readonly Numbered[] = [];
  lines.forEach((line, index) => {
    const item = numbered(line, index);
    const next = item !== null && choices.includes(item.number) ? item : null;
    if (next !== null && next.number === String(run.length + 1)) {
      run = [...run, next];
    } else {
      run = next !== null && next.number === "1" ? [next] : [];
    }
    if (run.length >= 2) best = run;
  });
  return best;
}

function toOption({ number, text }: Numbered): AskOption {
  const cut = text.indexOf(HINT_SEPARATOR);
  if (cut === -1) return { number, label: text, hint: null };
  return { number, label: text.slice(0, cut).trim(), hint: text.slice(cut + HINT_SEPARATOR.length).trim() };
}

const asks = (line: string): boolean => /[?？][\s*_`"')\]:]*$/.test(line) || /\((y|yes)\/(n|no)\)|\[y\/n\]/i.test(line);

export function askView(question: string, choices: readonly string[]): AskView {
  const lines = question
    .split("\n")
    .map((line) => line.trimEnd())
    .filter((line) => line.trim() !== "");
  const run = optionRun(lines, choices);
  const taken = new Set(run.map((item) => item.index));
  const rest = lines.map((line, index) => ({ line: line.trim(), index })).filter(({ index }) => !taken.has(index));
  const asking = [...rest].reverse().find(({ line }) => asks(line)) ?? rest.at(-1);
  return {
    context: rest.filter((item) => item !== asking).map(({ line }) => line),
    prompt: asking?.line ?? "",
    options: run.map(toOption),
  };
}

/** What an answer picked: `3 · Yes` for an option number, else the text as it was sent. */
export function answerText(view: AskView, sent: string): string {
  const text = sent.trim();
  const option = view.options.find((o) => o.number === text);
  return option === undefined ? text : `${option.number}${HINT_SEPARATOR}${option.label}`;
}

const TOOL_CALL = /^(\w+): (\S.*)$/;
const SHELL_BREAK = /[;|&<>`$()\n]/;
const PROGRAM = /^[\w./+-]+$/;
const SUBCOMMAND = /^[A-Za-z][\w-]*$/;

function commandPattern(command: string): string | null {
  const words = (command.split(SHELL_BREAK)[0] ?? "")
    .replace(/\u2026/g, "")
    .split(/\s+/)
    .filter((word) => word !== "");
  while (words[0] !== undefined && /^\w+=/.test(words[0])) words.shift();
  const [program, next] = words;
  if (program === undefined || !PROGRAM.test(program)) return null;
  const sub = next !== undefined && SUBCOMMAND.test(next) ? ` ${next}` : "";
  return `Bash(${program}${sub} *)`;
}

export function permissionTool(context: readonly string[]): string | null {
  const match = context.map((line) => TOOL_CALL.exec(line.trim())).find((m) => m !== null);
  if (match === undefined || match === null) return null;
  const [, tool = "", what = ""] = match;
  return tool === "Bash" ? commandPattern(what) : tool;
}

export function yesOption(options: readonly AskOption[]): string | null {
  return options.find((o) => /^yes\b/i.test(o.label))?.number ?? null;
}
