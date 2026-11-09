import type {
  AgentInfo,
  AgentType,
  AgentTypeSummary,
  Artifact,
  Attachment,
  ClaudeUsage,
  Conversation,
  ConversationDetail,
  EntryBody,
  Repo,
  Stats,
  Step,
  Task,
  ThreadEntry,
} from "./types";

export const FIXTURE_TIME = "2026-11-04T10:42:05.000Z";

export const aConversation = (over: Partial<Conversation> = {}): Conversation => ({
  id: "c1",
  sessionId: "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b",
  sessionStarted: false,
  title: "new conversation",
  status: "yourTurn",
  tasks: [],
  linkedRepo: null,
  ended: false,
  context: null,
  createdAt: FIXTURE_TIME,
  updatedAt: FIXTURE_TIME,
  ...over,
});

export const anEntry = (id: number, body: EntryBody, at: string = FIXTURE_TIME): ThreadEntry => ({
  id,
  at,
  ...body,
});

export const aYouEntry = (id: number, text: string): ThreadEntry =>
  anEntry(id, { kind: "you", text, attachments: [] });

export const aTask = (over: Partial<Task> = {}): Task => ({
  name: "fix-login",
  title: "fix the login redirect",
  prompt: "the login page loops",
  repo: "shop",
  baseBranch: "main",
  after: [],
  state: "queued",
  conversationId: "c1",
  attachments: [],
  createdAt: FIXTURE_TIME,
  pr: null,
  agent: "builder",
  blocked: null,
  prBase: null,
  doneWhen: null,
  check: null,
  maxRetries: 2,
  verify: null,
  ...over,
});

export const anAttachment = (over: Partial<Attachment> = {}): Attachment => ({
  id: "a1",
  conversationId: "c1",
  name: "shot.png",
  mime: "image/png",
  size: 1024,
  kind: "image",
  createdAt: FIXTURE_TIME,
  ...over,
});

export const anArtifact = (over: Partial<Artifact> = {}): Artifact => ({
  id: "r1",
  conversationId: "c1",
  task: "fix-login",
  name: "run.log",
  kind: "log",
  mime: "text/plain; charset=utf-8",
  size: 120,
  authorRole: "tester",
  caption: null,
  createdAt: FIXTURE_TIME,
  ...over,
});

export const aDetail = (over: Partial<ConversationDetail> = {}): ConversationDetail => ({
  conversation: aConversation(),
  entries: [],
  tasks: [],
  attachments: [],
  ...over,
});

export const anAgentInfo = (over: Partial<AgentInfo> = {}): AgentInfo => ({
  name: "fix-login",
  kind: "claude",
  branch: "townhall/fix-login",
  worktree: "/w/fix-login",
  alive: true,
  repo: "shop",
  state: { kind: "working" },
  ...over,
});

export const aRepo = (over: Partial<Repo> = {}): Repo => ({
  name: "shop",
  path: "/code/shop",
  baseBranch: "main",
  ...over,
});

export const aStep = (over: Partial<Step> = {}): Step => ({
  id: "s1",
  tool: "Read",
  summary: "src/login.ts",
  startedAt: FIXTURE_TIME,
  endedAt: null,
  result: "running",
  ...over,
});

export const someStats = (): Stats => ({
  days: [{ date: "2026-11-04", fixes: 2, features: 1, lines: 120, agents: 3 }],
});

export const aUsage = (over: Partial<ClaudeUsage> = {}): ClaudeUsage => ({
  session: { usedPercent: 30, resetsAt: Date.UTC(2026, 10, 5, 14, 10), ratePerHour: 12 },
  weekly: { usedPercent: 54, resetsAt: Date.UTC(2026, 10, 11, 7, 0), ratePerHour: 1.125 },
  updatedAt: Date.UTC(2026, 10, 5, 11, 39),
  error: null,
  ...over,
});

export const anAgentTypeSummary = (over: Partial<AgentTypeSummary> = {}): AgentTypeSummary => ({
  name: "builder",
  description: "implements a task, tests first",
  adapter: "claude",
  model: "sonnet",
  permissionMode: "acceptEdits",
  updatedAt: "2026-01-01T00:00:00Z",
  ...over,
});

export const anAgentType = (over: Partial<AgentType> = {}): AgentType => ({
  ...anAgentTypeSummary(),
  tools: [],
  prompt: "You implement the task.",
  command: null,
  canHandBackTo: null,
  env: {},
  source: 'name = "builder"\nadapter = "claude"\n',
  ...over,
});
