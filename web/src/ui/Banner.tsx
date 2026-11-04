import type { ReactNode } from "react";

export type BannerLevel = "error" | "warn" | "info";

type BannerProps = {
  level: BannerLevel;
  children: ReactNode;
  action?: ReactNode;
};

const LEVEL_CLASS: Readonly<Record<BannerLevel, string>> = {
  error: "th-banner",
  warn: "th-banner th-banner--warn",
  info: "th-banner th-banner--info",
};

export function Banner({ level, children, action }: BannerProps) {
  return (
    <div className={LEVEL_CLASS[level]} role={level === "error" ? "alert" : "status"}>
      <span className="th-level">{level}:</span>{" "}
      <span>{children}</span>
      {action ? " " : null}
      {action}
    </div>
  );
}
