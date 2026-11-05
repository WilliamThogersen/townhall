import { readFileSync } from "node:fs";

const IMG = new URL("../img/", import.meta.url);
const REDIRECTS = 20;

function harText() {
  const entry = (i) => ({
    startedDateTime: new Date(Date.UTC(2026, 10, 4, 9, 42, 5, i * 40)).toISOString(),
    request: { method: "GET", url: i % 2 === 0 ? "http://localhost:5173/login" : "http://localhost:5173/", headers: [] },
    response: { status: 302, redirectURL: i % 2 === 0 ? "/" : "/login", headers: [] },
    time: 3,
  });
  const log = { version: "1.2", creator: { name: "WebInspector", version: "537.36" }, entries: Array.from({ length: REDIRECTS }, (_, i) => entry(i)) };
  return `${JSON.stringify({ log }, null, 2)}\n`;
}

function slowLogText() {
  const row = (i) => `2026-11-04 09:${String(10 + i).padStart(2, "0")}:41.204 UTC [4121] LOG:  duration: ${3100 + i * 97}.442 ms  statement: select * from products where name ilike '%${["chair", "oak table", "lamp", "sofa"][i % 4]}%' order by rank limit 24`;
  return `${Array.from({ length: 24 }, (_, i) => row(i)).join("\n")}\n`;
}

const image = (file) => () => ({ bytes: readFileSync(new URL(file, IMG)), mime: "image/png" });
const text = (make, mime) => () => ({ bytes: Buffer.from(make(), "utf8"), mime });

const FILES = {
  "redirect-loop.png": image("redirect-loop.png"),
  "network.png": image("network-302.png"),
  "fix-login-after.png": image("login-fixed.png"),
  "localhost.har": text(harText, "application/json"),
  "slow-queries.log": text(slowLogText, "text/plain"),
};

export function demoFile(name) {
  const make = FILES[name];
  if (!make) throw new Error(`no demo file called ${name}`);
  return { name, ...make() };
}
