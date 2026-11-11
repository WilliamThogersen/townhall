import type { AvatarSpec } from "../views/agents/avatarSpec";

export interface Agent {
  readonly name: string;
  readonly kind: string | null;
  readonly branch: string;
  readonly worktree: string;
  readonly alive: boolean;
}

export type AgentState =
  | { readonly kind: "working" }
  | { readonly kind: "waiting"; readonly question: string }
  | { readonly kind: "error"; readonly exitCode: number | null }
  | { readonly kind: "ended" };

export interface AgentInfo extends Agent {
  readonly repo: string;
  readonly state: AgentState;
}

export type AgentKey = "up" | "down" | "enter" | "escape" | "tab";

export type SendInput =
  | { readonly text: string; readonly key?: never }
  | { readonly key: AgentKey; readonly text?: never };

export interface Info {
  readonly home: string;
  readonly repo: string;
  readonly repoKey: string;
  readonly userName?: string | null;
  readonly version?: string | null;
}

export interface Repo {
  readonly name: string;
  readonly path: string;
  readonly baseBranch: string;
  readonly githubAccount: string | null;
  readonly gitlabAccount: string | null;
}

export interface PutRepoAccountsInput {
  readonly githubAccount?: string | null;
  readonly gitlabAccount?: string | null;
}

export interface AddRepoInput {
  readonly path: string;
  readonly name?: string;
  readonly baseBranch?: string;
  readonly cloneDir?: string;
}

export interface PatchRepoInput {
  readonly name?: string;
  readonly baseBranch?: string;
  readonly path?: string;
  readonly moveFolder?: boolean;
}

export type RepoUnclear = "ask" | "guess";
export type NotifyWhen = "needsYou" | "failed" | "everything";
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
}

export type SettingKey = keyof Settings;

export interface PutSettingInput<K extends SettingKey = SettingKey> {
  readonly value: Settings[K];
}

export interface Profile {
  readonly text: string;
}

export interface PutProfileInput {
  readonly text: string;
}

export type ConversationStatus = "thinking" | "yourTurn";

export interface ContextUsage {
  readonly used: number;
  readonly limit: number;
}

