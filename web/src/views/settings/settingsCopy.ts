import type { ForgeKind, NotifyWhen, RepoUnclear } from "../../settings/settings";
import type { ThemeChoice } from "../../theme/theme";

export interface Choice<T extends string> {
  readonly value: T;
  readonly label: string;
  readonly tone?: "danger";
}

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
  { value: "off", label: "off" },
  { value: "needsYou", label: "needs you" },
  { value: "failed", label: "+ failed" },
  { value: "everything", label: "+ landed" },
];

export const FORGE_CHOICES: readonly Choice<ForgeKind>[] = [
  { value: "github", label: "github" },
  { value: "gitlab", label: "gitlab" },
];

export const PERMISSION_TIMEOUT_CHOICES: readonly Choice<string>[] = [
  { value: "30", label: "30 s" },
  { value: "60", label: "1 min" },
  { value: "120", label: "2 min" },
  { value: "300", label: "5 min" },
];

export const HELP = {
  agentsAtOnce: "the rest wait as queued and start when one finishes.",
  adviserPermissions:
    "when an agent asks to run a command, the adviser decides: allow, deny with a reason, or hand it to you. secrets, deletes outside the worktree, force pushes to main and dropping data always come to you.",
  permissionSuggestions:
    "the adviser allowed these three times or more for the same agent type. adding one saves it on that type's tools; it applies from the type's next launch.",
  permissionLog: "what the adviser decided, newest first. hard-deny and ask-user came to you.",
  permissionTimeout: "if the adviser has not answered by then, the prompt comes to you.",
  repoUnclear: "ask shows the candidates as buttons. guess picks the best match and says why.",
  sessionPanel: "open in new conversations. you can always fold it with s.",
  clonesDir:
    "where a repo added by link is cloned, as <folder>/<repo>. you can pick another folder each time you add one. empty uses ~/.townhall/repos.",
  forgeAccounts:
    "the gh and glab logins townhall may use. no token is stored. pick one per repo on the repos page, else the first for the host is used.",
  theme: "dark is the default.",
  pluginTheme: "a theme from an enabled plugin, drawn over the built in colours. none keeps the built in look.",
  notifyWhen: "desktop notifications. off sends none; each step adds to the one before it.",
  learnAboutMe:
    "while on, townhall reads your profile in its own conversations: how you work, your tone of voice, your typical flow. stored locally, edit or reset it below at any time.",
  profileOff: "not used while learn about me is off.",
  profileOn: "townhall reads this in its own conversations.",
} as const;
