import type { AgentInfo, Conversation, Repo, Task } from "../api/types";
import { AGENT_STATE_ORDER, AGENT_STATE_WORDS, type AgentState } from "../ui/StatusGlyph";
import { conversationCount, isManagedClone, shortRepoPath } from "./repoPath";

export const MAX_AGENT_GLYPHS = 5;

export interface RepoSummary {
  readonly repo: Repo;
  readonly shortPath: string;
  readonly conversations: string;
  readonly glyphs: readonly AgentState[];
  readonly agents: string;
  readonly spokenAgents: string;
  readonly managed: boolean;
}

export interface RepoSummaryInput {
  readonly home: string | null;
  readonly clonesDir?: string | null;
  readonly conversations: readonly Conversation[];
  readonly tasks: readonly Task[];
  readonly agents: readonly AgentInfo[];
}

const repoKey = (name: string): string => name.toLowerCase();

export function agentGlyph(agent: AgentInfo): AgentState {
  switch (agent.state.kind) {
    case "working":
      return "working";
    case "waiting":
      return "waiting";
    case "error":
      return "error";
    case "ended":
      return agent.alive ? "alive" : "dead";
  }
}

export function agentsLabel(count: number): string {
  if (count === 0) return "no agents";
  return `${count} agent${count === 1 ? "" : "s"}`;
}

function tallyRepos(tasks: readonly Task[]): ReadonlyMap<string, { readonly name: string; readonly count: number }> {
  return tasks.reduce((tally, task) => {
    const key = repoKey(task.repo);
    const seen = tally.get(key);
    return new Map(tally).set(key, { name: seen?.name ?? task.repo, count: (seen?.count ?? 0) + 1 });
  }, new Map<string, { readonly name: string; readonly count: number }>());
}

export function conversationReposOf(conversation: Conversation, tasks: readonly Task[]): readonly string[] {
  if (conversation.linkedRepo !== null) return [conversation.linkedRepo];
  const tally = [...tallyRepos(tasks.filter((task) => task.conversationId === conversation.id)).values()];
  const most = Math.max(0, ...tally.map((entry) => entry.count));
  return tally.filter((entry) => entry.count === most).map((entry) => entry.name);
}

export function conversationCounts(
  conversations: readonly Conversation[],
  tasks: readonly Task[],
): ReadonlyMap<string, number> {
  return conversations.reduce((counts, conversation) => {
    const keys = conversationReposOf(conversation, tasks).map(repoKey);
    return keys.reduce((next, key) => new Map(next).set(key, (next.get(key) ?? 0) + 1), counts);
  }, new Map<string, number>());
}

function byUrgency(a: AgentState, b: AgentState): number {
  return AGENT_STATE_ORDER.indexOf(a) - AGENT_STATE_ORDER.indexOf(b);
}

function spoken(glyphs: readonly AgentState[]): string {
  return AGENT_STATE_ORDER.map((state) => ({ state, count: glyphs.filter((glyph) => glyph === state).length }))
    .filter(({ count }) => count > 0)
    .map(({ state, count }) => `${count} ${AGENT_STATE_WORDS[state]}`)
    .join(", ");
}

export function summarizeRepos(repos: readonly Repo[], input: RepoSummaryInput): readonly RepoSummary[] {
  const counts = conversationCounts(input.conversations, input.tasks);
  return repos.map((repo) => {
    const key = repoKey(repo.name);
    const glyphs = input.agents
      .filter((agent) => repoKey(agent.repo) === key)
      .map(agentGlyph)
      .sort(byUrgency);
    return {
      repo,
      shortPath: shortRepoPath(repo.path, input.home),
      conversations: conversationCount(counts.get(key) ?? 0),
      glyphs: glyphs.slice(0, MAX_AGENT_GLYPHS),
      agents: agentsLabel(glyphs.length),
      spokenAgents: spoken(glyphs),
      managed: isManagedClone(repo.path, input.clonesDir ?? null),
    };
  });
}
