import { useCallback, useMemo, useState } from "react";
import { sendText } from "../../api/client";
import { toMessage } from "../../api/errors";
import type { ConversationDetail, Task } from "../../api/types";
import { useConversationActions } from "../../data";
import { navigate, PATHS, useRoute } from "../../router";
import { useSettings } from "../../settings/useSettings";
import type { ThreadContextValue } from "./thread/threadContext";

export interface PaneRoute {
  readonly wanted: string | null;
  readonly task: Task | null;
  readonly open: (name: string) => void;
  readonly close: () => void;
}

export function usePaneRoute(id: string, tasks: readonly Task[]): PaneRoute {
  const wanted = useRoute().query.t ?? null;
  const task = wanted === null ? null : (tasks.find((t) => t.name === wanted) ?? null);
  const open = useCallback(
    (name: string) => navigate(`${PATHS.conversation(id)}?t=${encodeURIComponent(name)}`),
    [id],
  );
  const close = useCallback(() => navigate(PATHS.conversation(id)), [id]);
  return { wanted, task, open, close };
}

export interface Rail {
  readonly visible: boolean;
  readonly folded: boolean;
  readonly toggle: () => void;
}

export function useRail(pane: PaneRoute): Rail {
  const { settings, update } = useSettings();
  const open = settings.sessionPanelOpen;
  const paneOpen = pane.task !== null;
  const toggle = () => {
    if (!paneOpen) {
      update({ sessionPanelOpen: !open });
      return;
    }
    pane.close();
    if (!open) update({ sessionPanelOpen: true });
  };
  return { visible: open && !paneOpen, folded: !open && !paneOpen, toggle };
}

export interface Commands {
  readonly problem: string | null;
  readonly dismiss: () => void;
  readonly stop: () => Promise<void>;
  readonly end: () => Promise<void>;
  readonly sendMessage: (text: string) => Promise<boolean>;
  readonly answerAgent: (agent: string, text: string) => Promise<boolean>;
}

export function useCommands(id: string): Commands {
  const actions = useConversationActions();
  const [problem, setProblem] = useState<string | null>(null);
  const calls = useMemo(() => {
    const attempt = async (action: () => Promise<unknown>): Promise<boolean> => {
      try {
        await action();
        setProblem(null);
        return true;
      } catch (e) {
        setProblem(toMessage(e));
        return false;
      }
    };
    return {
      dismiss: () => setProblem(null),
      stop: async () => void (await attempt(() => actions.stop(id))),
      end: async () => void (await attempt(() => actions.end(id))),
      sendMessage: (text: string) => attempt(() => actions.send(id, { text })),
      answerAgent: (agent: string, text: string) => attempt(() => sendText(agent, text)),
    };
  }, [actions, id]);
  return { ...calls, problem };
}

export function useThreadValue(detail: ConversationDetail, pane: PaneRoute, commands: Commands): ThreadContextValue {
  const { tasks, attachments, conversation } = detail;
  const { sendMessage, answerAgent } = commands;
  return useMemo(
    () => ({
      tasks,
      attachments,
      thinking: conversation.status === "thinking",
      openTask: pane.task?.name ?? null,
      onOpenTask: pane.open,
      sendMessage,
      answerAgent,
    }),
    [tasks, attachments, conversation.status, pane.task, pane.open, sendMessage, answerAgent],
  );
}
