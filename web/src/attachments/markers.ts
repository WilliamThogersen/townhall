const MARKER = /\[(?:image|pasted image)(?:\s*#?\d+|:[^\]]*)?\]/gi;

const basename = (path: string): string => path.slice(path.lastIndexOf("/") + 1);

export function stripAttachmentMarkers(text: string, attachedNames: readonly string[]): string {
  if (attachedNames.length === 0) return text;
  const names = new Set(attachedNames);
  const kept = text.split("\n").filter((line) => {
    const rest = line.replace(MARKER, "").trim();
    if (rest === "" && line.trim() !== "") return false;
    return !(!/\s/.test(rest) && names.has(basename(rest)));
  });
  const cleaned = kept.map((line) => line.replace(MARKER, "").replace(/ {2,}/g, " ").trim());
  return cleaned.join("\n").replace(/\n{3,}/g, "\n\n").trim();
}
