import { useRef } from "react";
import type { ConversationDetail } from "../../api/types";
import { useRepos } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { ConversationHeader } from "./ConversationHeader";
import { conversationRepo, repoText } from "./conversationRepo";
import { SessionPanel, TaskTerminal } from "./session";
import { Thread } from "./thread/Thread";
import { ThreadContext } from "./thread/threadContext";
import { useConversationKeys } from "./useConversationKeys";
import { useCommands, usePaneRoute, useRail, useThreadValue } from "./useConversationView";
import "./conversationPage.css";

export interface ConversationViewProps {
  readonly id: string;
  readonly detail: ConversationDetail;
}

export function ConversationView({ id, detail }: ConversationViewProps) {
  const pane = usePaneRoute(id, detail.tasks);
  const rail = useRail(pane);
  const commands = useCommands(id);
  const thread = useThreadValue(detail, pane, commands);
  const { repos } = useRepos();
  const viewRef = useRef<HTMLElement>(null);
  const focusComposer = () => viewRef.current?.querySelector<HTMLTextAreaElement>(".th-composer-input")?.focus();
  useConversationKeys({ onToggleSession: rail.toggle, onFocusComposer: focusComposer });
  const dismiss = (
    <Button variant="ghost" size="sm" onClick={commands.dismiss}>
      dismiss
    </Button>
  );
  return (
    <div
      className="th-convo-layout"
      data-pane={pane.task === null ? undefined : "true"}
      data-rail={rail.folded ? "closed" : undefined}
    >
      <section className="th-convo-view" aria-label={detail.conversation.title} ref={viewRef}>
        <ConversationHeader
          detail={detail}
          repo={repoText(conversationRepo(detail, repos))}
          railOpen={rail.visible}
          onToggleRail={rail.toggle}
          onStop={commands.stop}
          onEnd={commands.end}
        />
        {commands.problem === null ? null : (
          <Banner level="error" action={dismiss}>
            {commands.problem}
          </Banner>
        )}
        <ThreadContext value={thread}>
          <Thread key={id} entries={detail.entries} ended={detail.conversation.ended} />
        </ThreadContext>
      </section>
      {pane.task === null ? (
        <SessionPanel detail={detail} openTask={pane.wanted} onOpenTask={pane.open} />
      ) : (
        <TaskTerminal key={pane.task.name} task={pane.task} onClose={pane.close} />
      )}
    </div>
  );
}
