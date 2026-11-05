import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { uploadAttachment } from "../api/attachments";
import { ApiError, toMessage } from "../api/errors";
import {
  doneAttachmentIds,
  isUploading,
  latestError,
  newUpload,
  stagedFiles,
  stagedUpload,
  uploadsReducer,
  type PendingUpload,
  type UploadAction,
} from "./uploads";
import { isImageFile, rejectReason } from "./validate";

export interface Uploads {
  readonly items: readonly PendingUpload[];
  readonly enabled: boolean;
  readonly uploading: boolean;
  readonly attachmentIds: readonly string[];
  readonly stagedFiles: readonly File[];
  readonly error: string | null;
  readonly canRetry: boolean;
  readonly add: (files: Iterable<File> | ArrayLike<File>) => void;
  readonly retry: (key: string) => void;
  readonly retryFailed: () => void;
  readonly remove: (key: string) => void;
  readonly clear: () => void;
}

const NO_UPLOADS: readonly PendingUpload[] = [];

function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

function isRetryable(error: unknown): boolean {
  const refused = error instanceof ApiError && error.kind === "http" && (error.status ?? 500) < 500;
  return !refused;
}

const isRetryableFailure = (upload: PendingUpload): boolean => upload.status === "failed" && upload.retryable;

function previewOf(file: File): string | null {
  if (!isImageFile(file) || typeof URL.createObjectURL !== "function") return null;
  return URL.createObjectURL(file);
}

function revokePreview(upload: PendingUpload): void {
  if (upload.previewUrl !== null && typeof URL.revokeObjectURL === "function") {
    URL.revokeObjectURL(upload.previewUrl);
  }
}

interface Controllers {
  readonly track: (key: string, controller: AbortController) => void;
  readonly forget: (key: string, controller: AbortController) => void;
  readonly abort: (key: string) => void;
  readonly abortAll: () => void;
}

function createControllers(): Controllers {
  let running: Readonly<Record<string, AbortController>> = {};
  const without = (key: string) =>
    Object.fromEntries(Object.entries(running).filter(([current]) => current !== key));
  return {
    track: (key, controller) => {
      running = { ...running, [key]: controller };
    },
    forget: (key, controller) => {
      if (running[key] === controller) running = without(key);
    },
    abort: (key) => {
      running[key]?.abort();
      running = without(key);
    },
    abortAll: () => {
      const all = Object.values(running);
      running = {};
      all.forEach((controller) => controller.abort());
    },
  };
}

function usePreviewCleanup(items: readonly PendingUpload[]): void {
  const shown = useRef<readonly PendingUpload[]>(NO_UPLOADS);
  useEffect(() => {
    const kept = new Set(items.map((upload) => upload.previewUrl));
    shown.current.filter((upload) => !kept.has(upload.previewUrl)).forEach(revokePreview);
    shown.current = items;
  }, [items]);
  useEffect(() => () => shown.current.forEach(revokePreview), []);
}

function useLatest<T>(value: T): { readonly current: T } {
  const ref = useRef(value);
  useEffect(() => {
    ref.current = value;
  }, [value]);
  return ref;
}

type Dispatch = (action: UploadAction) => void;
type Start = (key: string, file: File, target: string) => void;

function useStarter(dispatch: Dispatch, controllers: Controllers): Start {
  return useCallback(
    (key, file, target) => {
      const controller = new AbortController();
      controllers.track(key, controller);
      const onProgress = (percent: number) => dispatch({ type: "progressed", key, percent });
      uploadAttachment(target, file, onProgress, controller.signal)
        .then((attachment) => dispatch({ type: "finished", key, attachment }))
        .catch((error: unknown) => {
          if (isAbort(error)) return;
          dispatch({ type: "failed", key, error: toMessage(error), retryable: isRetryable(error) });
        })
        .finally(() => controllers.forget(key, controller));
    },
    [controllers, dispatch],
  );
}

function useAdd(id: string | null, stage: boolean, dispatch: Dispatch, start: Start): Uploads["add"] {
  const counter = useRef(0);
  return useCallback(
    (files) => {
      if (id === null && !stage) return;
      const make = id === null ? stagedUpload : newUpload;
      const uploads = Array.from(files, (file) => {
        counter.current += 1;
        return make(`upload-${counter.current}`, file, previewOf(file), rejectReason(file));
      });
      if (uploads.length === 0) return;
      dispatch({ type: "added", uploads });
      if (id === null) return;
      uploads.filter((upload) => upload.status === "uploading").forEach((upload) => start(upload.key, upload.file, id));
    },
    [id, stage, dispatch, start],
  );
}

function useRetry(
  id: string | null,
  items: readonly PendingUpload[],
  dispatch: Dispatch,
  start: Start,
): Pick<Uploads, "retry" | "retryFailed"> {
  const latestItems = useLatest(items);
  const retry = useCallback(
    (key: string) => {
      const upload = latestItems.current.find((current) => current.key === key);
      if (id === null || upload === undefined || !isRetryableFailure(upload)) return;
      dispatch({ type: "restarted", key });
      start(key, upload.file, id);
    },
    [id, latestItems, dispatch, start],
  );
  const retryFailed = useCallback(
    () => latestItems.current.filter(isRetryableFailure).forEach((upload) => retry(upload.key)),
    [latestItems, retry],
  );
  return { retry, retryFailed };
}

function useRemoval(dispatch: Dispatch, controllers: Controllers): Pick<Uploads, "remove" | "clear"> {
  const remove = useCallback(
    (key: string) => {
      controllers.abort(key);
      dispatch({ type: "removed", key });
    },
    [controllers, dispatch],
  );
  const clear = useCallback(() => {
    controllers.abortAll();
    dispatch({ type: "cleared" });
  }, [controllers, dispatch]);
  return { remove, clear };
}

export function useUploads(conversationId: string | null | undefined, stage = false): Uploads {
  const [items, dispatch] = useReducer(uploadsReducer, NO_UPLOADS);
  const [controllers] = useState(createControllers);
  const id = conversationId ?? null;
  usePreviewCleanup(items);
  const start = useStarter(dispatch, controllers);
  const add = useAdd(id, stage, dispatch, start);
  const { retry, retryFailed } = useRetry(id, items, dispatch, start);
  const { remove, clear } = useRemoval(dispatch, controllers);
  useEffect(() => clear, [id, clear]);
  const attachmentIds = useMemo(() => doneAttachmentIds(items), [items]);
  const files = useMemo(() => stagedFiles(items), [items]);
  return {
    items,
    enabled: id !== null || stage,
    uploading: isUploading(items),
    attachmentIds,
    stagedFiles: files,
    error: latestError(items),
    canRetry: items.some(isRetryableFailure),
    add,
    retry,
    retryFailed,
    remove,
    clear,
  };
}
