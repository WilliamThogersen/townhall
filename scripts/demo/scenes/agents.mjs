const read = (file, lines) => ({ tool: "Read", input: { file_path: file }, out: [`Read ${lines} lines`] });
const write = (file, lines) => ({ tool: "Write", input: { file_path: file, content: "" }, out: [`Wrote ${lines} lines to ${file}`] });
const edit = (file, summary, diff = []) => ({
  tool: "Edit",
  input: { file_path: file, old_string: "", new_string: "" },
  out: [`Updated ${file} with ${summary}`, ...diff],
});
const bash = (command, out, extra = {}) => ({ tool: "Bash", input: { command }, out, ...extra });
const grep = (pattern, found) => ({ tool: "Grep", input: { pattern }, out: [`Found ${found} files`] });
const glob = (pattern, found) => ({ tool: "Glob", input: { pattern }, out: [`Found ${found} files`] });
const running = (step) => ({ ...step, running: true });
const failing = (step) => ({ ...step, error: true });

export const AGENTS = {
  "fix-login": {
    kind: "ask",
    minutes: 16,
    steps: [
      read("src/auth/session.ts", 48),
      read("src/auth/redirect.ts", 31),
      edit("src/auth/redirect.ts", "4 additions and 1 removal", [
        "- 13   if (session.expired) return \"/login\";",
        "+ 13   if (session.expired) {",
        "+ 14     clearSession();",
        "+ 15     return \"/login\";",
        "+ 16   }",
      ]),
      bash("npm test -- auth", ["✓ src/auth/redirect.test.ts (6 tests)", "✓ src/auth/session.test.ts (9 tests)"]),
    ],
    say: "commit the fix to townhall/fix-login? (y/n)",
    commit: {
      file: "src/auth/redirect.ts",
      text: "export const clearsSessionBeforeRedirect = true;\n",
      message: "fix: clear the session before redirecting to /login",
    },
  },
  "auth-tests": {
    kind: "work",
    minutes: 15,
    steps: [
      grep("session.expired", 3),
      read("src/auth/redirect.test.ts", 64),
      write("src/auth/expiry.test.ts", 58),
      running(bash("npm test -- auth --watch=false", ["Running…"])),
    ],
  },
  "logout-cookie": {
    kind: "work",
    minutes: 1,
    steps: [
      read("src/auth/session.ts", 48),
      edit("src/auth/session.ts", "3 additions"),
      running(bash("npm test -- session", ["Running…"])),
    ],
  },
  "redirect-guard": {
    kind: "fail",
    minutes: 28,
    exitCode: 1,
    steps: [
      read("src/auth/guard.ts", 22),
      edit("src/auth/guard.ts", "6 additions"),
      failing(bash("npm run typecheck", [
        "src/auth/guard.ts(14,9): error TS2345: Argument of type 'Session | null' is not",
        "assignable to parameter of type 'Session'.",
        "Found 1 error in src/auth/guard.ts:14",
      ])),
    ],
    say: "the typecheck fails on the guard and I cannot fix it without changing the session type. stopping here.",
  },
  "session-ttl": {
    kind: "ship",
    minutes: 22,
    endedSecondsAgo: 40,
    steps: [
      grep("SESSION_TTL", 2),
      read("src/config.ts", 19),
      edit("src/config.ts", "3 additions"),
      edit("src/auth/session.ts", "1 addition and 1 removal"),
      bash("npm test", ["✓ 42 tests passed"]),
      bash("git commit -am \"feat: read the session ttl from config\"", ["[townhall/session-ttl] feat: read the session ttl from config"]),
    ],
    say: "done: the session ttl comes from config.sessionTtlMinutes, 30 by default.",
    commit: {
      file: "src/config.ts",
      text: "export const sessionTtlMinutes = 30;\n",
      message: "feat: read the session ttl from config",
    },
  },
  "e2e-smoke": {
    kind: "fail",
    minutes: 35,
    exitCode: 1,
    steps: [
      read("e2e/smoke.spec.ts", 40),
      edit("e2e/smoke.spec.ts", "2 additions and 1 removal"),
      failing(bash("npx playwright test e2e/smoke.spec.ts", [
        "✘ smoke › home page loads (30.0s)",
        "  TimeoutError: page.goto: Timeout 30000ms exceeded.",
        "1 failed",
      ])),
    ],
    say: "the smoke test still times out on a cold start. stopping so you can look.",
  },
  "ci-cache": {
    kind: "done",
    minutes: 28,
    endedSecondsAgo: 0,
    steps: [
      read(".github/workflows/ci.yml", 54),
      edit(".github/workflows/ci.yml", "7 additions"),
      bash("git push -u origin townhall/ci-cache", ["branch 'townhall/ci-cache' set up to track 'origin/townhall/ci-cache'."]),
      bash("gh pr create --fill", ["https://github.com/example/shop/pull/52"]),
    ],
    say: "pushed. pull request #52 is open and waits for ci.",
  },
  "search-index": {
    kind: "work",
    minutes: 36,
    steps: [
      read("src/search/query.ts", 87),
      bash("psql shop -c \"explain analyze select * from products where name ilike '%chair%'\"", [
        "Seq Scan on products  (cost=0.00..48211.00 rows=1204 width=312)",
        "Execution Time: 3913.118 ms",
      ]),
      write("migrations/0042_products_trgm.sql", 4),
      running(bash("npm run migrate", ["Running…"])),
    ],
  },
  "load-test": {
    kind: "done",
    minutes: 26,
    endedSecondsAgo: 0,
    steps: [
      write("load/search.js", 31),
      bash("k6 run --vus 50 --duration 60s load/search.js", [
        "http_req_duration: avg=212ms p(95)=388ms",
        "http_reqs: 12034 200.5/s",
      ]),
      bash("gh pr create --fill", ["https://github.com/example/shop/pull/57"]),
    ],
    say: "p95 is 388 ms at 200 requests a second. pull request #57 is open for review.",
  },
  "query-plan": {
    kind: "done",
    minutes: 15,
    endedSecondsAgo: 60,
    steps: [
      bash("psql shop -c \"explain (analyze, buffers) select * from products where name ilike '%chair%'\"", [
        "Seq Scan on products  (actual time=0.031..3912.442 rows=1180 loops=1)",
        "Buffers: shared read=36211",
      ]),
      read("src/search/query.ts", 87),
    ],
    say: "the plan is a sequential scan on products: the leading wildcard in ilike '%term%' rules out the btree index. a trigram index fixes it, and search-index is on it.",
  },
  "i18n-keys": {
    kind: "work",
    minutes: 50,
    steps: [
      glob("locales/*.json", 2),
      bash("node scripts/unused-keys.mjs locales/en.json", ["41 unused keys"]),
      edit("locales/en.json", "41 removals"),
      running(bash("node scripts/unused-keys.mjs locales/da.json", ["Running…"])),
    ],
  },
  "coupon-rounding": {
    kind: "ship",
    minutes: 30,
    endedSecondsAgo: 40,
    steps: [
      read("src/cart/total.ts", 46),
      edit("src/cart/total.ts", "2 additions and 2 removals"),
      bash("npm test -- cart", ["✓ src/cart/total.test.ts (12 tests)"]),
    ],
    say: "done: coupon discounts keep øre now, and the cart tests cover 12.50 off.",
    commit: {
      file: "src/cart/total.ts",
      text: "export const roundsCouponToOre = true;\n",
      message: "fix: round coupon discounts to øre",
    },
  },
};

export const DEFAULT_AGENT = {
  kind: "work",
  minutes: 2,
  steps: [glob("src/**/*", 24), read("README.md", 40), running(bash("npm test", ["Running…"]))],
};

export function agentScene(name) {
  return AGENTS[name] ?? DEFAULT_AGENT;
}
