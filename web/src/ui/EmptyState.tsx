import type { ReactNode } from "react";

export interface EmptyStateProps {
  readonly title: string;
  readonly children?: ReactNode;
  readonly action?: ReactNode;
  readonly logo?: boolean;
}

export function EmptyState({ title, children, action }: EmptyStateProps) {
  return (
    <div className="th-empty">
      <h2 className="th-empty-title">{title}</h2>
      {children ? <p>{children}</p> : null}
      {action}
    </div>
  );
}
