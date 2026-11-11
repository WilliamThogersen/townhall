import { ApiError } from "./errors";
import { bool, list, listOf, nullable, num, oneOf, record, str, strings, type Check, type Guard } from "./guardKit";
import { request, TomlBody } from "./request";
import type { DefinitionError, Pipeline, PipelineStepSummary } from "./types";

const origin = oneOf<"user" | "plugin">(["user", "plugin"]);
const kind = oneOf<DefinitionError["kind"]>(["agent", "workflow"]);

const parseStep: Check<PipelineStepSummary> = (value, what) => {
  const r = record(value, what);
  return { id: str(r.id, `${what}.id`), agent: str(r.agent, `${what}.agent`) };
};

const parsePipeline: Guard<Pipeline> = (value) => {
  const r = record(value, "pipeline");
  return {
    name: str(r.name, "pipeline.name"),
    description: str(r.description, "pipeline.description"),
    repos: strings(r.repos, "pipeline.repos"),
    default: bool(r.default, "pipeline.default"),
    work: nullable(r.work, "pipeline.work", str),
    steps: list(r.steps, "pipeline.steps", parseStep),
    source: str(r.source, "pipeline.source"),
    updatedAt: str(r.updatedAt, "pipeline.updatedAt"),
    origin: origin(r.origin, "pipeline.origin"),
    plugin: nullable(r.plugin, "pipeline.plugin", str),
  };
};

const parseName: Guard<string> = (value) => str(value, "pipeline name");

const parseDefinitionError: Check<DefinitionError> = (value, what) => {
  const r = record(value, what);
  return {
    kind: kind(r.kind, `${what}.kind`),
    name: nullable(r.name, `${what}.name`, str),
    step: nullable(r.step, `${what}.step`, str),
    field: nullable(r.field, `${what}.field`, str),
    message: str(r.message, `${what}.message`),
    line: nullable(r.line, `${what}.line`, num),
  };
};

const path = (name: string) => `/library/workflows/${encodeURIComponent(name)}`;

export const listPipelines = (signal?: AbortSignal): Promise<readonly Pipeline[]> =>
  request("GET", "/library/workflows", listOf(parsePipeline, "pipelines"), undefined, signal);

export const getPipeline = (name: string, signal?: AbortSignal): Promise<Pipeline> =>
  request("GET", path(name), parsePipeline, undefined, signal);

export const savePipeline = (name: string, source: string, signal?: AbortSignal): Promise<Pipeline> =>
  request("PUT", path(name), parsePipeline, new TomlBody(source), signal);

export const deletePipeline = (name: string, signal?: AbortSignal): Promise<string> =>
  request("DELETE", path(name), parseName, undefined, signal);

export function definitionErrors(error: unknown): readonly DefinitionError[] {
  if (!(error instanceof ApiError) || !Array.isArray(error.data)) return [];
  return list(error.data, "errors", parseDefinitionError);
}
