import type { ReactNode } from "react";
import { Logo } from "./Logo";

const EMPTY_LOGO_WIDTH = 108;

export interface EmptyStateProps {
  readonly title: string;
  readonly children?: ReactNode;
  readonly action?: ReactNode;
  readonly logo?: boolean;
}

export function EmptyState({ title, children, action, logo = false }: EmptyStateProps) {
  return (
    <div className="th-empty">
      {logo ? <Logo width={EMPTY_LOGO_WIDTH} /> : null}
      <h2 className="th-empty-title">{title}</h2>
      {children ? <p>{children}</p> : null}
      {action}
    </div>
  );
}
