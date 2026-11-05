const DAY = 24 * 60;

export const CONVERSATIONS = [
  {
    key: "login",
    minutesAgo: 1,
    spanMinutes: 58,
    messages: [
      {
        text: "townhall's login page redirects forever after the session expires. fix it, and add tests so it can't come back",
        files: ["redirect-loop.png", "network.png", "localhost.har"],
      },
      { text: "also read the session ttl from config, and stop /login redirecting people who are signed in" },
    ],
    looseFiles: ["fix-login-after.png"],
    sends: [{ agent: "auth-tests", text: "cover the remember-me cookie too" }],
  },
  {
    key: "vite",
    minutesAgo: 2,
    spanMinutes: 3,
    messages: [{ text: "upgrade townhall to vite 8 and fix whatever breaks", until: "thinking" }],
  },
  {
    key: "flaky",
    minutesAgo: 9,
    spanMinutes: 48,
    messages: [{ text: "ci on shop main fails about every third run, the e2e smoke test times out. find out why" }],
  },
  {
    key: "search",
    minutesAgo: 14,
    spanMinutes: 38,
    messages: [
      { text: "search api is slow under load, p95 is 4 s. src/search/query.ts looks suspicious" },
      { text: "here is the slow query log from production", files: ["slow-queries.log"] },
    ],
    sends: [{ agent: "search-index", text: "the slow query log is in the conversation files" }],
  },
  {
    key: "i18n",
    minutesAgo: 62,
    spanMinutes: 20,
    messages: [{ text: "clean up unused i18n keys in wexo-access, en and da" }],
  },
  {
    key: "export",
    minutesAgo: 3 * 60,
    spanMinutes: 1,
    title: "nightly export drops rows",
    messages: [{ text: "the nightly export from shop into data-hub drops rows since monday", until: "asked" }],
  },
  {
    key: "coupon",
    minutesAgo: 2 * DAY,
    spanMinutes: 35,
    end: true,
    messages: [{ text: "coupons on the shop checkout round the discount down to whole kroner, they should keep øre" }],
  },
  {
    key: "contrast",
    minutesAgo: 5 * DAY,
    spanMinutes: 12,
    end: true,
    messages: [
      { text: "is the dark mode contrast in townhall good enough for the muted text?" },
      { text: "check the light theme too" },
    ],
  },
];

export const TASK_GOALS = {
  "fix-login": { state: "needsYou" },
  "auth-tests": { state: "working" },
  "logout-cookie": { state: "queued" },
  "redirect-guard": { state: "error", pr: { number: 41, checks: "failed" } },
  "session-ttl": { state: "merged", merge: true, pr: { number: 38, checks: "passed" } },
  "e2e-smoke": { state: "error" },
  "ci-cache": { state: "inCi", set: true, pr: { number: 52, checks: "pending" } },
  "search-index": { state: "working" },
  "load-test": { state: "inReview", set: true, pr: { number: 57, checks: "passed" } },
  "query-plan": { state: "ended" },
  "i18n-keys": { state: "working" },
  "coupon-rounding": { state: "merged", merge: true, pr: { number: 33, checks: "passed" } },
};

export const PR_URL_BASE = "https://github.com/example";
