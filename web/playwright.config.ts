import { defineConfig, devices } from "@playwright/test";

export const WEB_PORT = Number(process.env.TOWNHALL_E2E_WEB_PORT ?? 5191);

export default defineConfig({
  testDir: "./e2e",
  testMatch: "*.spec.ts",
  globalSetup: "./e2e/globalSetup.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 60_000,
  expect: { timeout: 15_000 },
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://localhost:${WEB_PORT}`,
    trace: "retain-on-failure",
    viewport: { width: 1400, height: 900 },
  },
});
