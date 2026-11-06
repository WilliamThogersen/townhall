import { useMemo } from "react";
import type { Task, ThreadEntry } from "../../../api/types";
import type { ReplyTarget } from "../../../composer/replies";
import { EndedNote, ThinkingRow } from "./EventEntries";
import { useThread } from "./threadContext";
import { ThreadEntryView } from "./ThreadEntryView";
import { askAnswers, isNeedsYouOpen, isRepoAskOpen, replyTarget } from "./threadModel";
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

/** What the entry a reply answers said; null when it is gone, hidden, or has nothing to quote. */
function quoteOf(entries: readonly ThreadEntry[], hidden: ReadonlySet<number>, entry: ThreadEntry): ReplyTarget | null {
  if (entry.kind !== "you" || entry.replyTo === undefined) return null;
  const at = entries.findIndex((other) => other.id === entry.replyTo);
  const original = entries[at];
  return original === undefined || hidden.has(at) ? null : replyTarget(original);
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
            <ThreadEntryView
              key={entry.id}
              entry={entry}
              open={open[i] ?? false}
              answer={answers.textOf(i)}
              quote={quoteOf(entries, answers.hidden, entry)}
            />
          ),
        )}
        {thinking ? <ThinkingRow /> : null}
        {ended && !thinking ? <EndedNote /> : null}
      </div>
    </div>
  );
}
