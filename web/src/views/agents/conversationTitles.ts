/** The conversation's title, or its id while the title is unknown. */
export function conversationLabel(id: string, titles: ReadonlyMap<string, string>): string {
  const title = titles.get(id)?.trim();
  return title ? title : id;
}
