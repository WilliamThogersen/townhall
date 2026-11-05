import { isDesktop } from "../api/base";
import { logError } from "../log/report";

async function call<T>(cmd: string): Promise<T> {
  if (!isDesktop()) throw new Error("desktop commands are only available in the app");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd);
}

export const pickFolder = (): Promise<string | null> => call("pick_folder");

const EXTERNAL_PROTOCOLS = new Set(["http:", "https:", "mailto:"]);

function externalUrl(href: string): string | null {
  try {
    const url = new URL(href);
    return EXTERNAL_PROTOCOLS.has(url.protocol) ? url.href : null;
  } catch {
    return null;
  }
}

export async function openExternal(href: string): Promise<void> {
  if (!isDesktop()) throw new Error("desktop commands are only available in the app");
  const url = externalUrl(href);
  if (!url) throw new Error(`refusing to open ${href}`);
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

export function installExternalLinks(doc: Document): () => void {
  const onClick = (event: MouseEvent) => {
    if (!isDesktop() || event.defaultPrevented) return;
    const target = event.target;
    if (!(target instanceof Element)) return;
    const anchor = target.closest("a[href]");
    if (!(anchor instanceof HTMLAnchorElement)) return;
    if (anchor.origin === doc.location.origin) return;
    const url = externalUrl(anchor.href);
    if (!url) return;
    event.preventDefault();
    openExternal(url).catch((e) => logError("web.open_link", "open link failed", e));
  };
  doc.addEventListener("click", onClick);
  return () => doc.removeEventListener("click", onClick);
}