export interface Conversation {
  readonly id: string;
  readonly sessionId: string;
  readonly sessionStarted: boolean;
  readonly title: string;
  readonly status: ConversationStatus;
  readonly tasks: readonly string[];
  readonly linkedRepo: string | null;
  readonly ended: boolean;
  readonly context: ContextUsage | null;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface NewMessageInput {
  readonly text: string;
  readonly attachments?: readonly string[];
  readonly replyTo?: number;
}

export interface ConversationPatch {
  readonly linkedRepo?: string | null;
  readonly title?: string;
}

export type RepoRule = "named" | "code" | "only" | "picked" | "linked";
export type AgentEvent = "sent" | "passed" | "failed" | "ended" | "allowed";

export type EntryBody =
  | {
      readonly kind: "you";
      readonly text: string;
      readonly attachments: readonly string[];
      readonly replyTo?: number;
    }
  | { readonly kind: "townhall"; readonly text: string }
  | {
      readonly kind: "repo";
      readonly repo: string;
      readonly baseBranch: string;
      readonly rule: RepoRule;
      readonly reason: string;
    }
  | {
      readonly kind: "repoAsk";
      readonly question: string;
      readonly candidates: readonly string[];
      readonly offerAll: boolean;
    }
  | { readonly kind: "tasksCreated"; readonly tasks: readonly string[] }
  | {
      readonly kind: "needsYou";
      readonly agent: string;
      readonly question: string;
      readonly choices: readonly string[];
    }
  | {
      readonly kind: "agent";
      readonly agent: string;
      readonly event: AgentEvent;
      readonly text: string;
    }
  | { readonly kind: "event"; readonly text: string }
  | {
      readonly kind: "handoff";
      readonly task: string;
      readonly from: string;
      readonly to: string;
      readonly reason: string;
    }
  | { readonly kind: "plugin"; readonly plugin: string; readonly text: string }
  | { readonly kind: "error"; readonly message: string };

export type ThreadEntry = { readonly id: number; readonly at: string } & EntryBody;

export type TaskState =
  | "queued"
  | "working"
  | "needsYou"
  | "inCi"
  | "inReview"
  | "merged"
  | "ended"
  | "error";

export type Checks = "pending" | "passed" | "failed" | "none";

export interface Pr {
  readonly number: number;
  readonly url: string | null;
  readonly checks: Checks;
}

export type VerifyPhase = "verifying" | "retrying" | "passed" | "failed";

export interface Verify {
  readonly phase: VerifyPhase;
  readonly retries: number;
  readonly output: string | null;
}

export interface Task {
  readonly name: string;
  readonly title: string;
  readonly prompt: string;
  readonly repo: string;
  readonly baseBranch: string;
  readonly after: readonly string[];
  readonly state: TaskState;
  readonly conversationId: string;
  readonly attachments: readonly string[];
  readonly createdAt: string;
  readonly pr: Pr | null;
  readonly agent: string;
  readonly blocked: string | null;
  readonly prBase: string | null;
  readonly doneWhen: string | null;
  readonly check: string | null;
  readonly maxRetries: number;
  readonly verify: Verify | null;
}

export interface CreateTaskInput {
  readonly name: string;
  readonly title: string;
  readonly prompt: string;
  readonly repo?: string;
  readonly baseBranch?: string;
  readonly prBase?: string;
  readonly doneWhen?: string;
  readonly check?: string;
  readonly maxRetries?: number;
  readonly after?: readonly string[];
  readonly attachments?: readonly string[];
  readonly agent?: string;
}

export type AgentAdapterKind = "claude" | "shell";

export type AgentTypeMode = "default" | "acceptEdits" | "plan" | "bypassPermissions";

export type AgentTypeEffort = "low" | "medium" | "high" | "xhigh" | "max";

export interface AgentTypeSummary {
  readonly name: string;
  readonly description: string;
  readonly adapter: AgentAdapterKind;
  readonly model: string | null;
  readonly effort?: AgentTypeEffort | null;
  readonly permissionMode: AgentTypeMode | null;
  readonly avatar: AvatarSpec | null;
  readonly updatedAt: string;
}

export interface AgentUsage {
  readonly agent: string;
  readonly runs: number;
  readonly tokens: number;
}

export type UsageRange = "24h" | "7d" | "30d";

export interface UsageBucket {
  readonly start: string;
  readonly runs: number;
  readonly tokens: number;
}

export interface UsageShare {
  readonly key: string;
  readonly runs: number;
  readonly tokens: number;
}

export interface AgentUsageSeries {
  readonly agent: string;
  readonly range: UsageRange;
  readonly bucketMs: number;
  readonly since: string;
  readonly until: string;
  readonly runs: number;
  readonly tokens: number;
  readonly buckets: readonly UsageBucket[];
  readonly byTask: readonly UsageShare[];
  readonly byRepo: readonly UsageShare[];
  readonly byConversation: readonly UsageShare[];
}

export type AgentRunKind = "task" | "turn" | "consult";

export interface AgentRun {
  readonly kind: AgentRunKind;
  readonly conversationId: string;
  readonly task: string;
  readonly repo: string;
  readonly state: string;
  readonly model: string | null;
  readonly tokens: number;
  readonly createdAt: string;
  readonly endedAt: string | null;
}

export interface AgentType {
  readonly name: string;
  readonly description: string;
  readonly adapter: AgentAdapterKind;
  readonly model: string | null;
  readonly effort?: AgentTypeEffort | null;
  readonly permissionMode: AgentTypeMode | null;
  readonly tools: readonly string[];
  readonly prompt: string | null;
  readonly command: string | null;
  readonly canHandBackTo: readonly string[] | null;
  readonly avatar: AvatarSpec | null;
  readonly env: Readonly<Record<string, string>>;
  readonly source: string;
  readonly updatedAt: string;
  readonly origin?: "user" | "plugin";
  readonly plugin?: string | null;
}

export type MarketplaceSource =
  | { readonly source: "github"; readonly repo: string }
  | { readonly source: "url"; readonly url: string }
  | { readonly source: "directory"; readonly path: string };

export interface MarketplaceEntry {
  readonly name: string;
  readonly description: string;
  readonly version: string | null;
  readonly category: string | null;
}

export interface Marketplace {
  readonly name: string;
  readonly source: MarketplaceSource;
  readonly account: string | null;
  readonly enabled: boolean;
  readonly official: boolean;
  readonly headSha: string | null;
  readonly fetchedAt: string | null;
  readonly error: string | null;
  readonly plugins: readonly MarketplaceEntry[];
}

export interface AddMarketplaceInput {
  readonly source: string;
  readonly name?: string;
  readonly account?: string;
}

export interface Plugin {
  readonly id: string;
  readonly marketplace: string;
  readonly name: string;
  readonly version: string | null;
  readonly sha: string;
  readonly enabled: boolean;
  readonly pinned: boolean;
  readonly permissions: readonly string[];
  readonly permissionsHash: string;
  readonly installedAt: string;
  readonly updatedAt: string;
  readonly error: string | null;
}

export interface AvailablePlugin {
  readonly id: string;
  readonly marketplace: string;
  readonly name: string;
  readonly description: string;
  readonly version: string | null;
  readonly category: string | null;
  readonly installed: boolean;
}

export interface PluginList {
  readonly available: readonly AvailablePlugin[];
  readonly installed: readonly Plugin[];
}

export interface PluginPreview {
  readonly plugin: string;
  readonly marketplace: string;
  readonly name: string;
  readonly version: string | null;
  readonly sha: string;
  readonly permissions: readonly string[];
  readonly permissionsHash: string;
}

export type PluginUpdateOutcome = "unchanged" | "updated" | "pinned" | "needsApproval";

export interface PluginUpdate {
  readonly outcome: PluginUpdateOutcome;
  readonly sha: string | null;
  readonly added: readonly string[];
  readonly permissionsHash: string | null;
  readonly plugin: Plugin;
}

export interface PatchPluginInput {
  readonly enabled?: boolean;
  readonly pinned?: boolean;
}

export interface PluginStarter {
  readonly plugin: string;
  readonly title: string;
  readonly text: string;
}

export interface PluginThemeChoice {
  readonly plugin: string;
  readonly name: string;
}

export interface ActivePluginTheme {
  readonly plugin: string;
  readonly css: string;
}

export type SettingFieldType = "boolean" | "string" | "integer" | "enum";

export type SettingFieldValue = boolean | string | number;

export interface PluginSettingField {
  readonly key: string;
  readonly label: string;
  readonly description: string | null;
  readonly type: SettingFieldType;
  readonly default: SettingFieldValue;
  readonly min: number | null;
  readonly max: number | null;
  readonly choices: readonly string[] | null;
  readonly secret: boolean;
  readonly value: SettingFieldValue | null;
  readonly set: boolean;
}

export interface PluginSettingsGroup {
  readonly plugin: string;
  readonly name: string;
  readonly fields: readonly PluginSettingField[];
}

export interface PluginContributions {
  readonly starters: readonly PluginStarter[];
  readonly themes: readonly PluginThemeChoice[];
  readonly theme: ActivePluginTheme | null;
  readonly settings: readonly PluginSettingsGroup[];
}

export interface DefinitionError {
  readonly kind: "agent" | "workflow";
  readonly name: string | null;
  readonly step: string | null;
  readonly field: string | null;
  readonly message: string;
  readonly line: number | null;
}

export type AttachmentKind = "image" | "file";

export interface Attachment {
  readonly id: string;
  readonly conversationId: string;
  readonly name: string;
  readonly mime: string;
  readonly size: number;
  readonly kind: AttachmentKind;
  readonly createdAt: string;
}

export type ArtifactKind = "screenshot" | "log" | "note" | "report";
export type AuthorRole = "coder" | "tester" | "reviewer" | "other";

export interface Artifact {
  readonly id: string;
  readonly conversationId: string;
  readonly task: string;
  readonly name: string;
  readonly kind: ArtifactKind;
  readonly mime: string;
  readonly size: number;
  readonly authorRole: AuthorRole;
  readonly caption: string | null;
  readonly createdAt: string;
}

export interface ConversationDetail {
  readonly conversation: Conversation;
  readonly entries: readonly ThreadEntry[];
  readonly tasks: readonly Task[];
  readonly attachments: readonly Attachment[];
}

export type ServerEvent =
  | { readonly type: "conversationUpdated"; readonly conversation: Conversation }
  | {
      readonly type: "entryAppended";
      readonly conversationId: string;
      readonly entry: ThreadEntry;
    }
  | { readonly type: "taskUpdated"; readonly task: Task }
  | { readonly type: "agentUpdated"; readonly agent: AgentInfo }
  | { readonly type: "resync" };

export type StepResult = "running" | "ok" | "error";

export interface Step {
  readonly id: string;
  readonly tool: string;
  readonly summary: string;
  readonly startedAt: string;
  readonly endedAt: string | null;
  readonly result: StepResult;
}

export interface ShippedDay {
  readonly date: string;
  readonly fixes: number;
  readonly features: number;
  readonly lines: number;
  readonly agents: number;
}

export interface Stats {
  readonly days: readonly ShippedDay[];
}

export interface UsageWindow {
  readonly usedPercent: number;
  readonly resetsAt: number | null;
  readonly ratePerHour: number | null;
}

export interface ClaudeUsage {
  readonly session: UsageWindow | null;
  readonly weekly: UsageWindow | null;
  readonly updatedAt: number;
  readonly error: string | null;
}

export interface SystemStats {
  readonly cpuPercent: number;
  readonly memoryUsedBytes: number;
  readonly memoryTotalBytes: number;
  readonly diskUsedBytes: number;
  readonly diskTotalBytes: number;
}
