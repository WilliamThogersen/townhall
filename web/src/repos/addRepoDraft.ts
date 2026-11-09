import type { AddRepoInput } from "../api/types";

export interface AddRepoDraft {
  readonly path: string;
  readonly name: string;
  readonly baseBranch: string;
  readonly cloneDir: string;
}

export type AddMode = "folder" | "url";

export const EMPTY_DRAFT: AddRepoDraft = { path: "", name: "", baseBranch: "", cloneDir: "" };

export const DEFAULT_CLONES_DIR = "~/.townhall/repos";

const LINK = /^(https?:\/\/|ssh:\/\/|git:\/\/|git@)/i;

export function isLink(input: string): boolean {
  return LINK.test(input.trim());
}

function linkPath(link: string): string {
  const afterScheme = link.includes("://") ? link.slice(link.indexOf("://") + 3).split("/").slice(1).join("/") : link.split(":").slice(1).join(":");
  const trimmed = afterScheme.replace(/^\/+|\/+$/g, "");
  return trimmed.endsWith(".git") ? trimmed.slice(0, -4) : trimmed;
}

export type CloneProtocol = "ssh" | "https";

export const DEFAULT_PROTOCOL: CloneProtocol = "ssh";

function linkHost(link: string): string {
  const authority = link.includes("://") ? link.slice(link.indexOf("://") + 3).split("/")[0] ?? "" : link.split(":")[0] ?? "";
  return authority.slice(authority.lastIndexOf("@") + 1);
}

export function withProtocol(input: string, protocol: CloneProtocol): string {
  const link = input.trim();
  if (!isLink(link) || linkProblem(link) !== null) return link;
  const host = linkHost(link);
  if (host === "" || (link.includes("://") && host.includes(":"))) return link;
  const path = linkPath(link);
  return protocol === "ssh" ? `git@${host}:${path}.git` : `https://${host}/${path}.git`;
}

export function linkProblem(input: string): string | null {
  const link = input.trim();
  if (!isLink(link)) return null;
  if (/[\s\u0000-\u001f]/.test(link)) return "a link has no blanks";
  const segments = linkPath(link).split("/");
  const bad = segments.length < 2 || segments.some((s) => s === "" || s === "." || s === "..");
  return bad ? "expected host/owner/repo" : null;
}

export function modeProblem(mode: AddMode, path: string): string | null {
  if (mode === "folder") return isLink(path) ? "that is a link: choose url to clone it" : null;
  if (path.trim() !== "" && !isLink(path)) return "expected a git url: https://, ssh:// or git@";
  return linkProblem(path);
}

function linkName(link: string): string {
  const last = linkPath(link).split("/").at(-1) ?? "";
  const safe = last.replace(/[^A-Za-z0-9._-]/g, "-").replace(/^[^A-Za-z0-9]+/, "");
  return safe.slice(0, 64) || "repo";
}

export function cloneTargetOf(draft: AddRepoDraft, defaultDir: string): string | null {
  const link = draft.path.trim();
  if (!isLink(link) || linkProblem(link) !== null) return null;
  const dir = draft.cloneDir.trim() || defaultDir.trim() || DEFAULT_CLONES_DIR;
  return `${dir.replace(/\/+$/, "")}/${linkName(link)}`;
}

export function canSubmit(draft: AddRepoDraft): boolean {
  return draft.path.trim() !== "" && linkProblem(draft.path) === null;
}

export function addProgress(draft: AddRepoDraft): string {
  return isLink(draft.path) ? "cloning…" : "initialising…";
}

export function draftToInput(draft: AddRepoDraft, protocol: CloneProtocol = DEFAULT_PROTOCOL): AddRepoInput {
  const name = draft.name.trim();
  const baseBranch = draft.baseBranch.trim();
  const cloneDir = draft.cloneDir.trim();
  return {
    path: withProtocol(draft.path, protocol),
    ...(name === "" ? {} : { name }),
    ...(baseBranch === "" ? {} : { baseBranch }),
    ...(cloneDir === "" || !isLink(draft.path) ? {} : { cloneDir }),
  };
}
