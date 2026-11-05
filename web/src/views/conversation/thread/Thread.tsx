import { useMemo } from "react";
import type { Task, ThreadEntry } from "../../../api/types";
import { EndedNote, ThinkingRow } from "./EventEntries";
import { useThread } from "./threadContext";
import { ThreadEntryView } from "./ThreadEntryView";
import { askAnswers, isNeedsYouOpen, isRepoAskOpen } from "./threadModel";
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

/** The text each answered question got; the `sent` lines that carry it are not shown again. */
function useAnswers(entries: readonly ThreadEntry[]) {
  return useMemo(() => {
    const pairs = askAnswers(entries);
    const hidden = new Set(pairs.values());
    const textOf = (i: number): string | undefined => {
      const at = pairs.get(i);
      const sent = at === undefined ? undefined : entries[at];
      return sent?.kind === "agent" ? sent.text : undefined;
    };
    return { hidden, textOf };
  }, [entries]);
}

export function Thread({ entries, ended }: ThreadProps) {
  const { tasks, thinking } = useThread();
  const open = useMemo(() => openAsks(entries, tasks), [entries, tasks]);
  const answers = useAnswers(entries);
  const tail = thinking ? "thinking" : ended ? "ended" : "";
  const { scrollRef, contentRef, onScroll } = useStickToBottom(entries, tail);
  return (
    <div className="th-thread" ref={scrollRef} onScroll={onScroll} role="log" aria-label="Thread" tabIndex={0}>
      <div className="th-thread-inner" ref={contentRef}>
        {entries.map((entry, i) =>
          answers.hidden.has(i) ? null : (
            <ThreadEntryView key={entry.id} entry={entry} open={open[i] ?? false} answer={answers.textOf(i)} />
          ),
        )}
        {thinking ? <ThinkingRow /> : null}
        {ended && !thinking ? <EndedNote /> : null}
      </div>
    </div>
  );
}
