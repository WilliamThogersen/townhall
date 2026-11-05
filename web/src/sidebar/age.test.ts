import { describe, expect, it } from "vitest";
import { ageLabel } from "./age";

const NOW = Date.parse("2026-11-04T12:00:00.000Z");
const ago = (ms: number) => new Date(NOW - ms).toISOString();
const MINUTE = 60_000;

describe("ageLabel", () => {
  it.each([
    [0, "now"],
    [59_000, "now"],
    [9 * MINUTE, "9m"],
    [59 * MINUTE, "59m"],
    [3 * 60 * MINUTE, "3h"],
    [2 * 24 * 60 * MINUTE, "2d"],
    [400 * 24 * 60 * MINUTE, "1y"],
  ])("%i ms ago is %s", (ms, label) => {
    expect(ageLabel(ago(ms), NOW)).toBe(label);
  });

  it("reads a time in the future as now and a broken one as nothing", () => {
    expect(ageLabel(ago(-5 * MINUTE), NOW)).toBe("now");
    expect(ageLabel("yesterday", NOW)).toBe("");
  });
});
