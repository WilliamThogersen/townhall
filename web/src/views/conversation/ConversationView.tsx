import { useCallback, useRef } from "react";
import type { ConversationDetail } from "../../api/types";
import { DropZone, useFileDrop } from "../../attachments";
import { Composer, type ComposerHandle } from "../../composer";
import { writeReply, type ReplyTarget } from "../../composer/replies";
import { useRepos } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { ConversationHeader } from "./ConversationHeader";
import { conversationRepo, repoText } from "./conversationRepo";
import { SessionPanel, TaskTerminal } from "./session";
import { Thread } from "./thread/Thread";
import { ThreadContext } from "./thread/threadContext";
import { useConversationKeys } from "./useConversationKeys";
import { useCommands, usePaneRoute, useRail, useThreadValue, type Commands } from "./useConversationView";
import "./conversationPage.css";

export interface ConversationViewProps {
  readonly id: string;
  readonly detail: ConversationDetail;
}

function Problem({ commands }: { readonly commands: Commands }) {
  if (commands.problem === null) return null;
  const dismiss = (
    <Button variant="ghost" size="sm" onClick={commands.dismiss}>
      dismiss
    </Button>
  );
  return (
    <Banner level="error" action={dismiss}>
      {commands.problem}
    </Banner>
  );
}

export function ConversationView({ id, detail }: ConversationViewProps) {
  const pane = usePaneRoute(id, detail.tasks);
  const rail = useRail(pane);
  const commands = useCommands(id);
  const composer = useRef<ComposerHandle>(null);
  const onReply = useCallback(
    (target: ReplyTarget) => {
      writeReply(id, target);
      composer.current?.focus();
    },
    [id],
  );
  const thread = useThreadValue(detail, pane, commands, onReply);
  const { repos } = useRepos();
  const drop = useFileDrop((files) => composer.current?.addFiles(files));
  useConversationKeys({
    onToggleSession: rail.toggle,
    onFocusComposer: () => composer.current?.focus(),
    panelKeys: rail.visible && detail.tasks.length > 0,
    terminalOpen: pane.task !== null,
  });
  const { conversation } = detail;
  return (
    <div
      className="th-convo-layout"
      data-pane={pane.task === null ? undefined : "true"}
      data-rail={rail.attribute}
    >
      <section className="th-convo-view" aria-label={conversation.title} {...drop.dropProps}>
        <ConversationHeader
          detail={detail}
          repo={repoText(conversationRepo(detail, repos))}
          railOpen={rail.visible}
          onToggleRail={rail.toggle}
          onStop={commands.stop}
          onEnd={commands.end}
        />
        <Problem commands={commands} />
        <ThreadContext value={thread}>
          <Thread key={id} entries={detail.entries} ended={conversation.ended} />
        </ThreadContext>
        <Composer
          ref={composer}
          conversationId={id}
          busy={conversation.status === "thinking"}
          onStop={() => void commands.stop()}
          onSubmit={commands.submit}
        />
        {drop.dragging ? <DropZone /> : null}
      </section>
      {pane.task === null ? (
        <SessionPanel detail={detail} openTask={pane.wanted} onOpenTask={pane.open} />
      ) : (
        <TaskTerminal key={pane.task.name} task={pane.task} onClose={pane.close} />
      )}
    </div>
  );
}
