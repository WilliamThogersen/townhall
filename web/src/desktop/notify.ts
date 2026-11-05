import { isDesktop } from "../api/base";
import { logError } from "../log/report";

/** Native notification that an agent's session died. No-op outside the desktop shell. */
export async function notifyAgentDied(name: string): Promise<void> {
  if (!isDesktop()) return;
  try {
    const n = await import("@tauri-apps/plugin-notification");
    let granted = await n.isPermissionGranted();
    if (!granted) granted = (await n.requestPermission()) === "granted";
    if (granted) n.sendNotification({ title: "Agent stopped", body: `${name}: session ended` });
  } catch (e) {
    logError("web.notification", "notification failed", e);
  }
}
