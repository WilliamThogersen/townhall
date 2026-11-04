import { compactNumber } from "../home/format";
import type { ControlStat } from "./traceStats";

export function ControlStats({ stats }: { readonly stats: readonly ControlStat[] }) {
  return (
    <div className="th-trace-stats">
      {stats.map((stat) => (
        <div key={stat.label} className="th-trace-stat">
          <span>{stat.label}</span>
          <b>{compactNumber(stat.value)}</b>
        </div>
      ))}
    </div>
  );
}
