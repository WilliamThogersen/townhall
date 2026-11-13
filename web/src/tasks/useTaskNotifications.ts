import { useEffect, useRef } from "react";
import type { Task } from "../api/types";
import { notifyDesktop } from "../desktop/notify";
import { navigate, PATHS } from "../router";
import { useSettings } from "../settings/useSettings";
import { allowedKinds, describeEvents, taskEvents, type TaskEvent } from "./progress";

export const BATCH_MS = 3000;
export const OPEN_WINDOW_MS = 2 * 60 * 1000;

interface Opening {
  readonly conversationId: string;
  readonly at: number;
}

export function useTaskNotifications(tasks: readonly Task[], loading: boolean): void {
  const { settings } = useSettings();
  const kinds = allowedKinds(settings.notifyWhen);
  const kindsRef = useRef(kinds);
  kindsRef.current = kinds;
  const prev = useRef<readonly Task[] | null>(null);
  const pending = useRef<TaskEvent[]>([]);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const opening = useRef<Opening | null>(null);

  useEffect(() => {
    if (loading) return;
    const before = prev.current;
    prev.current = tasks;
    if (!before) return;
    const wanted = taskEvents(before, tasks).filter((e) => kindsRef.current.includes(e.kind));
    if (wanted.length === 0) return;
    pending.current = [...pending.current, ...wanted];
    if (timer.current !== null) return;
    timer.current = setTimeout(() => {
      timer.current = null;
      const batch = pending.current;
      pending.current = [];
      const text = describeEvents(batch);
      void notifyDesktop(text).then((sent) => {
        if (sent && text.conversationId !== null) {
          opening.current = { conversationId: text.conversationId, at: Date.now() };
        }
      });
    }, BATCH_MS);
  }, [tasks, loading]);

  useEffect(() => {
    const onFocus = () => {
      const target = opening.current;
      opening.current = null;
      if (target && Date.now() - target.at <= OPEN_WINDOW_MS) {
        navigate(PATHS.conversation(target.conversationId));
      }
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );
}
