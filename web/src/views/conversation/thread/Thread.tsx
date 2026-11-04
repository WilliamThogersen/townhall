import { useMemo } from "react";
import type { Task, ThreadEntry } from "../../../api/types";
import { EndedNote, ThinkingRow } from "./EventEntries";
import { useThread } from "./threadContext";
import { ThreadEntryView } from "./ThreadEntryView";
import { isNeedsYouOpen, isRepoAskOpen } from "./threadModel";
import { useStickToBottom } from "./useStickToBottom";

export interface ThreadProps {
  readonly entries: readonly ThreadEntry[];
  readonly ended: boolean;
}

function openAsks(entries: readonly ThreadEntry[], tasks: readonly Task[]): readonly boolean[] {
  return entries.map((entry, i) => {
    if (entry.kind === "needsYou") return isNeedsYouOpen(entries, i, tasks);
    if (entry.kind === "repoAsk") return isRepoAskOpen(entries, i);
    return false;
  });
}

export function Thread({ entries, ended }: ThreadProps) {
  const { tasks, thinking } = useThread();
  const open = useMemo(() => openAsks(entries, tasks), [entries, tasks]);
  const tail = thinking ? "thinking" : ended ? "ended" : "";
  const { scrollRef, contentRef, onScroll } = useStickToBottom(entries, tail);
  return (
    <div className="th-thread" ref={scrollRef} onScroll={onScroll} role="log" aria-label="Thread" tabIndex={0}>
      <div className="th-thread-inner" ref={contentRef}>
        {entries.map((entry, i) => (
          <ThreadEntryView key={entry.id} entry={entry} open={open[i] ?? false} />
        ))}
        {thinking ? <ThinkingRow /> : null}
        {ended && !thinking ? <EndedNote /> : null}
      </div>
    </div>
  );
}
