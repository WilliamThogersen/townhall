import type { ReactNode } from "react";

export interface KbdProps {
  readonly children: ReactNode;
}

export function Kbd({ children }: KbdProps) {
  return <kbd className="th-kbd">{children}</kbd>;
}
