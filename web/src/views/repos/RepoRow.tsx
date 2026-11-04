import { countByState } from "../../agents/agentState";
import type { Agent, Info } from "../../api/types";
import { conversationCount, repoName, shortRepoPath, userHomeOf } from "../../repos/repoPath";
import { Link, PATHS } from "../../router";
import { GlyphCounts } from "../../ui/StatusGlyph";

function AgentSummary({ agents }: { readonly agents: readonly Agent[] }) {
  if (agents.length === 0) {
    return (
      <div className="th-repo-meta th-repo-agents">
        <span>no agents</span>
      </div>
    );
  }
  return (
    <div className="th-repo-meta th-repo-agents">
      <GlyphCounts counts={countByState(agents)} className="th-nav-count" />
      <span>{`${agents.length} agent${agents.length === 1 ? "" : "s"}`}</span>
    </div>
  );
}

export interface RepoRowProps {
  readonly info: Info;
  readonly agents: readonly Agent[];
}

export function RepoRow({ info, agents }: RepoRowProps) {
  return (
    <div className="th-repo">
      <div className="th-repo-name">
        <b>{repoName(info.repo)}</b>
        <span title={info.repo}>{shortRepoPath(info.repo, userHomeOf(info.home))}</span>
      </div>
      <div className="th-repo-meta th-repo-count">
        <span>{conversationCount(0)}</span>
      </div>
      <AgentSummary agents={agents} />
      <Link className="th-btn th-btn--ghost th-btn--sm" to={PATHS.home}>
        new conversation
      </Link>
    </div>
  );
}
