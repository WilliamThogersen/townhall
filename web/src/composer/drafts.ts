const HOME = "\0home";
const drafts = new Map<string, string>();

const keyOf = (conversationId: string | null): string => conversationId ?? HOME;

export function readDraft(conversationId: string | null): string {
  return drafts.get(keyOf(conversationId)) ?? "";
}

export function writeDraft(conversationId: string | null, text: string): void {
  if (text === "") drafts.delete(keyOf(conversationId));
  else drafts.set(keyOf(conversationId), text);
}

export function clearDraft(conversationId: string | null): void {
  drafts.delete(keyOf(conversationId));
}

export function clearAllDrafts(): void {
  drafts.clear();
}
