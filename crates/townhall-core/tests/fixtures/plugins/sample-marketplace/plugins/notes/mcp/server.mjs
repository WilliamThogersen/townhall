import { createInterface } from "node:readline";

const NOTES = ["write the changelog line", "ask before deleting a branch"];

const TOOLS = [
  {
    name: "search_notes",
    description: "Find saved notes that contain a word",
    inputSchema: {
      type: "object",
      properties: { query: { type: "string" } },
      required: ["query"],
    },
  },
];

function answer(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
}

function fail(id, message) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, error: { code: -32601, message } })}\n`);
}

function search(query) {
  const found = NOTES.filter((note) => note.includes(String(query ?? "")));
  return { content: [{ type: "text", text: found.join("\n") || "no notes" }] };
}

function handle(message) {
  const { id, method, params } = message;
  if (id === undefined) return;
  if (method === "initialize") {
    answer(id, {
      protocolVersion: params?.protocolVersion ?? "2024-11-05",
      capabilities: { tools: {} },
      serverInfo: { name: "notes", version: "1.0.0" },
    });
  } else if (method === "tools/list") {
    answer(id, { tools: TOOLS });
  } else if (method === "tools/call" && params?.name === "search_notes") {
    answer(id, search(params.arguments?.query));
  } else {
    fail(id, `unknown method ${method}`);
  }
}

createInterface({ input: process.stdin }).on("line", (line) => {
  if (line.trim() !== "") handle(JSON.parse(line));
});
