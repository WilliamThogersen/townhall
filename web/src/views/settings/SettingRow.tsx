import type { ReactNode } from "react";

export interface SettingGroupProps {
  readonly title: string;
  readonly children: ReactNode;
}

export function SettingGroup({ title, children }: SettingGroupProps) {
  return (
    <section className="th-setting-group">
      <h2>{title}</h2>
      <div className="th-setting-rows">{children}</div>
    </section>
  );
}

export interface SettingRowProps {
  readonly label: string;
  readonly help: ReactNode;
  readonly helpTone?: "danger";
  readonly details?: ReactNode;
  readonly wide?: boolean;
  readonly children: ReactNode;
}

export function SettingRow({ label, help, helpTone, details, wide, children }: SettingRowProps) {
  return (
    <div className="th-setting-row" data-wide={wide ? "true" : undefined}>
      <div>
        <div className="th-setting-label">{label}</div>
        <div className="th-setting-help" data-tone={helpTone}>
          {help}
        </div>
        {details}
      </div>
      {children}
    </div>
  );
}
