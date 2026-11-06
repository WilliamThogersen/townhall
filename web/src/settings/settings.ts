export type PermissionMode = "default" | "acceptEdits" | "plan" | "auto" | "bypassPermissions";
export type RepoUnclear = "ask" | "guess";
export type NotifyWhen = "needsYou" | "failed" | "everything";
export type TownhallModel = "sonnet" | "opus" | "haiku";

export interface Settings {
  readonly permissionMode: PermissionMode;
  readonly workWithoutAsking: boolean;
  readonly agentsAtOnce: number;
  readonly repoUnclear: RepoUnclear;
  readonly sessionPanelOpen: boolean;
  readonly notifyWhen: NotifyWhen;
  readonly townhallModel: TownhallModel;
  readonly githubAccount: string;
}

export const PERMISSION_MODES: readonly PermissionMode[] = [
  "default",
  "acceptEdits",
  "plan",
  "auto",
  "bypassPermissions",
];
export const REPO_UNCLEAR_VALUES: readonly RepoUnclear[] = ["ask", "guess"];
export const NOTIFY_VALUES: readonly NotifyWhen[] = ["needsYou", "failed", "everything"];
export const TOWNHALL_MODELS: readonly TownhallModel[] = ["sonnet", "opus", "haiku"];

export const AGENTS_AT_ONCE_MIN = 1;
export const AGENTS_AT_ONCE_MAX = 50;

export const DEFAULT_SETTINGS: Settings = Object.freeze({
  permissionMode: "acceptEdits",
  workWithoutAsking: false,
  agentsAtOnce: 6,
  repoUnclear: "ask",
  sessionPanelOpen: true,
  notifyWhen: "needsYou",
  townhallModel: "sonnet",
  githubAccount: "",
});

export type SettingKey = keyof Settings;

export const SETTING_KEYS: readonly SettingKey[] = Object.freeze([
  "permissionMode",
  "workWithoutAsking",
  "agentsAtOnce",
  "repoUnclear",
  "sessionPanelOpen",
  "notifyWhen",
  "townhallModel",
  "githubAccount",
]);

type Fields = Readonly<Record<string, unknown>>;
type Check<T> = (value: unknown) => value is T;

export function isFields(raw: unknown): raw is Fields {
  return typeof raw === "object" && raw !== null && !Array.isArray(raw);
}

function choiceOf<T extends string>(choices: readonly T[]): Check<T> {
  return (value): value is T => choices.some((choice) => choice === value);
}

const isFlag: Check<boolean> = (value): value is boolean => typeof value === "boolean";

const isAgentsAtOnce: Check<number> = (value): value is number =>
  typeof value === "number" &&
  Number.isInteger(value) &&
  value >= AGENTS_AT_ONCE_MIN &&
  value <= AGENTS_AT_ONCE_MAX;

const GITHUB_LOGIN = /^(?!-)[A-Za-z0-9-]{0,39}$/;

const isGithubAccount: Check<string> = (value): value is string =>
  typeof value === "string" && GITHUB_LOGIN.test(value);

const CHECKS: { readonly [K in SettingKey]: Check<Settings[K]> } = {
  permissionMode: choiceOf(PERMISSION_MODES),
  workWithoutAsking: isFlag,
  agentsAtOnce: isAgentsAtOnce,
  repoUnclear: choiceOf(REPO_UNCLEAR_VALUES),
  sessionPanelOpen: isFlag,
  notifyWhen: choiceOf(NOTIFY_VALUES),
  townhallModel: choiceOf(TOWNHALL_MODELS),
  githubAccount: isGithubAccount,
};

export function isSettingValue<K extends SettingKey>(key: K, value: unknown): value is Settings[K] {
  const check: Check<Settings[K]> = CHECKS[key];
  return check(value);
}

function pick<K extends SettingKey>(fields: Fields, key: K): Settings[K] {
  const value = fields[key];
  return isSettingValue(key, value) ? value : DEFAULT_SETTINGS[key];
}

export function clampAgentsAtOnce(value: number): number {
  return Math.min(AGENTS_AT_ONCE_MAX, Math.max(AGENTS_AT_ONCE_MIN, Math.round(value)));
}

export function parseSettings(raw: unknown): Settings {
  const fields: Fields = isFields(raw) ? raw : {};
  return Object.freeze({
    permissionMode: pick(fields, "permissionMode"),
    workWithoutAsking: pick(fields, "workWithoutAsking"),
    agentsAtOnce: pick(fields, "agentsAtOnce"),
    repoUnclear: pick(fields, "repoUnclear"),
    sessionPanelOpen: pick(fields, "sessionPanelOpen"),
    notifyWhen: pick(fields, "notifyWhen"),
    townhallModel: pick(fields, "townhallModel"),
    githubAccount: pick(fields, "githubAccount"),
  });
}
