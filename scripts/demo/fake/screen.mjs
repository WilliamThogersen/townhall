import { isAbsolute, relative } from "node:path";

const LINE_MS = 70;
const DIM = "\u001b[2m";
const GREEN = "\u001b[32m";
const RED = "\u001b[31m";
const YELLOW = "\u001b[33m";
const BOLD = "\u001b[1m";
const RESET = "\u001b[0m";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export function header(ctx) {
  return [
    `${BOLD}✻ Claude Code${RESET} ${DIM}townhall demo stand-in${RESET}`,
    `${DIM}  ${ctx.name} · ${ctx.sessionId.slice(0, 8)}${RESET}`,
    "",
    `${DIM}>${RESET} ${ctx.prompt.trim()}`,
    "",
  ];
}

function shown(ctx, file) {
  return isAbsolute(file) && file.startsWith(`${ctx.cwd}/`) ? relative(ctx.cwd, file) : file;
}

function label(ctx, step) {
  const input = step.input ?? {};
  const names = { Read: "Read", Write: "Write", Edit: "Update", Bash: "Bash" };
  if (input.file_path) return `${names[step.tool] ?? step.tool}(${shown(ctx, input.file_path)})`;
  if (input.command) return `Bash(${input.command})`;
  if (input.pattern) return `Search(pattern: "${input.pattern}")`;
  return step.tool;
}

function tint(line, step) {
  if (step.error) return `${RED}${line}${RESET}`;
  if (/^[+✓]/.test(line.trim())) return `${GREEN}${line}${RESET}`;
  if (/^[-✘]/.test(line.trim())) return `${RED}${line}${RESET}`;
  return `${DIM}${line}${RESET}`;
}

export function stepLines(ctx, step) {
  const bullet = step.error ? `${RED}⏺${RESET}` : step.running ? `${YELLOW}⏺${RESET}` : `${GREEN}⏺${RESET}`;
  const out = step.out.map((line, i) => tint(`${i === 0 ? "  ⎿  " : "     "}${line}`, step));
  return [`${bullet} ${label(ctx, step)}`, ...out, ""];
}

export function sayLines(text) {
  return [`${BOLD}⏺${RESET} ${text}`, ""];
}

export function workingLine(step) {
  return `${YELLOW}✻ ${step.input?.command ?? step.tool}…${RESET} ${DIM}(esc to interrupt)${RESET}`;
}

export async function show(lines) {
  for (const line of lines) {
    process.stdout.write(`${line}\n`);
    await sleep(LINE_MS);
  }
}

export function promptMark() {
  process.stdout.write(`${DIM}>${RESET} `);
}
