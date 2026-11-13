export type RepoUnclear = "ask" | "guess";
export type NotifyWhen = "off" | "needsYou" | "failed" | "everything";

export type ForgeKind = "github" | "gitlab";

export interface ForgeAccount {
  readonly forge: ForgeKind;
  readonly login: string;
  readonly host: string;
  readonly label: string;
}

export interface Settings {
  readonly agentsAtOnce: number;
  readonly repoUnclear: RepoUnclear;
  readonly sessionPanelOpen: boolean;
  readonly notifyWhen: NotifyWhen;
  readonly minFreeDiskGb: number;
  readonly learnAboutMe: boolean;
  readonly githubAccount: string;
  readonly forgeAccounts: readonly ForgeAccount[];
  readonly reflectAfterTasks: boolean;
  readonly clonesDir: string;
  readonly adviserPermissions: boolean;
  readonly permissionTimeoutSecs: number;
}

export const REPO_UNCLEAR_VALUES: readonly RepoUnclear[] = ["ask", "guess"];
export const NOTIFY_VALUES: readonly NotifyWhen[] = ["off", "needsYou", "failed", "everything"];
export const FORGE_KINDS: readonly ForgeKind[] = ["github", "gitlab"];
export const FORGE_ACCOUNTS_MAX = 50;

export const AGENTS_AT_ONCE_MIN = 1;
export const AGENTS_AT_ONCE_MAX = 50;
export const DISK_GB_MAX = 100_000;
export const PERMISSION_TIMEOUT_MIN = 5;
export const PERMISSION_TIMEOUT_MAX = 600;

export const DEFAULT_SETTINGS: Settings = Object.freeze({
  agentsAtOnce: 6,
  repoUnclear: "ask",
  sessionPanelOpen: true,
  notifyWhen: "everything",
  minFreeDiskGb: 5,
  learnAboutMe: false,
  githubAccount: "",
  forgeAccounts: Object.freeze([]),
  reflectAfterTasks: false,
  clonesDir: "",
  adviserPermissions: true,
  permissionTimeoutSecs: 60,
});

export type SettingKey = keyof Settings;

export const SETTING_KEYS: readonly SettingKey[] = Object.freeze([
  "agentsAtOnce",
  "repoUnclear",
  "sessionPanelOpen",
  "notifyWhen",
  "minFreeDiskGb",
  "learnAboutMe",
  "githubAccount",
  "forgeAccounts",
  "reflectAfterTasks",
  "clonesDir",
  "adviserPermissions",
  "permissionTimeoutSecs",
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

const isDiskGb: Check<number> = (value): value is number =>
  typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= DISK_GB_MAX;

const isPermissionTimeout: Check<number> = (value): value is number =>
  typeof value === "number" &&
  Number.isInteger(value) &&
  value >= PERMISSION_TIMEOUT_MIN &&
  value <= PERMISSION_TIMEOUT_MAX;

const GITHUB_LOGIN = /^(?!-)[A-Za-z0-9-]{0,39}$/;

const isGithubAccount: Check<string> = (value): value is string =>
  typeof value === "string" && GITHUB_LOGIN.test(value);

export const isClonesDirText = (value: string): boolean =>
  value === "" || value === "~" || value.startsWith("~/") || value.startsWith("/");

const isClonesDir: Check<string> = (value): value is string => typeof value === "string" && isClonesDirText(value);

const ACCOUNT_KEYS = ["forge", "login", "host", "label"];

function isForgeAccount(value: unknown): value is ForgeAccount {
  if (!isFields(value)) return false;
  const { forge, login, host, label } = value;
  return (
    Object.keys(value).every((key) => ACCOUNT_KEYS.includes(key)) &&
    choiceOf(FORGE_KINDS)(forge) &&
    typeof login === "string" &&
    login !== "" &&
    typeof host === "string" &&
    host !== "" &&
    typeof label === "string"
  );
}

const isForgeAccounts: Check<readonly ForgeAccount[]> = (value): value is readonly ForgeAccount[] =>
  Array.isArray(value) && value.length <= FORGE_ACCOUNTS_MAX && value.every(isForgeAccount);

const CHECKS: { readonly [K in SettingKey]: Check<Settings[K]> } = {
  agentsAtOnce: isAgentsAtOnce,
  repoUnclear: choiceOf(REPO_UNCLEAR_VALUES),
  sessionPanelOpen: isFlag,
  notifyWhen: choiceOf(NOTIFY_VALUES),
  minFreeDiskGb: isDiskGb,
  learnAboutMe: isFlag,
  githubAccount: isGithubAccount,
  forgeAccounts: isForgeAccounts,
  reflectAfterTasks: isFlag,
  clonesDir: isClonesDir,
  adviserPermissions: isFlag,
  permissionTimeoutSecs: isPermissionTimeout,
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
    agentsAtOnce: pick(fields, "agentsAtOnce"),
    repoUnclear: pick(fields, "repoUnclear"),
    sessionPanelOpen: pick(fields, "sessionPanelOpen"),
    notifyWhen: pick(fields, "notifyWhen"),
    minFreeDiskGb: pick(fields, "minFreeDiskGb"),
    learnAboutMe: pick(fields, "learnAboutMe"),
    githubAccount: pick(fields, "githubAccount"),
    forgeAccounts: pick(fields, "forgeAccounts"),
    reflectAfterTasks: pick(fields, "reflectAfterTasks"),
    clonesDir: pick(fields, "clonesDir"),
    adviserPermissions: pick(fields, "adviserPermissions"),
    permissionTimeoutSecs: pick(fields, "permissionTimeoutSecs"),
  });
}
