import { useCallback, useEffect, useMemo, useState } from "react";
import { allowTool, sendText } from "../../api/client";
import { toMessage } from "../../api/errors";
import type { AgentKey, ConversationDetail, Task } from "../../api/types";
import { readReply, type ReplyTarget } from "../../composer/replies";
import { useConversationActions } from "../../data";
import { navigate, PATHS, useRoute } from "../../router";
import { useSettings } from "../../settings/useSettings";
import type { ThreadContextValue } from "./thread/threadContext";
import { pressKeys } from "./thread/trustDialog";
import { SESSION_PANEL_ID } from "./session";
import { isInTerminal, isPlainKey } from "./session/keys";
import { useNarrow } from "./useNarrow";

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

export type RailAttribute = "closed" | "open" | undefined;

export interface Rail {
  readonly visible: boolean;
  readonly attribute: RailAttribute;
  readonly toggle: () => void;
}

function useWideRail(pane: PaneRoute): Rail {
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
  return { visible: open && !paneOpen, attribute: !open && !paneOpen ? "closed" : undefined, toggle };
}

function useOverlayRail(pane: PaneRoute): Rail {
  const [open, setOpen] = useState(false);
  const paneOpen = pane.task !== null;
  const toggle = () => {
    if (!paneOpen) {
      setOpen((was) => !was);
      return;
    }
    pane.close();
    setOpen(true);
  };
  const visible = open && !paneOpen;
  useEffect(() => {
    if (!visible) return;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    document.getElementById(SESSION_PANEL_ID)?.focus();
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || !isPlainKey(e) || isInTerminal(e.target)) return;
      e.preventDefault();
      setOpen(false);
      opener?.focus();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [visible]);
  return { visible, attribute: visible ? "open" : undefined, toggle };
}

export function useRail(pane: PaneRoute): Rail {
  const narrow = useNarrow();
  const wide = useWideRail(pane);
  const overlay = useOverlayRail(pane);
  return narrow ? overlay : wide;
}

export interface Commands {
  readonly problem: string | null;
  readonly dismiss: () => void;
  readonly stop: () => Promise<void>;
  readonly end: () => Promise<void>;
  readonly sendMessage: (text: string) => Promise<boolean>;
  readonly answerAgent: (agent: string, text: string) => Promise<boolean>;
  readonly answerKeys: (agent: string, keys: readonly AgentKey[]) => Promise<boolean>;
  readonly allowTool: (agent: string, tool: string) => Promise<boolean>;
  readonly submit: (text: string, attachmentIds: readonly string[]) => Promise<unknown>;
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
      answerKeys: (agent: string, keys: readonly AgentKey[]) => attempt(() => pressKeys(agent, keys)),
      allowTool: (agent: string, tool: string) => attempt(() => allowTool(agent, tool)),
      submit: (text: string, attachmentIds: readonly string[]) => {
        const replyTo = readReply(id)?.id;
        return actions.send(id, {
          text,
          ...(attachmentIds.length > 0 ? { attachments: attachmentIds } : {}),
          ...(replyTo === undefined ? {} : { replyTo }),
        });
      },
    };
  }, [actions, id]);
  return { ...calls, problem };
}

export function useThreadValue(
  detail: ConversationDetail,
  pane: PaneRoute,
  commands: Commands,
  onReply: (target: ReplyTarget) => void,
): ThreadContextValue {
  const { tasks, attachments, conversation } = detail;
  const { sendMessage, answerAgent, answerKeys, allowTool } = commands;
  return useMemo(
    () => ({
      conversationId: conversation.id,
      tasks,
      attachments,
      thinking: conversation.status === "thinking",
      openTask: pane.task?.name ?? null,
      onOpenTask: pane.open,
      onReply,
      sendMessage,
      answerAgent,
      answerKeys,
      allowTool,
    }),
    [conversation.id, tasks, attachments, conversation.status, pane.task, pane.open, onReply, sendMessage, answerAgent, answerKeys, allowTool],
  );
}
