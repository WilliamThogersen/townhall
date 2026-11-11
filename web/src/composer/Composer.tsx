import {
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type ChangeEvent,
  type ClipboardEvent,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
  type RefObject,
} from "react";
import { clipboardFiles, hasClipboardText } from "../attachments/clipboard";
import { UploadChips } from "../attachments/UploadChips";
import { useUploads, type Uploads } from "../attachments/useUploads";
import { ACCEPTED_FILES } from "../attachments/validate";
import { ComposerTools } from "./ComposerTools";
import { readDraft, writeDraft } from "./drafts";
import { PluginComposerActions } from "./PluginComposerActions";
import { QueueList } from "./QueueList";
import { readReply, useReply, writeReply } from "./replies";
import { ReplyBar } from "./ReplyBar";
import { useAutoGrow } from "./useAutoGrow";
import { useMessageQueue, type MessageQueue } from "./useMessageQueue";
import { useSubmit, type SubmitHandler } from "./useSubmit";
import { helpText, LOOKS, THINKING_PLACEHOLDER, type ComposerVariant } from "./variants";
import "./composer.css";

export type { ComposerVariant } from "./variants";

export interface ComposerHandle {
  readonly focus: () => void;
  readonly fill: (text: string) => void;
  readonly addFiles: (files: Iterable<File> | ArrayLike<File>) => void;
}

export interface ComposerProps {
  readonly onSubmit: SubmitHandler;
  readonly variant?: ComposerVariant;
  readonly placeholder?: string;
  readonly ariaLabel?: string;
  readonly submitLabel?: string;
  readonly disabled?: boolean;
  readonly busy?: boolean;
  readonly onStop?: () => void;
  readonly tools?: ReactNode;
  readonly conversationId?: string | null;
  readonly stageFiles?: boolean;
  readonly autoFocus?: boolean;
  readonly focusKey?: unknown;
  readonly ref?: Ref<ComposerHandle>;
}

function useFocusKey(inputRef: RefObject<HTMLTextAreaElement | null>, focusKey: unknown): void {
  const previous = useRef(focusKey);
  useEffect(() => {
    if (Object.is(previous.current, focusKey)) return;
    previous.current = focusKey;
    inputRef.current?.focus();
  }, [focusKey, inputRef]);
}

function useComposerHandle(
  ref: Ref<ComposerHandle> | undefined,
  inputRef: RefObject<HTMLTextAreaElement | null>,
  setText: (text: string) => void,
  uploads: Uploads,
): void {
  const { add } = uploads;
  useImperativeHandle(
    ref,
    () => ({
      focus: () => inputRef.current?.focus(),
      fill: (text) => {
        setText(text);
        inputRef.current?.focus();
      },
      addFiles: add,
    }),
    [add, inputRef, setText],
  );
}

interface InputActions {
  readonly uploads: Uploads;
  readonly queue: MessageQueue;
  readonly conversationId: string | null;
  readonly busy: boolean;
  readonly disabled: boolean;
  readonly text: string;
  readonly submit: () => Promise<void>;
  readonly setText: (text: string) => void;
  readonly clearError: () => void;
  readonly cancelReply: () => boolean;
}

const hasModifier = (event: KeyboardEvent): boolean =>
  event.shiftKey || event.ctrlKey || event.altKey || event.metaKey;

function useInputHandlers(actions: InputActions) {
  const { uploads, queue, conversationId, busy, disabled, text, submit, setText, clearError, cancelReply } = actions;
  const onChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    setText(event.target.value);
    clearError();
    queue.clearError();
  };
  const addToQueue = () => {
    if (disabled || uploads.uploading) return;
    const reply = conversationId === null ? null : readReply(conversationId);
    if (!queue.add(text.trim(), uploads.attachmentIds, reply)) return;
    setText("");
    uploads.clear();
    cancelReply();
  };
  const pullLast = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (hasModifier(event) || text !== "") return;
    const pulled = queue.pullLast();
    if (pulled === null) return;
    event.preventDefault();
    setText(pulled.text);
    if (conversationId !== null && pulled.reply !== null) writeReply(conversationId, pulled.reply);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      if (queue.restore()) {
        setText("");
        cancelReply();
      } else if (!cancelReply()) event.currentTarget.blur();
      return;
    }
    if (event.key === "ArrowUp") {
      pullLast(event);
      return;
    }
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
    event.preventDefault();
    if (busy || queue.items.length > 0) addToQueue();
    else void submit();
  };
  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = clipboardFiles(event.clipboardData);
    if (!uploads.enabled || files.length === 0) return;
    if (!hasClipboardText(event.clipboardData)) event.preventDefault();
    uploads.add(files);
  };
  const onPick = (event: ChangeEvent<HTMLInputElement>) => {
    const picked = Array.from(event.currentTarget.files ?? []);
    event.currentTarget.value = "";
    uploads.add(picked);
  };
  return { onChange, onKeyDown, onPaste, onPick };
}

