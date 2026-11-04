#!/usr/bin/env node
import { runAgent } from "./fake/agent.mjs";
import { runTurn } from "./fake/turn.mjs";

const VERSION = "2.1.318 (Claude Code, townhall demo stand-in)";
const args = process.argv.slice(2);

function run() {
  if (args.includes("--version") || args.includes("-v")) {
    process.stdout.write(`${VERSION}\n`);
    return Promise.resolve(0);
  }
  return args.includes("-p") || args.includes("--print") ? runTurn(args) : runAgent(args);
}

run().then(
  (code) => process.exit(code ?? 0),
  (error) => {
    process.stderr.write(`fake claude: ${error.stack ?? error}\n`);
    process.exit(1);
  },
);
