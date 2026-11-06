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
}

export interface Repo {
  readonly name: string;
  readonly path: string;
  readonly baseBranch: string;
}

export interface AddRepoInput {
  readonly path: string;
  readonly name?: string;
  readonly baseBranch?: string;
}

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

export type SettingKey = keyof Settings;

export interface PutSettingInput<K extends SettingKey = SettingKey> {
  readonly value: Settings[K];
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
}

export interface CreateTaskInput {
  readonly name: string;
  readonly title: string;
  readonly prompt: string;
  readonly repo?: string;
  readonly baseBranch?: string;
  readonly after?: readonly string[];
  readonly attachments?: readonly string[];
  readonly agent?: string;
}

export type AgentAdapterKind = "claude" | "shell";

export type AgentTypeMode = "default" | "acceptEdits" | "plan";

export interface AgentTypeSummary {
  readonly name: string;
  readonly description: string;
  readonly adapter: AgentAdapterKind;
  readonly model: string | null;
  readonly permissionMode: AgentTypeMode | null;
  readonly updatedAt: string;
}

export interface AgentUsage {
  readonly agent: string;
  readonly runs: number;
  readonly tokens: number;
}

export interface AgentType {
  readonly name: string;
  readonly description: string;
  readonly adapter: AgentAdapterKind;
  readonly model: string | null;
  readonly permissionMode: AgentTypeMode | null;
  readonly tools: readonly string[];
  readonly prompt: string | null;
  readonly command: string | null;
  readonly canHandBackTo: readonly string[] | null;
  readonly source: string;
  readonly updatedAt: string;
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
