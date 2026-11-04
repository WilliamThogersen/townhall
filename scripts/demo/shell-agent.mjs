#!/usr/bin/env node
const LINES_EVERY_MS = 3500;
const ROUTES = [
  ["GET", "/api/search?q=chair&page=1", 200],
  ["GET", "/api/products/1042", 200],
  ["POST", "/api/cart/items", 201],
  ["GET", "/api/search?q=oak+table", 200],
  ["GET", "/api/cart", 200],
  ["POST", "/api/checkout/coupon", 422],
  ["GET", "/assets/index-4f2a1c9.js", 304],
];
const BANNER = [
  "",
  "> shop@2.4.0 dev",
  "> vite --port 4100",
  "",
  "",
  "  VITE v8.3.5  ready in 412 ms",
  "",
  "  ➜  Local:   http://localhost:4100/",
  "  ➜  Network: use --host to expose",
  "  ➜  press h + enter to show help",
  "",
];

const clock = () => new Date().toTimeString().slice(0, 8);
const pick = (n) => ROUTES[n % ROUTES.length];

function line(n) {
  const [method, path, status] = pick(n);
  const ms = 18 + ((n * 37) % 380);
  return `${clock()} ${method} ${path} ${status} in ${ms}ms`;
}

process.stdout.write(`${BANNER.join("\n")}\n`);
Array.from({ length: 4 }, (_, n) => process.stdout.write(`${line(n)}\n`));
const tick = (n) => setTimeout(() => {
  process.stdout.write(`${line(n)}\n`);
  tick(n + 1);
}, LINES_EVERY_MS);
tick(4);
