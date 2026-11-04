import { useCallback, useState } from "react";
import { toMessage } from "../api/errors";
import type { Uploads } from "../attachments/useUploads";

export type SubmitHandler = (text: string, attachmentIds: readonly string[]) => Promise<unknown>;

export interface SubmitOptions {
  readonly text: string;
  readonly uploads: Uploads;
  readonly blocked: boolean;
  readonly onSubmit: SubmitHandler;
  readonly onSent: () => void;
}

export interface Submit {
  readonly submitting: boolean;
  readonly error: string | null;
  readonly canSubmit: boolean;
  readonly submit: () => Promise<void>;
  readonly clearError: () => void;
}

export function useSubmit({ text, uploads, blocked, onSubmit, onSent }: SubmitOptions): Submit {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trimmed = text.trim();
  const { attachmentIds, uploading, clear } = uploads;
  const hasContent = trimmed !== "" || attachmentIds.length > 0;
  const canSubmit = !blocked && !submitting && !uploading && hasContent;

  const submit = useCallback(async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      await onSubmit(trimmed, attachmentIds);
      clear();
      onSent();
    } catch (failure) {
      setError(toMessage(failure));
    } finally {
      setSubmitting(false);
    }
  }, [canSubmit, onSubmit, trimmed, attachmentIds, clear, onSent]);

  const clearError = useCallback(() => setError(null), []);
  return { submitting, error, canSubmit, submit, clearError };
}
