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
import { useAutoGrow } from "./useAutoGrow";
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
  readonly submit: () => Promise<void>;
  readonly setText: (text: string) => void;
  readonly clearError: () => void;
}

function useInputHandlers({ uploads, submit, setText, clearError }: InputActions) {
  const onChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    setText(event.target.value);
    clearError();
  };
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.currentTarget.blur();
      return;
    }
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
    event.preventDefault();
    void submit();
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
  const clearText = useCallback(() => setText(""), [setText]);
  const sending = useSubmit({ text, uploads, blocked: disabled || busy, onSubmit: props.onSubmit, onSent: clearText });
  const handlers = useInputHandlers({ uploads, submit: sending.submit, setText, clearError: sending.clearError });
  useAutoGrow(inputRef, text);
  useFocusKey(inputRef, props.focusKey);
  useComposerHandle(props.ref, inputRef, setText, uploads);
  const onSubmitForm = (event: FormEvent) => {
    event.preventDefault();
    void sending.submit();
  };
  return { look: LOOKS[variant], disabled, busy, inputRef, fileRef, text, uploads, sending, handlers, onSubmitForm };
}

export function Composer(props: ComposerProps) {
  const { look, disabled, busy, inputRef, fileRef, text, uploads, sending, handlers, onSubmitForm } = useComposer(props);
  const submitLabel = props.submitLabel ?? look.submitLabel;
  const restingPlaceholder = props.placeholder ?? (uploads.enabled ? look.attachPlaceholder : look.placeholder);
  return (
    <form className={look.className} onSubmit={onSubmitForm}>
      {uploads.enabled && (
        <input ref={fileRef} type="file" multiple hidden tabIndex={-1} accept={ACCEPTED_FILES} onChange={handlers.onPick} />
      )}
      <div className="th-composer-box">
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
          disabled={disabled || busy}
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
          error={sending.error ?? uploads.error}
          tools={props.tools}
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
