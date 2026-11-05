import { useEffect, useRef } from "react";
import type { ConversationDetail } from "../../api/types";

export function unknownAttachmentIds(detail: ConversationDetail): readonly string[] {
  const known = new Set(detail.attachments.map((attachment) => attachment.id));
  const named = detail.entries.flatMap((entry) => (entry.kind === "you" ? entry.attachments : []));
  return [...new Set(named.filter((id) => !known.has(id)))];
}

export function useAttachmentRefresh(detail: ConversationDetail | null, reload: () => void): void {
  const asked = useRef("");
  const unknown = detail === null ? "" : unknownAttachmentIds(detail).join(" ");
  useEffect(() => {
    if (unknown === "" || unknown === asked.current) return;
    asked.current = unknown;
    reload();
  }, [unknown, reload]);
}
