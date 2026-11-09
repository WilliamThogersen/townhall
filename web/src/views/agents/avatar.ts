export const GRID = { width: 9, height: 9 } as const;

const HEAD_TOP = 2;

export const AVATAR_PARTS = {
  head: ["square", "round", "domed", "tapered"],
  eyes: ["dots", "tall", "wide", "visor", "cyclops"],
  mouth: ["flat", "smile", "open", "grill", "none"],
  accessory: ["none", "antenna", "twin", "horns", "crest"],
  color: ["accent", "ok", "info", "fg", "fg-muted"],
} as const;

export type AvatarPart = keyof typeof AVATAR_PARTS;

export type Avatar = { readonly [P in AvatarPart]: (typeof AVATAR_PARTS)[P][number] };

export interface AvatarCell {
  readonly x: number;
  readonly y: number;
  /** body is drawn in the avatar color, face is cut out of it. */
  readonly tone: "body" | "face";
}

type Point = readonly [x: number, y: number];

const HEADS: Readonly<Record<Avatar["head"], readonly string[]>> = {
  square: ["#########", "#########", "#########", "#########", "#########", "#########", "#########"],
  round: [".#######.", "#########", "#########", "#########", "#########", "#########", ".#######."],
  domed: ["..#####..", ".#######.", "#########", "#########", "#########", "#########", "#########"],
  tapered: ["#########", "#########", "#########", "#########", "#########", ".#######.", "..#####.."],
};

const row = (y: number, from: number, to: number): readonly Point[] =>
  Array.from({ length: to - from + 1 }, (_, i): Point => [from + i, y]);

const EYES: Readonly<Record<Avatar["eyes"], readonly Point[]>> = {
  dots: [[2, 4], [6, 4]],
  tall: [[2, 3], [2, 4], [6, 3], [6, 4]],
  wide: [[1, 4], [2, 4], [6, 4], [7, 4]],
  visor: row(4, 1, 7),
  cyclops: [...row(3, 3, 5), ...row(4, 3, 5)],
};

const MOUTHS: Readonly<Record<Avatar["mouth"], readonly Point[]>> = {
  flat: row(7, 3, 5),
  smile: [[2, 6], [6, 6], ...row(7, 3, 5)],
  open: [...row(6, 3, 5), ...row(7, 3, 5)],
  grill: [[2, 7], [3, 6], [4, 7], [5, 6], [6, 7]],
  none: [],
};

const ACCESSORIES: Readonly<Record<Avatar["accessory"], readonly Point[]>> = {
  none: [],
  antenna: [[4, 0], [4, 1]],
  twin: [[1, 0], [1, 1], [7, 0], [7, 1]],
  horns: [[1, 0], [2, 1], [7, 0], [6, 1]],
  crest: [[4, 0], ...row(1, 3, 5)],
};

const FNV_OFFSET = 0x811c9dc5;
const FNV_PRIME = 0x01000193;

/** FNV-1a over the lowercased, trimmed name, as an unsigned 32 bit integer. */
export function hashName(name: string): number {
  let hash = FNV_OFFSET;
  for (const byte of new TextEncoder().encode(name.trim().toLowerCase())) {
    hash = Math.imul(hash ^ byte, FNV_PRIME);
  }
  return hash >>> 0;
}

function mix(hash: number, salt: number): number {
  let h = Math.imul(hash ^ Math.imul(salt + 1, 0x9e3779b1), 0x85ebca6b);
  h ^= h >>> 13;
  h = Math.imul(h, 0xc2b2ae35);
  return (h ^ (h >>> 16)) >>> 0;
}

const PART_ORDER = Object.keys(AVATAR_PARTS) as readonly AvatarPart[];

function pick<P extends AvatarPart>(part: P, hash: number): Avatar[P] {
  const options: readonly string[] = AVATAR_PARTS[part];
  return options[mix(hash, PART_ORDER.indexOf(part)) % options.length] as Avatar[P];
}

export function defaultAvatar(name: string): Avatar {
  const hash = hashName(name);
  return {
    head: pick("head", hash),
    eyes: pick("eyes", hash),
    mouth: pick("mouth", hash),
    accessory: pick("accessory", hash),
    color: pick("color", hash),
  };
}

export function headCells(head: Avatar["head"]): readonly AvatarCell[] {
  return HEADS[head].flatMap((line, dy) =>
    [...line].flatMap((ch, x) => (ch === "#" ? [{ x, y: HEAD_TOP + dy, tone: "body" as const }] : [])),
  );
}

function gridCells(grid: readonly string[]): readonly AvatarCell[] {
  return grid.flatMap((line, y) =>
    [...line].flatMap((ch, x) => (ch === "#" ? [{ x, y, tone: "body" as const }] : ch === "o" ? [{ x, y, tone: "face" as const }] : [])),
  );
}

export function composeAvatar(avatar: Avatar, grid: readonly string[] | null = null): readonly AvatarCell[] {
  if (grid !== null) return gridCells(grid);
  const face = new Set([...EYES[avatar.eyes], ...MOUTHS[avatar.mouth]].map(([x, y]) => `${x},${y}`));
  const head = headCells(avatar.head).map((cell) =>
    face.has(`${cell.x},${cell.y}`) ? { ...cell, tone: "face" as const } : cell,
  );
  const accessory = ACCESSORIES[avatar.accessory].map(([x, y]) => ({ x, y, tone: "body" as const }));
  return [...accessory, ...head];
}

export function cyclePart(avatar: Avatar, part: AvatarPart, step: 1 | -1): Avatar {
  const options: readonly string[] = AVATAR_PARTS[part];
  const index = options.indexOf(avatar[part]);
  const next = options[(index + step + options.length) % options.length];
  return { ...avatar, [part]: next };
}

export function parseAvatar(value: unknown): Avatar | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  const valid = PART_ORDER.every((part) => (AVATAR_PARTS[part] as readonly unknown[]).includes(record[part]));
  return valid ? (Object.fromEntries(PART_ORDER.map((part) => [part, record[part]])) as unknown as Avatar) : null;
}
