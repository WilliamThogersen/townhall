import { record, str } from "./guardKit";
import { request } from "./request";
import type { Profile, PutProfileInput } from "./types";

export const MAX_PROFILE_CHARS = 20_000;

export function parseProfile(value: unknown): Profile {
  return Object.freeze({ text: str(record(value, "profile").text, "profile.text") });
}

export const getProfile = (signal?: AbortSignal): Promise<Profile> =>
  request("GET", "/profile", parseProfile, undefined, signal);

export function putProfile(text: string): Promise<Profile> {
  const body: PutProfileInput = { text };
  return request("PUT", "/profile", parseProfile, body);
}

export const deleteProfile = (): Promise<Profile> => request("DELETE", "/profile", parseProfile);
