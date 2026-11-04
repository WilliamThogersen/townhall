import { wsBase } from "./base";
import { parseAgent, parseAgents, parseString } from "./guards";
import { request } from "./request";
import type { Agent, CreateAgentInput } from "./types";

const enc = encodeURIComponent;

export const listAgents = (signal?: AbortSignal): Promise<readonly Agent[]> =>
  request("GET", "/agents", parseAgents, undefined, signal);

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
  loc: Location = window.location,
  win: object = window,
): string {
  const base =
    wsBase(win) ?? `${loc.protocol === "https:" ? "wss:" : "ws:"}//${loc.host}/api`;
  return `${base}/agents/${enc(name)}/stream`;
}
