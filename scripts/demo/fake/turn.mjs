import { createInterface } from "node:readline";
import { findTurn } from "../scenes/turns.mjs";
import { callTool, mcpEndpoint } from "./mcp.mjs";

const MODEL = "claude-demo";
const CONTEXT_WINDOW = 200000;
const DEFAULT_CONTEXT = 20000;
const THINK_MS = 700;
const OUTPUT_TOKENS = 180;

const emit = (value) => process.stdout.write(`${JSON.stringify(value)}\n`);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function parse(line) {
  try {
    return JSON.parse(line);
  } catch {
    return null;
  }
}

function userText(line) {
  const content = parse(line)?.message?.content;
  if (typeof content === "string") return content;
  return (Array.isArray(content) ? content : [])
    .filter((block) => block?.type === "text")
    .map((block) => block.text ?? "")
    .join("\n");
}

function sessionOf(args) {
  const flag = args.find((arg) => arg.startsWith("--session-id=") || arg.startsWith("--resume="));
  return flag ? flag.slice(flag.indexOf("=") + 1) : "demo";
}

function say(text, used) {
  const usage = { input_tokens: used, cache_creation_input_tokens: 0, cache_read_input_tokens: 0, output_tokens: OUTPUT_TOKENS };
  emit({ type: "assistant", message: { model: MODEL, role: "assistant", content: [{ type: "text", text }], usage } });
}

function succeed(text, used) {
  emit({
    type: "result",
    subtype: "success",
    is_error: false,
    result: text,
    usage: { input_tokens: used, output_tokens: OUTPUT_TOKENS },
    modelUsage: { [MODEL]: { contextWindow: CONTEXT_WINDOW } },
  });
}

async function drain(lines, code) {
  for (let next = await lines.next(); !next.done; next = await lines.next()) {
    parse(next.value);
  }
  return code;
}

function isInterrupt(line) {
  const request = parse(line);
  return request?.type === "control_request" && request?.request?.subtype === "interrupt" ? request : null;
}

async function hang(lines) {
  emit({ type: "stream_event", event: { type: "message_start" } });
  for (let next = await lines.next(); !next.done; next = await lines.next()) {
    const interrupt = isInterrupt(next.value);
    if (interrupt) {
      emit({ type: "control_response", response: { subtype: "success", request_id: interrupt.request_id } });
      emit({ type: "result", subtype: "error_during_execution", is_error: true, terminal_reason: "aborted_streaming" });
      return drain(lines, 0);
    }
  }
  return 0;
}

async function createTasks(endpoint, tasks) {
  const results = [];
  for (const task of tasks) {
    const input = { name: task.name, title: task.title, prompt: task.prompt, ...(task.after?.length ? { after: task.after } : {}) };
    results.push(await callTool(endpoint, "create_task", input));
  }
  return results.every((result) => result.ok);
}

async function answer(endpoint, scene) {
  const used = scene.context ?? DEFAULT_CONTEXT;
  if (scene.say) say(scene.say, used);
  const created = await createTasks(endpoint, scene.tasks ?? []);
  const closing = !created && scene.noTask ? scene.noTask : null;
  if (closing) say(closing, used);
  succeed(closing ?? scene.say ?? "", used);
}

export async function runTurn(args) {
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity })[Symbol.asyncIterator]();
  const first = await lines.next();
  if (first.done) return 4;
  const scene = findTurn(userText(first.value));
  emit({ type: "system", subtype: "init", session_id: sessionOf(args), model: MODEL, tools: [] });
  const endpoint = mcpEndpoint(args);
  if (scene.title) await callTool(endpoint, "set_title", { title: scene.title });
  if (scene.hang) return hang(lines);
  await sleep(THINK_MS);
  if (scene.fail) {
    emit({ type: "result", subtype: "error_during_execution", is_error: true, result: scene.fail });
    return drain(lines, 1);
  }
  await answer(endpoint, scene);
  return drain(lines, 0);
}
