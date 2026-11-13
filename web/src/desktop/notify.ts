import { isDesktop } from "../api/base";
import { logError } from "../log/report";

export interface DesktopNotification {
  readonly title: string;
  readonly body: string;
}

export async function notifyDesktop({ title, body }: DesktopNotification): Promise<boolean> {
  if (!isDesktop()) return false;
  try {
    const n = await import("@tauri-apps/plugin-notification");
    let granted = await n.isPermissionGranted();
    if (!granted) granted = (await n.requestPermission()) === "granted";
    if (granted) n.sendNotification({ title, body });
    return granted;
  } catch (e) {
    logError("web.notification", "notification failed", e);
    return false;
  }
}

/** Native notification that an agent's session died. No-op outside the desktop shell. */
export async function notifyAgentDied(name: string): Promise<void> {
  await notifyDesktop({ title: "Agent stopped", body: `${name}: session ended` });
}
