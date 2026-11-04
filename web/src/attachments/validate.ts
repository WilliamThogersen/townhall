import { MAX_ATTACHMENT_BYTES } from "../api/attachments";
import { fileExtension } from "./format";

const IMAGE_TYPES: readonly string[] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

const DOCUMENT_TYPES: readonly string[] = [
  "application/pdf",
  "application/json",
  "application/xml",
  "application/yaml",
  "application/x-yaml",
  "application/toml",
  "application/javascript",
  "application/typescript",
  "application/sql",
  "application/x-sh",
];

const CODE_EXTENSIONS: readonly string[] = [
  "txt", "md", "markdown", "log", "csv", "tsv", "json", "jsonl", "yaml", "yml", "toml", "xml",
  "ini", "conf", "cfg", "har", "diff", "patch", "pdf",
  "ts", "tsx", "js", "jsx", "mjs", "cjs", "css", "scss", "less", "html", "htm", "vue", "svelte",
  "svg", "rs", "go", "py", "rb", "php", "java", "kt", "kts", "swift", "c", "h", "cc", "cpp", "hpp",
  "cs", "m", "scala", "lua", "pl", "r", "dart", "ex", "exs", "erl", "hs", "clj", "elm", "zig",
  "sh", "bash", "zsh", "fish", "ps1", "sql", "graphql", "gql", "proto", "tf", "hcl", "gradle",
  "lock", "twig", "blade", "liquid", "env",
];

const CODE_NAMES: readonly string[] = [
  "dockerfile", "makefile", "justfile", "procfile", "gemfile", "rakefile", "brewfile",
  "jenkinsfile", "vagrantfile", "license", "readme", "changelog",
];

export const ACCEPTED_FILES = [
  ...IMAGE_TYPES,
  "text/*",
  ...DOCUMENT_TYPES,
  ...CODE_EXTENSIONS.map((ext) => `.${ext}`),
].join(",");

export function isImageFile(file: Pick<File, "type">): boolean {
  return IMAGE_TYPES.includes(file.type);
}

function isAcceptedType(file: File): boolean {
  if (isImageFile(file) || file.type.startsWith("text/")) return true;
  if (DOCUMENT_TYPES.includes(file.type)) return true;
  const ext = fileExtension(file.name);
  if (ext !== "") return CODE_EXTENSIONS.includes(ext);
  return CODE_NAMES.includes(file.name.toLowerCase());
}

export function rejectReason(file: File): string | null {
  if (file.size > MAX_ATTACHMENT_BYTES) return `${file.name} is over 20 MB`;
  if (file.size === 0) return `${file.name} is empty`;
  if (!isAcceptedType(file)) return `${file.name} is not an image, text, code or pdf`;
  return null;
}
