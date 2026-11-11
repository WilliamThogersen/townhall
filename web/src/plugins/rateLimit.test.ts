import { describe, expect, it } from "vitest";
import { createRateLimiter } from "./rateLimit";

describe("createRateLimiter", () => {
  it("lets the limit through in one window and trips on the next message", () => {
    const now = 0;
    const limiter = createRateLimiter(3, 1000, () => now);
    expect([limiter(), limiter(), limiter()]).toEqual([true, true, true]);
    expect(limiter()).toBe(false);
  });

  it("opens again once the window has moved on", () => {
    let now = 0;
    const limiter = createRateLimiter(2, 1000, () => now);
    limiter();
    limiter();
    expect(limiter()).toBe(false);
    now = 1001;
    expect(limiter()).toBe(true);
  });

  it("counts a sliding window, not a fixed one", () => {
    let now = 0;
    const limiter = createRateLimiter(2, 1000, () => now);
    limiter();
    now = 900;
    limiter();
    now = 1100;
    expect(limiter()).toBe(true);
    expect(limiter()).toBe(false);
  });
});