function useComposer(props: ComposerProps) {
  const { variant = "thread", disabled = false, busy = false, conversationId = null, stageFiles = false } = props;
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const [text, setDraftText] = useState(() => readDraft(conversationId));
  const setText = useCallback(
    (next: string) => {
      writeDraft(conversationId, next);
      setDraftText(next);
    },
    [conversationId],
  );
  const uploads = useUploads(conversationId, stageFiles);
  const reply = useReply(conversationId);
  const cancelReply = useCallback((): boolean => {
    if (conversationId === null || readReply(conversationId) === null) return false;
    writeReply(conversationId, null);
    return true;
  }, [conversationId]);
  const queue = useMessageQueue({ conversationId, busy, disabled, onSubmit: props.onSubmit });
  const { editing, finishEdit } = queue;
  const { onSubmit } = props;
  const submitEdited = useCallback<SubmitHandler>(
    (message, attachmentIds, files) => onSubmit(message, [...(editing?.attachmentIds ?? []), ...attachmentIds], files),
    [onSubmit, editing],
  );
  const onSent = useCallback(() => {
    setText("");
    cancelReply();
    finishEdit();
  }, [setText, cancelReply, finishEdit]);
  const sending = useSubmit({ text, uploads, blocked: disabled || busy, onSubmit: submitEdited, onSent });
  const handlers = useInputHandlers({
    uploads,
    queue,
    conversationId,
    busy,
    disabled,
    text,
    submit: sending.submit,
    setText,
    clearError: sending.clearError,
    cancelReply,
  });
  useAutoGrow(inputRef, text);
  useFocusKey(inputRef, props.focusKey);
  useComposerHandle(props.ref, inputRef, setText, uploads);
  const insert = (inserted: string) => setText(text + inserted);
  const onSubmitForm = (event: FormEvent) => {
    event.preventDefault();
    void sending.submit();
  };
  return {
    look: LOOKS[variant],
    disabled,
    busy,
    inputRef,
    fileRef,
    text,
    uploads,
    sending,
    handlers,
    onSubmitForm,
    reply,
    cancelReply,
    queue,
    insert,
  };
}

export function Composer(props: ComposerProps) {
  const {
    look,
    disabled,
    busy,
    inputRef,
    fileRef,
    text,
    uploads,
    sending,
    handlers,
    onSubmitForm,
    reply,
    cancelReply,
    queue,
    insert,
  } = useComposer(props);
  const submitLabel = props.submitLabel ?? look.submitLabel;
  const restingPlaceholder = props.placeholder ?? (uploads.enabled ? look.attachPlaceholder : look.placeholder);
  return (
    <form className={look.className} onSubmit={onSubmitForm}>
      {uploads.enabled && (
        <input ref={fileRef} type="file" multiple hidden tabIndex={-1} accept={ACCEPTED_FILES} onChange={handlers.onPick} />
      )}
      <div className="th-composer-box">
        {reply === null ? null : <ReplyBar target={reply} onCancel={() => void cancelReply()} />}
        <QueueList items={queue.items} onRemove={queue.remove} />
        <UploadChips items={uploads.items} onRemove={uploads.remove} />
        <span className="th-composer-sign" aria-hidden="true">
          ›
        </span>
        <textarea
          ref={inputRef}
          className="th-composer-input"
          rows={look.rows}
          value={text}
          placeholder={busy ? THINKING_PLACEHOLDER : restingPlaceholder}
          aria-label={props.ariaLabel ?? look.ariaLabel}
          disabled={disabled}
          readOnly={sending.submitting}
          autoFocus={props.autoFocus}
          onChange={handlers.onChange}
          onKeyDown={handlers.onKeyDown}
          onPaste={handlers.onPaste}
        />
        <ComposerTools
          look={look}
          submitLabel={submitLabel}
          help={helpText(submitLabel, uploads.enabled)}
          error={sending.error ?? queue.error ?? uploads.error}
          tools={props.tools}
          pluginActions={<PluginComposerActions conversationId={props.conversationId ?? null} onInsert={insert} />}
          onAttach={uploads.enabled ? () => fileRef.current?.click() : null}
          attachDisabled={disabled}
          onRetry={uploads.canRetry ? uploads.retryFailed : null}
          onStop={busy && props.onStop ? props.onStop : null}
          canSubmit={sending.canSubmit}
          submitting={sending.submitting}
        />
      </div>
    </form>
  );
}
