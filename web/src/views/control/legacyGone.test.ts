import { describe, expect, it } from "vitest";

const LEGACY_CLASS =
  /className="(app|grid|card|dot|badge|create|confirm|banner|muted|panel|sendbox|terminal|stream-status)\b/;

const sources = import.meta.glob<string>(["/src/**/*.{ts,tsx}", "!/src/**/*.test.{ts,tsx}"], {
  query: "?raw",
  import: "default",
  eager: true,
});

const styles = import.meta.glob("/src/**/*.css");

describe("legacy control center", () => {
  it("is gone from the source tree", () => {
    const paths = Object.keys({ ...sources, ...styles });
    const legacy = paths.filter((path) => /LegacyControlCenter|legacyControlCenter|AgentCard|styles\/legacy\.css/.test(path));
    expect(legacy).toEqual([]);
  });

  it("leaves no legacy class names behind", () => {
    const offenders = Object.entries(sources)
      .filter(([, text]) => LEGACY_CLASS.test(text))
      .map(([path]) => path);
    expect(offenders).toEqual([]);
  });
});
