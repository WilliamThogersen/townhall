import { httpBase } from "./base";
import { ApiError } from "./errors";
import { parseEnvelope } from "./guards";
import { request } from "./request";
import type { Attachment, AttachmentKind } from "./types";

export const MAX_ATTACHMENT_BYTES = 20 * 1024 * 1024;

export type UploadProgress = (percent: number) => void;

const enc = encodeURIComponent;

function fail(what: string): never {
  throw new ApiError("protocol", `unexpected response shape: ${what}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isKind(value: unknown): value is AttachmentKind {
  return value === "image" || value === "file";
}

export function parseAttachment(value: unknown): Attachment {
  if (!isRecord(value)) return fail("attachment is not an object");
  const { id, conversationId, name, mime, size, kind, createdAt } = value;
  if (typeof id !== "string") return fail("attachment.id");
  if (typeof conversationId !== "string") return fail("attachment.conversationId");
  if (typeof name !== "string") return fail("attachment.name");
  if (typeof mime !== "string") return fail("attachment.mime");
  if (typeof size !== "number") return fail("attachment.size");
  if (!isKind(kind)) return fail("attachment.kind");
  if (typeof createdAt !== "string") return fail("attachment.createdAt");
  return { id, conversationId, name, mime, size, kind, createdAt };
}

export function parseAttachments(value: unknown): readonly Attachment[] {
  if (!Array.isArray(value)) return fail("attachments is not an array");
  return value.map(parseAttachment);
}

const attachmentsPath = (conversationId: string): string =>
  `/conversations/${enc(conversationId)}/attachments`;

export function attachmentUrl(conversationId: string, id: string, win: object = window): string {
  return `${httpBase(win)}${attachmentsPath(conversationId)}/${enc(id)}`;
}

export const listAttachments = (
  conversationId: string,
  signal?: AbortSignal,
): Promise<readonly Attachment[]> =>
  request("GET", attachmentsPath(conversationId), parseAttachments, undefined, signal);

function abortError(): DOMException {
  return new DOMException("the upload was cancelled", "AbortError");
}

function readJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

function settle(xhr: XMLHttpRequest): Attachment {
  const json = readJson(xhr.responseText);
  if (json === undefined) {
    throw new ApiError("protocol", `HTTP ${xhr.status}: response is not JSON`, xhr.status);
  }
  const env = parseEnvelope(json);
  const ok = xhr.status >= 200 && xhr.status < 300;
  if (!ok || !env.success) {
    throw new ApiError("http", env.error ?? `HTTP ${xhr.status}`, xhr.status);
  }
  return parseAttachment(env.data);
}

function reportProgress(xhr: XMLHttpRequest, onProgress: UploadProgress | undefined): void {
  if (onProgress === undefined) return;
  xhr.upload.onprogress = (event) => {
    if (event.lengthComputable && event.total > 0) {
      onProgress(Math.round((event.loaded / event.total) * 100));
    }
  };
}

export function uploadAttachment(
  conversationId: string,
  file: File,
  onProgress?: UploadProgress,
  signal?: AbortSignal,
): Promise<Attachment> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(abortError());
      return;
    }
    const xhr = new XMLHttpRequest();
    const cancel = () => xhr.abort();
    const detach = () => signal?.removeEventListener("abort", cancel);
    xhr.open("POST", `${httpBase()}${attachmentsPath(conversationId)}`);
    reportProgress(xhr, onProgress);
    xhr.onload = () => {
      detach();
      try {
        resolve(settle(xhr));
      } catch (error) {
        reject(error);
      }
    };
    xhr.onerror = () => {
      detach();
      reject(new ApiError("network", `cannot reach the daemon: the upload of ${file.name} stopped`));
    };
    xhr.onabort = () => {
      detach();
      reject(abortError());
    };
    signal?.addEventListener("abort", cancel, { once: true });
    const form = new FormData();
    form.append("file", file, file.name);
    xhr.send(form);
  });
}
