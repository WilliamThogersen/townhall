import { useCallback, useMemo, useState } from "react";
import { useAgentInfos, useConversations, useTasks } from "../../data";
import { loopSummary } from "../../tasks/stage";
import { timeAxis } from "./timeAxis";
import { buildRows, latestStep, toggleKey } from "./traceRows";
import { earliestStart, scopeTrace } from "./traceScope";
import { headCaption, traceStats } from "./traceStats";
import { useControlFilter } from "./useControlFilter";
import { useNow } from "./useNow";
import { useStepsBook } from "./useStepsBook";

const NO_TOGGLES: ReadonlySet<string> = new Set();

function useTimeline() {
  const { filter, setRepo } = useControlFilter();
  const now = useNow();
  const conversations = useConversations();
  const tasks = useTasks();
  const entries = useMemo(
    () => scopeTrace(conversations.conversations, tasks.tasks, filter, now),
    [conversations.conversations, tasks.tasks, filter, now],
  );
  const axis = useMemo(() => timeAxis(earliestStart(entries, now), now), [entries, now]);
  const loading = (conversations.loading || tasks.loading) && entries.length === 0;
  return { filter, setRepo, entries, axis, loading, error: conversations.error ?? tasks.error };
}

export function useControlView() {
  const timeline = useTimeline();
  const { entries, axis, filter } = timeline;
  const { agents } = useAgentInfos();
  const { book, record } = useStepsBook();
  const [toggled, setToggled] = useState<ReadonlySet<string>>(NO_TOGGLES);
  const shownTasks = useMemo(() => entries.flatMap((entry) => entry.tasks), [entries]);
  const rows = useMemo(() => buildRows({ entries, book, agents, toggled, axis }), [entries, book, agents, toggled, axis]);
  const loop = useMemo(() => loopSummary(shownTasks, (name) => latestStep(book, name)), [shownTasks, book]);
  const stats = useMemo(() => traceStats(entries, book, axis.now), [entries, book, axis.now]);
  const toggle = useCallback((key: string) => setToggled((prev) => toggleKey(prev, key)), []);
  return {
    ...timeline,
    shownTasks,
    rows,
    loop,
    stats,
    caption: headCaption(entries, filter.conversationId, axis),
    toggle,
    record,
  };
}
