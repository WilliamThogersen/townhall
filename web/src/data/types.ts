import type {
  AddRepoInput,
  AgentInfo,
  ClaudeUsage,
  Conversation,
  ConversationDetail,
  ConversationPatch,
  NewMessageInput,
  Repo,
  Stats,
  Step,
  Task,
  ThreadEntry,
} from "../api/types";

export type { ConnectionStatus } from "../api/events";

export interface Loadable {
  readonly loading: boolean;
  readonly error: string | null;
}

export interface ConversationsState extends Loadable {
  readonly conversations: readonly Conversation[];
}

export interface ConversationState extends Loadable {
  readonly detail: ConversationDetail | null;
  readonly reload: () => void;
}

export interface TaskFilter {
  readonly conversationId?: string;
  readonly repo?: string;
}

export interface TasksState extends Loadable {
  readonly tasks: readonly Task[];
}

export interface AgentInfosState extends Loadable {
  readonly agents: readonly AgentInfo[];
}

export interface ReposState extends Loadable {
  readonly repos: readonly Repo[] | null;
  readonly add: (input: AddRepoInput) => Promise<Repo>;
  readonly remove: (name: string, deleteClone?: boolean) => Promise<void>;
}

export interface StatsState extends Loadable {
  readonly stats: Stats | null;
}

export interface UsageState extends Loadable {
  readonly usage: ClaudeUsage | null;
}

export interface StepsOptions {
  readonly live?: boolean;
  readonly repo?: string;
  readonly intervalMs?: number;
}

export interface StepsState extends Loadable {
  readonly steps: readonly Step[];
}

export interface ConversationActions {
  readonly start: (input: NewMessageInput, files?: readonly File[]) => Promise<ConversationDetail>;
  readonly send: (id: string, input: NewMessageInput) => Promise<ThreadEntry>;
  readonly stop: (id: string) => Promise<Conversation>;
  readonly end: (id: string) => Promise<Conversation>;
  readonly patch: (id: string, patch: ConversationPatch) => Promise<Conversation>;
}
