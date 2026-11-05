import type { TraceRow } from "./traceRows";

export type TreeAction =
  | { readonly kind: "focus"; readonly key: string }
  | { readonly kind: "toggle"; readonly key: string }
  | { readonly kind: "open"; readonly row: TraceRow };

const focus = (row: TraceRow | undefined): TreeAction | null => (row ? { kind: "focus", key: row.key } : null);

function right(rows: readonly TraceRow[], index: number): TreeAction | null {
  const row = rows[index];
  if (row?.expanded === false) return { kind: "toggle", key: row.key };
  const next = rows[index + 1];
  return row?.expanded === true && next?.parent === row.key ? focus(next) : null;
}

function left(rows: readonly TraceRow[], index: number): TreeAction | null {
  const row = rows[index];
  if (row?.expanded === true) return { kind: "toggle", key: row.key };
  return focus(rows.find((candidate) => candidate.key === row?.parent));
}

export function treeAction(rows: readonly TraceRow[], key: string, pressed: string): TreeAction | null {
  const index = rows.findIndex((row) => row.key === key);
  const row = rows[index];
  if (row === undefined) return null;
  switch (pressed) {
    case "ArrowDown":
      return focus(rows[index + 1]);
    case "ArrowUp":
      return focus(rows[index - 1]);
    case "Home":
      return focus(rows[0]);
    case "End":
      return focus(rows.at(-1));
    case "ArrowRight":
      return right(rows, index);
    case "ArrowLeft":
      return left(rows, index);
    case "Enter":
      return { kind: "open", row };
    default:
      return null;
  }
}
