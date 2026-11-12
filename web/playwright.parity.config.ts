import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "*.shots.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 60_000,
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    baseURL: process.env.PARITY_WEB_URL,
    viewport: { width: 1200, height: 800 },
    deviceScaleFactor: 1,
    reducedMotion: "reduce",
  },
});
