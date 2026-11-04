import { wsApiBase, type PageLocation } from "./base";
import { parseAgent, parseAgentInfos, parseInfo, parseString } from "./guards";
import { request } from "./request";
import type { Agent, AgentInfo, CreateAgentInput, Info } from "./types";

const enc = encodeURIComponent;

export const listAgents = (signal?: AbortSignal): Promise<readonly AgentInfo[]> =>
  request("GET", "/agents", parseAgentInfos, undefined, signal);

export const getInfo = (signal?: AbortSignal): Promise<Info> =>
  request("GET", "/info", parseInfo, undefined, signal);

export const createAgent = (input: CreateAgentInput): Promise<Agent> =>
  request("POST", "/agents", parseAgent, input);

export const stopAgent = (name: string): Promise<string> =>
  request("DELETE", `/agents/${enc(name)}`, parseString);

export const sendText = (name: string, text: string): Promise<string> =>
  request("POST", `/agents/${enc(name)}/send`, parseString, { text });

export const getOutput = (name: string): Promise<string> =>
  request("GET", `/agents/${enc(name)}/output`, parseString);

export function streamUrl(
  name: string,
  loc: PageLocation = window.location,
  win: object = window,
): string {
  return `${wsApiBase(loc, win)}/agents/${enc(name)}/stream`;
}
