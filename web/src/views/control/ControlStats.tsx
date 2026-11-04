import type { ControlStat } from "./controlModel";

export function ControlStats({ stats }: { readonly stats: readonly ControlStat[] }) {
  return (
    <div className="th-trace-stats">
      {stats.map((stat) => (
        <div key={stat.label} className="th-trace-stat">
          <span>{stat.label}</span>
          <b>{stat.value}</b>
        </div>
      ))}
    </div>
  );
}
