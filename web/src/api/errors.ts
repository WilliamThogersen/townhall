export type ApiErrorKind = "network" | "http" | "protocol";

/** Every failure from the API client, mapped to one shape. */
export class ApiError extends Error {
  readonly kind: ApiErrorKind;
  readonly status: number | null;

  constructor(kind: ApiErrorKind, message: string, status: number | null = null) {
    super(message);
    this.name = "ApiError";
    this.kind = kind;
    this.status = status;
  }
}

export function toMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
