import type { Attachment } from "../api/types";
import { isImageFile } from "./validate";

export type UploadStatus = "uploading" | "done" | "failed";

export interface PendingUpload {
  readonly key: string;
  readonly file: File;
  readonly isImage: boolean;
  readonly previewUrl: string | null;
  readonly status: UploadStatus;
  readonly percent: number;
  readonly attachment: Attachment | null;
  readonly error: string | null;
  readonly retryable: boolean;
}

export type UploadAction =
  | { readonly type: "added"; readonly uploads: readonly PendingUpload[] }
  | { readonly type: "progressed"; readonly key: string; readonly percent: number }
  | { readonly type: "finished"; readonly key: string; readonly attachment: Attachment }
  | { readonly type: "failed"; readonly key: string; readonly error: string; readonly retryable: boolean }
  | { readonly type: "restarted"; readonly key: string }
  | { readonly type: "removed"; readonly key: string }
  | { readonly type: "cleared" };

export function newUpload(
  key: string,
  file: File,
  previewUrl: string | null,
  rejection: string | null,
): PendingUpload {
  return {
    key,
    file,
    isImage: isImageFile(file),
    previewUrl,
    status: rejection === null ? "uploading" : "failed",
    percent: 0,
    attachment: null,
    error: rejection,
    retryable: false,
  };
}

type Change = (upload: PendingUpload) => PendingUpload | null;

function changeOne(
  uploads: readonly PendingUpload[],
  key: string,
  change: Change,
): readonly PendingUpload[] {
  const index = uploads.findIndex((upload) => upload.key === key);
  const current = uploads[index];
  if (current === undefined) return uploads;
  const next = change(current);
  if (next === current) return uploads;
  return next === null
    ? uploads.filter((upload) => upload.key !== key)
    : uploads.map((upload) => (upload.key === key ? next : upload));
}

const whileUploading =
  (change: (upload: PendingUpload) => PendingUpload): Change =>
  (upload) =>
    upload.status === "uploading" ? change(upload) : upload;

function changeFor(action: Exclude<UploadAction, { type: "added" | "cleared" }>): Change {
  switch (action.type) {
    case "progressed":
      return whileUploading((upload) => ({ ...upload, percent: action.percent }));
    case "finished":
      return whileUploading((upload) => ({ ...upload, status: "done", percent: 100, attachment: action.attachment }));
    case "failed":
      return whileUploading((upload) => ({
        ...upload,
        status: "failed",
        error: action.error,
        retryable: action.retryable,
      }));
    case "restarted":
      return (upload) =>
        upload.status === "failed"
          ? { ...upload, status: "uploading", percent: 0, error: null, retryable: false }
          : upload;
    case "removed":
      return () => null;
  }
}

export function uploadsReducer(
  uploads: readonly PendingUpload[],
  action: UploadAction,
): readonly PendingUpload[] {
  if (action.type === "added") return [...uploads, ...action.uploads];
  if (action.type === "cleared") return uploads.length === 0 ? uploads : [];
  return changeOne(uploads, action.key, changeFor(action));
}

export const isUploading = (uploads: readonly PendingUpload[]): boolean =>
  uploads.some((upload) => upload.status === "uploading");

export const doneAttachmentIds = (uploads: readonly PendingUpload[]): readonly string[] =>
  uploads.flatMap((upload) => (upload.status === "done" && upload.attachment ? [upload.attachment.id] : []));

export function latestError(uploads: readonly PendingUpload[]): string | null {
  const failed = uploads.filter((upload) => upload.status === "failed");
  return failed[failed.length - 1]?.error ?? null;
}
