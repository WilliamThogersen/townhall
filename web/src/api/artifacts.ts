import { httpBase } from "./base";
import { ApiError, toMessage } from "./errors";
import { listOf, nullable, num, oneOf, record, str, type Guard } from "./guardKit";
import { request } from "./request";
import type { Artifact, ArtifactKind, AuthorRole } from "./types";

export const ARTIFACT_KINDS: readonly ArtifactKind[] = ["screenshot", "log", "note", "report"];
export const AUTHOR_ROLES: readonly AuthorRole[] = ["coder", "tester", "reviewer", "other"];

const enc = encodeURIComponent;

export function parseArtifact(value: unknown): Artifact {
  const r = record(value, "artifact");
  return {
    id: str(r.id, "artifact.id"),
    conversationId: str(r.conversationId, "artifact.conversationId"),
    task: str(r.task, "artifact.task"),
    name: str(r.name, "artifact.name"),
    kind: oneOf(ARTIFACT_KINDS)(r.kind, "artifact.kind"),
    mime: str(r.mime, "artifact.mime"),
    size: num(r.size, "artifact.size"),
    authorRole: oneOf(AUTHOR_ROLES)(r.authorRole, "artifact.authorRole"),
    caption: nullable(r.caption, "artifact.caption", str),
    createdAt: str(r.createdAt, "artifact.createdAt"),
  };
}

export const parseArtifacts: Guard<readonly Artifact[]> = listOf(parseArtifact, "artifacts");

const artifactsPath = (task: string): string => `/tasks/${enc(task)}/artifacts`;

export function artifactUrl(task: string, id: string, win: object = window): string {
  return `${httpBase(win)}${artifactsPath(task)}/${enc(id)}`;
}

export const listArtifacts = (task: string, signal?: AbortSignal): Promise<readonly Artifact[]> =>
  request("GET", artifactsPath(task), parseArtifacts, undefined, signal);

export async function readArtifactText(task: string, id: string, signal?: AbortSignal): Promise<string> {
  let res: Response;
  try {
    res = await fetch(artifactUrl(task, id), { signal });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    throw new ApiError("network", `cannot reach the daemon: ${toMessage(e)}`);
  }
  if (!res.ok) throw new ApiError("http", `HTTP ${res.status}: the artifact cannot be read`, res.status);
  return res.text();
}
