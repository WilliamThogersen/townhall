export type ApiErrorKind = "network" | "http" | "protocol";

/** Every failure from the API client, mapped to one shape. */
export class ApiError extends Error {
  readonly kind: ApiErrorKind;
  readonly status: number | null;
  readonly data: unknown;

  constructor(kind: ApiErrorKind, message: string, status: number | null = null, data: unknown = null) {
    super(message);
    this.name = "ApiError";
    this.kind = kind;
    this.status = status;
    this.data = data;
  }
}

export function toMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
