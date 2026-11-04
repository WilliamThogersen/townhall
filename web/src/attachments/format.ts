const KILO = 1024;
const UNITS = ["KB", "MB", "GB"] as const;
const TAG_LETTERS = 4;

function compact(value: number): string {
  if (value >= 10) return String(Math.round(value));
  const rounded = Math.round(value * 10) / 10;
  return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
}

export function formatSize(bytes: number): string {
  if (bytes < KILO) return `${bytes} B`;
  const step = Math.min(Math.floor(Math.log(bytes) / Math.log(KILO)), UNITS.length);
  return `${compact(bytes / KILO ** step)} ${UNITS[step - 1]}`;
}

export function fileExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return "";
  return name.slice(dot + 1).toLowerCase();
}

export function extensionTag(name: string): string {
  return fileExtension(name).slice(0, TAG_LETTERS) || "file";
}

const pad = (value: number): string => String(value).padStart(2, "0");

export function formatClock(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
