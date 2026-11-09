import { describe, expect, it } from "vitest";
import {
  AVATAR_PARTS,
  composeAvatar,
  cyclePart,
  defaultAvatar,
  GRID,
  hashName,
  headCells,
  parseAvatar,
  type Avatar,
} from "./avatar";

const sample: Avatar = { head: "square", eyes: "dots", mouth: "flat", accessory: "none", color: "accent" };

const at = (avatar: Avatar, x: number, y: number) => composeAvatar(avatar).find((c) => c.x === x && c.y === y);

describe("hashName", () => {
  it("is stable for the same name", () => {
    expect(hashName("planner")).toBe(hashName("planner"));
  });

  it("ignores case and surrounding space", () => {
    expect(hashName(" Planner ")).toBe(hashName("planner"));
  });

  it("differs between names", () => {
    expect(hashName("planner")).not.toBe(hashName("coder"));
  });

  it("is an unsigned 32 bit integer", () => {
    const hash = hashName("builder");
    expect(Number.isInteger(hash)).toBe(true);
    expect(hash).toBeGreaterThanOrEqual(0);
    expect(hash).toBeLessThan(2 ** 32);
  });
});

describe("defaultAvatar", () => {
  it("is deterministic per name", () => {
    expect(defaultAvatar("planner")).toEqual(defaultAvatar("planner"));
    expect(defaultAvatar("Planner")).toEqual(defaultAvatar("planner"));
  });

  it("only uses known parts", () => {
    for (const name of ["a", "builder", "planner", "dev-server", "x".repeat(40)]) {
      const avatar = defaultAvatar(name);
      expect(AVATAR_PARTS.head).toContain(avatar.head);
      expect(AVATAR_PARTS.eyes).toContain(avatar.eyes);
      expect(AVATAR_PARTS.mouth).toContain(avatar.mouth);
      expect(AVATAR_PARTS.accessory).toContain(avatar.accessory);
      expect(AVATAR_PARTS.color).toContain(avatar.color);
    }
  });

  it("gives a spread of faces over many names", () => {
    const faces = new Set(
      Array.from({ length: 60 }, (_, i) => JSON.stringify(defaultAvatar(`agent-${i}`))),
    );
    expect(faces.size).toBeGreaterThan(40);
  });

  it("pins the hash of a known name (FNV-1a) so defaults never drift", () => {
    expect(hashName("builder")).toBe(3_828_680_000);
  });
});

describe("composeAvatar", () => {
  it("fills the head and cuts the eyes and mouth out of it", () => {
    expect(at(sample, 0, 2)).toEqual({ x: 0, y: 2, tone: "body" });
    expect(at(sample, 2, 4)).toEqual({ x: 2, y: 4, tone: "face" });
    expect(at(sample, 6, 4)).toEqual({ x: 6, y: 4, tone: "face" });
    expect(at(sample, 4, 7)).toEqual({ x: 4, y: 7, tone: "face" });
  });

  it("round heads drop their corners", () => {
    expect(at({ ...sample, head: "round" }, 0, 2)).toBeUndefined();
    expect(at({ ...sample, head: "round" }, 1, 2)).toBeDefined();
  });

  it("changing one part changes only that part", () => {
    const base = composeAvatar(sample);
    const visor = composeAvatar({ ...sample, eyes: "visor" });
    expect(visor).not.toEqual(base);
    const bodyOf = (cells: ReturnType<typeof composeAvatar>) => cells.filter((c) => c.tone === "body" && c.y >= 2);
    expect(bodyOf(visor).length).toBeLessThan(bodyOf(base).length);
    expect(visor.filter((c) => c.y < 2)).toEqual(base.filter((c) => c.y < 2));
  });

  it("draws the accessory above the head", () => {
    expect(composeAvatar(sample).filter((c) => c.y < 2)).toEqual([]);
    expect(at({ ...sample, accessory: "antenna" }, 4, 0)).toEqual({ x: 4, y: 0, tone: "body" });
  });

  it("mouth none draws nothing", () => {
    const withMouth = composeAvatar(sample).filter((c) => c.tone === "face").length;
    const without = composeAvatar({ ...sample, mouth: "none" }).filter((c) => c.tone === "face").length;
    expect(without).toBe(withMouth - 3);
  });

  it("stays inside the grid for every part", () => {
    for (const head of AVATAR_PARTS.head)
      for (const eyes of AVATAR_PARTS.eyes)
        for (const mouth of AVATAR_PARTS.mouth)
          for (const accessory of AVATAR_PARTS.accessory)
            for (const cell of composeAvatar({ head, eyes, mouth, accessory, color: "ok" })) {
              expect(cell.x).toBeGreaterThanOrEqual(0);
              expect(cell.x).toBeLessThan(GRID.width);
              expect(cell.y).toBeGreaterThanOrEqual(0);
              expect(cell.y).toBeLessThan(GRID.height);
            }
  });

  it("only cuts face cells where the head has body", () => {
    for (const head of AVATAR_PARTS.head)
      for (const eyes of AVATAR_PARTS.eyes)
        for (const mouth of AVATAR_PARTS.mouth) {
          const solid = headCells(head);
          const cells = composeAvatar({ head, eyes, mouth, accessory: "none", color: "ok" });
          for (const face of cells.filter((c) => c.tone === "face")) {
            expect(solid.some((c) => c.x === face.x && c.y === face.y)).toBe(true);
          }
        }
  });
});

describe("cyclePart", () => {
  it("steps to the next and previous option", () => {
    expect(cyclePart(sample, "eyes", 1).eyes).toBe(AVATAR_PARTS.eyes[1]);
    expect(cyclePart(sample, "eyes", -1).eyes).toBe(AVATAR_PARTS.eyes[AVATAR_PARTS.eyes.length - 1]);
  });

  it("wraps around and leaves the other parts alone", () => {
    const last = { ...sample, color: AVATAR_PARTS.color[AVATAR_PARTS.color.length - 1] as Avatar["color"] };
    const next = cyclePart(last, "color", 1);
    expect(next.color).toBe(AVATAR_PARTS.color[0]);
    expect({ ...next, color: sample.color }).toEqual(sample);
  });
});

describe("parseAvatar", () => {
  it("accepts a valid avatar", () => {
    expect(parseAvatar(JSON.parse(JSON.stringify(sample)))).toEqual(sample);
  });

  it.each([null, 3, "x", [], {}, { ...sample, head: "cube" }, { ...sample, color: "#f00" }, { ...sample, eyes: 1 }])(
    "rejects %j",
    (value) => {
      expect(parseAvatar(value)).toBeNull();
    },
  );
});
