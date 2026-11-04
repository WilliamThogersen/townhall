export type PermissionMode = "default" | "acceptEdits" | "plan" | "auto" | "bypassPermissions";
export type RepoUnclear = "ask" | "guess";
export type NotifyWhen = "needsYou" | "failed" | "everything";

export interface Settings {
  readonly permissionMode: PermissionMode;
  readonly workWithoutAsking: boolean;
  readonly agentsAtOnce: number;
  readonly repoUnclear: RepoUnclear;
  readonly sessionPanelOpen: boolean;
  readonly notifyWhen: NotifyWhen;
}

export const PERMISSION_MODES: readonly PermissionMode[] = [
  "default",
  "acceptEdits",
  "plan",
  "auto",
  "bypassPermissions",
];
export const REPO_UNCLEAR_CHOICES: readonly RepoUnclear[] = ["ask", "guess"];
export const NOTIFY_CHOICES: readonly NotifyWhen[] = ["needsYou", "failed", "everything"];

export const AGENTS_AT_ONCE_MIN = 1;
export const AGENTS_AT_ONCE_MAX = 50;

export const DEFAULT_SETTINGS: Settings = Object.freeze({
  permissionMode: "acceptEdits",
  workWithoutAsking: false,
  agentsAtOnce: 6,
  repoUnclear: "ask",
  sessionPanelOpen: true,
  notifyWhen: "needsYou",
});

type Fields = Readonly<Record<string, unknown>>;

function isFields(raw: unknown): raw is Fields {
  return typeof raw === "object" && raw !== null && !Array.isArray(raw);
}

function oneOf<T extends string>(choices: readonly T[], value: unknown, fallback: T): T {
  return choices.find((choice) => choice === value) ?? fallback;
}

function flag(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function agentsAtOnce(value: unknown, fallback: number): number {
  const valid =
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= AGENTS_AT_ONCE_MIN &&
    value <= AGENTS_AT_ONCE_MAX;
  return valid ? value : fallback;
}

export function clampAgentsAtOnce(value: number): number {
  return Math.min(AGENTS_AT_ONCE_MAX, Math.max(AGENTS_AT_ONCE_MIN, Math.round(value)));
}

export function parseSettings(raw: unknown): Settings {
  const fields: Fields = isFields(raw) ? raw : {};
  const d = DEFAULT_SETTINGS;
  return Object.freeze({
    permissionMode: oneOf(PERMISSION_MODES, fields.permissionMode, d.permissionMode),
    workWithoutAsking: flag(fields.workWithoutAsking, d.workWithoutAsking),
    agentsAtOnce: agentsAtOnce(fields.agentsAtOnce, d.agentsAtOnce),
    repoUnclear: oneOf(REPO_UNCLEAR_CHOICES, fields.repoUnclear, d.repoUnclear),
    sessionPanelOpen: flag(fields.sessionPanelOpen, d.sessionPanelOpen),
    notifyWhen: oneOf(NOTIFY_CHOICES, fields.notifyWhen, d.notifyWhen),
  });
}
