import { FORGE_KINDS, type ForgeKind } from "../settings/settings";
import { bool, listOf, oneOf, record, str } from "./guardKit";
import { request } from "./request";

export interface DetectedAccount {
  readonly forge: ForgeKind;
  readonly login: string;
  readonly host: string;
  readonly registered: boolean;
}

function parseDetected(value: unknown): DetectedAccount {
  const fields = record(value, "detected account");
  return Object.freeze({
    forge: oneOf(FORGE_KINDS)(fields.forge, "detected account.forge"),
    login: str(fields.login, "detected account.login"),
    host: str(fields.host, "detected account.host"),
    registered: bool(fields.registered, "detected account.registered"),
  });
}

export const getDetectedAccounts = (signal?: AbortSignal): Promise<readonly DetectedAccount[]> =>
  request("GET", "/forge-accounts/detected", listOf(parseDetected, "detected accounts"), undefined, signal);
