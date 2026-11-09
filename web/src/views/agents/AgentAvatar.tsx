import { composeAvatar, GRID, type Avatar } from "./avatar";
import { useAvatar } from "./avatarPrefs";

export interface AvatarFaceProps {
  readonly avatar: Avatar;
  /** Pixels per grid cell. Whole numbers only, so every cell stays a sharp square. */
  readonly scale?: number;
  readonly label?: string;
}

function squares(cells: readonly { readonly x: number; readonly y: number }[]): string {
  return cells.map(({ x, y }) => `M${x} ${y}h1v1h-1z`).join("");
}

export function AvatarFace({ avatar, scale = 2, label }: AvatarFaceProps) {
  const cells = composeAvatar(avatar);
  const labelled = label === undefined ? { "aria-hidden": true } : { role: "img", "aria-label": label };
  return (
    <svg
      className={`th-avatar th-avatar--${avatar.color}`}
      viewBox={`0 0 ${GRID.width} ${GRID.height}`}
      width={GRID.width * scale}
      height={GRID.height * scale}
      shapeRendering="crispEdges"
      {...labelled}
    >
      <path data-tone="body" fill="currentColor" d={squares(cells.filter((c) => c.tone === "body"))} />
      <path data-tone="face" className="th-avatar-face" d={squares(cells.filter((c) => c.tone === "face"))} />
    </svg>
  );
}

export interface AgentAvatarProps {
  readonly name: string;
  readonly scale?: number;
  readonly label?: string;
}

export function AgentAvatar({ name, scale, label }: AgentAvatarProps) {
  return <AvatarFace avatar={useAvatar(name)} scale={scale} label={label} />;
}
