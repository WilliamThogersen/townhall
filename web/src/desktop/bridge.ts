import { isDesktop } from "../api/base";

async function call<T>(cmd: string): Promise<T> {
  if (!isDesktop()) throw new Error("desktop commands are only available in the app");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd);
}

export const pickFolder = (): Promise<string | null> => call("pick_folder");
