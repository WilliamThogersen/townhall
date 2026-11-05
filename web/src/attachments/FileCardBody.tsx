import { extensionTag } from "./format";

export interface FileCardBodyProps {
  readonly name: string;
  readonly detail: string;
}

export function FileCardBody({ name, detail }: FileCardBodyProps) {
  return (
    <>
      <svg className="th-file-icon" viewBox="0 0 16 20" width="16" height="20" aria-hidden="true" focusable="false">
        <path d="M1.5 1.5h8l5 5v12h-13z M9.5 1.5v5h5" fill="none" stroke="currentColor" strokeWidth="1" />
      </svg>
      <span className="th-file-text">
        <span className="th-file-name">{name}</span>
        <span className="th-file-size">{`${extensionTag(name).toUpperCase()} · ${detail}`}</span>
      </span>
    </>
  );
}
