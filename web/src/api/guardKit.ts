import { ApiError } from "./errors";

export type Guard<T> = (value: unknown) => T;
export type Check<T> = (value: unknown, what: string) => T;
export type Fields = Readonly<Record<string, unknown>>;

export function fail(what: string): never {
  throw new ApiError("protocol", `unexpected response shape: ${what}`);
}

export function isRecord(value: unknown): value is Fields {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function record(value: unknown, what: string): Fields {
  return isRecord(value) ? value : fail(`${what} is not an object`);
}

export function str(value: unknown, what: string): string {
  return typeof value === "string" ? value : fail(what);
}

export function num(value: unknown, what: string): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fail(what);
}

export function bool(value: unknown, what: string): boolean {
  return typeof value === "boolean" ? value : fail(what);
}

export function nullable<T>(value: unknown, what: string, check: Check<T>): T | null {
  return value === null || value === undefined ? null : check(value, what);
}

export function oneOf<T extends string>(options: readonly T[]): Check<T> {
  return (value, what) =>
    typeof value === "string" && (options as readonly string[]).includes(value)
      ? (value as T)
      : fail(what);
}

export function list<T>(value: unknown, what: string, check: Check<T>): readonly T[] {
  if (!Array.isArray(value)) return fail(`${what} is not an array`);
  return value.map((item: unknown, index) => check(item, `${what}[${index}]`));
}

export function strings(value: unknown, what: string): readonly string[] {
  return list(value, what, str);
}

export function listOf<T>(guard: Guard<T>, what: string): Guard<readonly T[]> {
  return (value) => list(value, what, guard);
}
