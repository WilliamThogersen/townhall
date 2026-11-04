import type { Adapter, CreateAgentInput } from "../api/types";

const NAME_RE = /^[A-Za-z0-9_-]+$/;

export function validateName(name: string): string | null {
  if (name.length === 0) return "name is required";
  if (!NAME_RE.test(name)) return "use letters, digits, - and _ only";
  return null;
}

export interface FormValues {
  readonly name: string;
  readonly adapter: Adapter;
  readonly prompt: string;
  readonly command: string;
  readonly base: string;
}

export const EMPTY_FORM: FormValues = {
  name: "",
  adapter: "claude",
  prompt: "",
  command: "",
  base: "",
};

export interface FormResult {
  readonly errors: Readonly<Partial<Record<"name" | "base" | "command", string>>>;
  readonly input: CreateAgentInput | null;
}

export function validateForm(v: FormValues): FormResult {
  const errors: { name?: string; base?: string; command?: string } = {};
  const nameError = validateName(v.name);
  if (nameError) errors.name = nameError;
  const base = v.base.trim();
  if (base.startsWith("-")) errors.base = "base branch must not start with -";
  if (v.adapter === "shell" && v.command.trim() === "") {
    errors.command = "a shell agent needs a command";
  }
  if (Object.keys(errors).length > 0) return { errors, input: null };
  const extra =
    v.adapter === "claude"
      ? v.prompt.trim() ? { prompt: v.prompt.trim() } : {}
      : { command: v.command.trim() };
  return {
    errors,
    input: { name: v.name, adapter: v.adapter, ...(base ? { base } : {}), ...extra },
  };
}
