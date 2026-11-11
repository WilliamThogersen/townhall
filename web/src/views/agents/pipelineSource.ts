import type { DefinitionError } from "../../api/types";

export const NEW_PIPELINE = `name = "my-pipeline"
description = ""
work = "build"

[[step]]
id = "build"
agent = "builder"
prompt = "{{input}}"
`;

const TOP_LEVEL_NAME = /^name\s*=\s*"([^"\n]*)"/m;
const FIRST_TABLE = /^\s*\[/m;

export function sourceName(text: string): string | null {
  const top = text.split(FIRST_TABLE)[0] ?? "";
  const name = TOP_LEVEL_NAME.exec(top)?.[1]?.trim();
  return name === undefined || name === "" ? null : name;
}

export function problem(field: string, message: string): DefinitionError {
  return { kind: "workflow", name: null, step: null, field, message, line: null };
}

export function errorLine(error: DefinitionError): string {
  const { step, field, line, message } = error;
  if (step !== null && field !== null) return `step ${step} · ${field}: ${message}`;
  if (step !== null) return `step ${step}: ${message}`;
  if (field !== null) return `${field}: ${message}`;
  if (line !== null) return `line ${line}: ${message}`;
  return message;
}
