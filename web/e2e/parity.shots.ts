import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";

interface DemoJson {
  readonly conversations: readonly { readonly key: string; readonly id: string }[];
}

interface Scene {
  readonly name: string;
  readonly size: string;
  readonly web?: boolean;
}

const need = (name: string): string => {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set: run scripts/parity.sh`);
  return value;
};

const OUT = need("PARITY_OUT");
const PROXY = need("PARITY_PROXY");
const DEMO = JSON.parse(readFileSync(join(need("PARITY_DEMO_DIR"), "demo.json"), "utf8")) as DemoJson;
const SCENES = JSON.parse(readFileSync(join(import.meta.dirname, "../../scripts/parity/scenes.json"), "utf8")) as Scene[];
const WANTED = new Set((process.env.PARITY_SCENES ?? "").split(",").filter(Boolean));
const THEMES = ["dark", "light"] as const;

const idOf = (key: string): string => {
  const found = DEMO.conversations.find((c) => c.key === key);
  if (!found) throw new Error(`demo.json has no conversation ${key}`);
  return found.id;
};

async function open(page: Page, path: string, theme: string): Promise<void> {
  await page.addInitScript((choice) => window.localStorage.setItem("th-theme", choice), theme);
  await page.route(/\/api\/(usage|system)$/, async (route) => {
    const path = new URL(route.request().url()).pathname;
    await route.fulfill({ response: await route.fetch({ url: `${PROXY}${path}` }) });
  });
  await page.goto(path);
  await page.locator(".th-shell").waitFor();
}

async function settled(page: Page): Promise<void> {
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready);
  await page.mouse.move(0, 0);
}

async function conversation(page: Page, key: string, theme: string): Promise<void> {
  await open(page, `/c/${idOf(key)}`, theme);
  await expect(page.locator(".th-convo-head-title")).toBeVisible();
}

async function blurText(page: Page): Promise<void> {
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
}

type Capture = (page: Page, theme: string) => Promise<void>;

const CAPTURES: Record<string, Capture> = {
  home: (page, theme) => open(page, "/", theme),
  sidebar: (page, theme) => conversation(page, "flaky", theme),
  conversation: (page, theme) => conversation(page, "login", theme),
  "session-panel": async (page, theme) => {
    await conversation(page, "login", theme);
    await blurText(page);
    await page.keyboard.press("s");
    await expect(page.getByRole("complementary", { name: "Session" })).toBeVisible();
  },
  "row-menu": async (page, theme) => {
    await conversation(page, "login", theme);
    await page.locator(".th-convo").first().click({ button: "right" });
    await expect(page.getByRole("menu")).toBeVisible();
  },
  "menu-drawer": async (page, theme) => {
    await open(page, "/", theme);
    await blurText(page);
    await page.keyboard.press("m");
    await expect(page.locator(".th-shell")).toHaveAttribute("data-view", "side");
  },
  "usage-meter": async (page, theme) => {
    await open(page, "/", theme);
    await expect(page.getByRole("button", { name: "Claude plan usage" })).toBeVisible();
    await page.getByRole("button", { name: "Claude plan usage" }).hover();
  },
  "usage-open": async (page, theme) => {
    await open(page, "/", theme);
    await page.getByRole("button", { name: "Claude plan usage" }).click();
    await expect(page.getByRole("dialog", { name: "Claude plan usage" })).toBeVisible();
  },
  "system-stats": async (page, theme) => {
    await open(page, "/", theme);
    await expect(page.locator(".th-keybar-system")).toContainText("cpu 23%");
  },
  settings: (page, theme) => open(page, "/settings", theme),
  repos: (page, theme) => open(page, "/repos", theme),
  agents: (page, theme) => open(page, "/agents", theme),
  control: (page, theme) => open(page, "/townhall", theme),
  heatmap: async (page, theme) => {
    await open(page, "/", theme);
    await page.locator(".th-heat-grid").first().scrollIntoViewIfNeeded();
  },
  terminal: async (page, theme) => {
    await conversation(page, "login", theme);
    await blurText(page);
    await page.keyboard.press("s");
    await page.locator("[data-task]").first().click();
    await expect(page.locator(".th-pane").first()).toBeVisible();
    await page.waitForTimeout(2000);
  },
  "daemon-down": async (page, theme) => {
    await page.route("**/api/**", (route) => route.abort());
    await page.addInitScript((choice) => window.localStorage.setItem("th-theme", choice), theme);
    await page.goto("/");
    await expect(page.getByText("cannot reach the daemon")).toBeVisible();
  },
  narrow: (page, theme) => conversation(page, "login", theme),
};

for (const scene of SCENES) {
  for (const theme of THEMES) {
    const name = theme === "dark" ? scene.name : `${scene.name}-light`;
    const run = CAPTURES[scene.name];
    const skipped = scene.web === false || run === undefined || (WANTED.size > 0 && !WANTED.has(name));
    test(`shot ${name}`, async ({ page }) => {
      test.skip(skipped, "not requested or not reachable in a browser");
      const [width, height] = scene.size.split("x").map(Number) as [number, number];
      await page.setViewportSize({ width, height });
      await run?.(page, theme);
      if (scene.name !== "daemon-down") await settled(page);
      await page.screenshot({ path: join(OUT, `${name}.web.png`) });
    });
  }
}
