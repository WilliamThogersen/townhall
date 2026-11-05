const MARKER = /\[(?:image|pasted image)(?:\s*#?\d+|:[^\]]*)?\]/gi;

const basename = (path: string): string => path.slice(path.lastIndexOf("/") + 1);

/**
 * Drops the raw text an image or file was pasted as (`[Image #1]`, or a line holding just its path),
 * since the thumbnail or card shows it instead. Only applies when the message has attachments.
 */
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
