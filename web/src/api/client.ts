import { wsApiBase, type PageLocation } from "./base";
import { record } from "./guardKit";
import { parseAgentInfos, parseInfo, parseString } from "./guards";
import { request } from "./request";
import type { AgentInfo, AgentKey, Info } from "./types";

const enc = encodeURIComponent;

export const listAgents = (signal?: AbortSignal): Promise<readonly AgentInfo[]> =>
  request("GET", "/agents", parseAgentInfos, undefined, signal);

export const getInfo = (signal?: AbortSignal): Promise<Info> =>
  request("GET", "/info", parseInfo, undefined, signal);

export const stopAgent = (name: string): Promise<string> =>
  request("DELETE", `/agents/${enc(name)}`, parseString);

export const sendText = (name: string, text: string): Promise<string> =>
  request("POST", `/agents/${enc(name)}/send`, parseString, { text });

export const sendKey = (name: string, key: AgentKey): Promise<string> =>
  request("POST", `/agents/${enc(name)}/send`, parseString, { key });

export interface AllowedTool {
  readonly agentType: string;
  readonly tool: string;
  readonly added: boolean;
}

function parseAllowedTool(value: unknown): AllowedTool {
  const r = record(value, "allowed tool");
  return { agentType: parseString(r.agentType), tool: parseString(r.tool), added: r.added === true };
}

export const allowTool = (name: string, tool: string): Promise<AllowedTool> =>
  request("POST", `/agents/${enc(name)}/allow`, parseAllowedTool, { tool });

export const getOutput = (name: string): Promise<string> =>
  request("GET", `/agents/${enc(name)}/output`, parseString);

export function streamUrl(
  name: string,
  loc: PageLocation = window.location,
  win: object = window,
): string {
  return `${wsApiBase(loc, win)}/agents/${enc(name)}/stream`;
}

export function attachUrl(
  name: string,
  loc: PageLocation = window.location,
  win: object = window,
): string {
  return `${wsApiBase(loc, win)}/agents/${enc(name)}/attach`;
}
