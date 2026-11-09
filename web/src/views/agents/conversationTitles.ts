export function conversationLabel(id: string, titles: ReadonlyMap<string, string>): string {
  const title = titles.get(id)?.trim();
  return title ? title : id;
}
