export function clipboardFiles(data: DataTransfer | null): readonly File[] {
  if (data === null) return [];
  const files = Array.from(data.files);
  if (files.length > 0) return files;
  return Array.from(data.items)
    .filter((item) => item.kind === "file")
    .map((item) => item.getAsFile())
    .filter((file): file is File => file !== null);
}

export function hasClipboardText(data: DataTransfer | null): boolean {
  return data !== null && Array.from(data.types).includes("text/plain");
}
