import { createContext, useContext } from "react";
import type { AgentKey, Attachment, Task } from "../../../api/types";

export interface ThreadContextValue {
  readonly conversationId: string;
  readonly tasks: readonly Task[];
  readonly attachments: readonly Attachment[];
  readonly thinking: boolean;
  readonly openTask: string | null;
  readonly onOpenTask: (name: string) => void;
  readonly sendMessage: (text: string) => Promise<boolean>;
  readonly answerAgent: (agent: string, text: string) => Promise<boolean>;
  readonly answerKeys: (agent: string, keys: readonly AgentKey[]) => Promise<boolean>;
}

export const ThreadContext = createContext<ThreadContextValue | null>(null);

export function useThread(): ThreadContextValue {
  const value = useContext(ThreadContext);
  if (value === null) throw new Error("thread parts need a ThreadContext");
  return value;
}
