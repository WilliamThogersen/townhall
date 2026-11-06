import type { NotifyWhen, PermissionMode, RepoUnclear } from "../../settings/settings";
import type { ThemeChoice } from "../../theme/theme";

export interface Choice<T extends string> {
  readonly value: T;
  readonly label: string;
  readonly tone?: "danger";
}

export const DANGEROUS_MODE: PermissionMode = "bypassPermissions";

export const PERMISSION_CHOICES: readonly Choice<PermissionMode>[] = [
  { value: "default", label: "ask" },
  { value: "acceptEdits", label: "accept edits" },
  { value: "plan", label: "plan" },
  { value: "auto", label: "auto" },
  { value: DANGEROUS_MODE, label: "skip all", tone: "danger" },
];

export const PERMISSION_HELP: Readonly<Record<PermissionMode, string>> = {
  default: "agents ask before every edit or command.",
  acceptEdits: "edits go through, commands ask first.",
  plan: "agents plan first and change nothing.",
  auto: "agents decide what needs your approval.",
  bypassPermissions: "agents never ask. only use on repos you trust.",
};

export const REPO_UNCLEAR_CHOICES: readonly Choice<RepoUnclear>[] = [
  { value: "ask", label: "ask" },
  { value: "guess", label: "guess" },
];

export const THEME_CHOICES: readonly Choice<ThemeChoice>[] = [
  { value: "dark", label: "dark" },
  { value: "light", label: "light" },
  { value: "system", label: "system" },
];

export const NOTIFY_CHOICES: readonly Choice<NotifyWhen>[] = [
  { value: "needsYou", label: "needs you" },
  { value: "failed", label: "+ failed" },
  { value: "everything", label: "everything" },
];

export const HELP = {
  workWithoutAsking:
    "agents assume and list their assumptions instead of asking. they still stop for missing access or destructive actions.",
  agentsAtOnce: "the rest wait as queued and start when one finishes.",
  repoUnclear: "ask shows the candidates as buttons. guess picks the best match and says why.",
  sessionPanel: "open in new conversations. you can always fold it with s.",
  theme: "dark is the default.",
  notifyWhen: "desktop notifications and toasts.",
  learnAboutMe:
    "while on, townhall reads your profile in its own conversations: how you work, your tone of voice, your typical flow. stored locally, edit or reset it below at any time.",
  profileOff: "not used while learn about me is off.",
  profileOn: "townhall reads this in its own conversations.",
} as const;
