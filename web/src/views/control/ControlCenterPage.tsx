import { useRef, useState, type RefObject } from "react";
import type { Task } from "../../api/types";
import { useConnection, useRepos } from "../../data";
import { Link, navigate, PATHS } from "../../router";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { ControlHead } from "./ControlHead";
import { ControlStats } from "./ControlStats";
import { LoopStrip } from "./LoopStrip";
import { RepoFilter } from "./RepoFilter";
import { focusCurrentRow } from "./rowFocus";
import { StepsLoader } from "./StepsLoader";
import type { StepsEntry, TraceRow } from "./traceRows";
import type { ControlFilter } from "./traceScope";
import { TraceTree } from "./TraceTree";
import { controlHref } from "./useControlFilter";
import { useControlKeys } from "./useControlKeys";
import { useControlView } from "./useControlView";
import { useDocumentVisible } from "./useDocumentVisible";
import "./control.css";

type ControlView = ReturnType<typeof useControlView>;

const LIVE_STEPS: ReadonlySet<Task["state"]> = new Set(["working", "needsYou"]);

const openRow = (row: TraceRow) => navigate(row.href);

function StepsLoaders({ tasks, onSteps }: { readonly tasks: readonly Task[]; readonly onSteps: (key: string, entry: StepsEntry) => void }) {
  const visible = useDocumentVisible();
  return tasks
    .filter((task) => task.state !== "queued")
    .map((task) => (
      <StepsLoader key={`${task.repo}/${task.name}`} task={task} live={visible && LIVE_STEPS.has(task.state)} onSteps={onSteps} />
    ));
}

function Empty({ filter, error }: { readonly filter: ControlFilter; readonly error: string | null }) {
  if (error) return <EmptyState title={`error: ${error}`} />;
  if (filter.conversationId) {
    const all = controlHref({ conversationId: null, repo: filter.repo });
    const action = <Link className="th-btn" to={all} replace>all conversations</Link>;
    return <EmptyState title="nothing to show for this conversation" action={action} />;
  }
  const home = <Link className="th-btn" to={PATHS.home}>new conversation</Link>;
  return (
    <EmptyState title="nothing ran today" action={home}>
      {filter.repo ? `in ${filter.repo}` : null}
    </EmptyState>
  );
}

function Timeline({ view, treeRef }: { readonly view: ControlView; readonly treeRef: RefObject<HTMLDivElement | null> }) {
  if (view.loading) {
    return (
      <div className="th-empty">
        <Loading verb="loading timeline" />
      </div>
    );
  }
  if (view.entries.length === 0) return <Empty filter={view.filter} error={view.error} />;
  return (
    <>
      <LoopStrip loop={view.loop} />
      <ControlStats stats={view.stats} />
      <TraceTree rows={view.rows} axis={view.axis} treeRef={treeRef} onOpen={openRow} onToggle={view.toggle} />
    </>
  );
}

function useRepoNames(): readonly string[] {
  const { repos } = useRepos();
  return repos?.map((repo) => repo.name) ?? [];
}

export function ControlCenterPage() {
  const view = useControlView();
  const connection = useConnection();
  const repos = useRepoNames();
  const treeRef = useRef<HTMLDivElement>(null);
  const filterRef = useRef<HTMLButtonElement>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  useControlKeys({
    hasRows: view.rows.length > 0,
    onFocusRows: () => focusCurrentRow(treeRef.current),
    onFilter: () => setMenuOpen(true),
  });
  const { conversationId, repo } = view.filter;
  return (
    <div className="th-page th-control">
      <ControlHead
        caption={view.loading ? null : view.caption}
        connection={connection}
        allConversationsHref={conversationId ? controlHref({ conversationId: null, repo }) : null}
      >
        <RepoFilter
          repos={repos}
          value={repo}
          open={menuOpen}
          buttonRef={filterRef}
          onOpenChange={setMenuOpen}
          onPick={view.setRepo}
        />
      </ControlHead>
      <div className="th-trace">
        <StepsLoaders tasks={view.shownTasks} onSteps={view.record} />
        <Timeline view={view} treeRef={treeRef} />
      </div>
    </div>
  );
}
