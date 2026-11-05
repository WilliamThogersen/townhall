import { wsApiBase, type PageLocation } from "./base";
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
