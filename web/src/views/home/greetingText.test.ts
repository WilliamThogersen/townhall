import { describe, expect, it } from "vitest";
import { firstName, greetingText, timeOfDay } from "./greetingText";

const at = (hour: number) => new Date(2026, 10, 4, hour, 30);

describe("timeOfDay", () => {
  it("is morning from 5 to noon", () => {
    expect(timeOfDay(at(5))).toBe("morning");
    expect(timeOfDay(at(11))).toBe("morning");
  });

  it("is afternoon from noon to 18", () => {
    expect(timeOfDay(at(12))).toBe("afternoon");
    expect(timeOfDay(at(17))).toBe("afternoon");
  });

  it("is evening from 18 through the night", () => {
    expect(timeOfDay(at(18))).toBe("evening");
    expect(timeOfDay(at(23))).toBe("evening");
    expect(timeOfDay(at(2))).toBe("evening");
  });
});

describe("firstName", () => {
  it("takes the first word and keeps its capitals", () => {
    expect(firstName("William Thøgersen")).toBe("William");
    expect(firstName("  McKenzie  Lee ")).toBe("McKenzie");
  });

  it("is null without a usable name", () => {
    expect(firstName(null)).toBeNull();
    expect(firstName(undefined)).toBeNull();
    expect(firstName("   ")).toBeNull();
  });
});

describe("greetingText", () => {
  it("greets by first name", () => {
    expect(greetingText(at(14), "William Thøgersen")).toBe("good afternoon, William");
  });

  it("greets without a name", () => {
    expect(greetingText(at(9), null)).toBe("good morning");
  });
});
