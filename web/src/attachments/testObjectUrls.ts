import { vi } from "vitest";

type CreateObjectUrl = typeof URL.createObjectURL;
type RevokeObjectUrl = typeof URL.revokeObjectURL;

export interface FakeObjectUrls {
  readonly create: ReturnType<typeof vi.fn<CreateObjectUrl>>;
  readonly revoke: ReturnType<typeof vi.fn<RevokeObjectUrl>>;
  readonly remove: () => void;
  readonly restore: () => void;
}

const nameOf = (blob: Blob | MediaSource): string => (blob instanceof File ? blob.name : "blob");

export function stubObjectUrls(): FakeObjectUrls {
  const original = { create: URL.createObjectURL, revoke: URL.revokeObjectURL };
  const create = vi.fn<CreateObjectUrl>((blob) => `blob:${nameOf(blob)}`);
  const revoke = vi.fn<RevokeObjectUrl>();
  const assign = (createUrl: CreateObjectUrl | undefined, revokeUrl: RevokeObjectUrl | undefined) => {
    Object.defineProperty(URL, "createObjectURL", { value: createUrl, configurable: true, writable: true });
    Object.defineProperty(URL, "revokeObjectURL", { value: revokeUrl, configurable: true, writable: true });
  };
  assign(create, revoke);
  return {
    create,
    revoke,
    remove: () => assign(undefined, undefined),
    restore: () => assign(original.create, original.revoke),
  };
}
