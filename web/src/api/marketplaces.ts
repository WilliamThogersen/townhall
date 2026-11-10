import { parseMarketplace, parseMarketplaces } from "./pluginGuards";
import { request } from "./request";
import type { AddMarketplaceInput, Marketplace } from "./types";

const path = (name: string): string => `/marketplaces/${encodeURIComponent(name)}`;

export const listMarketplaces = (signal?: AbortSignal): Promise<readonly Marketplace[]> =>
  request("GET", "/marketplaces", parseMarketplaces, undefined, signal);

export const addMarketplace = (input: AddMarketplaceInput): Promise<Marketplace> =>
  request("POST", "/marketplaces", parseMarketplace, input);

export const refreshMarketplace = (name: string): Promise<Marketplace> =>
  request("POST", `${path(name)}/refresh`, parseMarketplace);

export const removeMarketplace = (name: string): Promise<Marketplace> =>
  request("DELETE", path(name), parseMarketplace);
