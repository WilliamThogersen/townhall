import { useRef } from "react";
import { WEEKDAYS } from "./format";
import type { HeatModel } from "./heat";
import { useHeatTip } from "./useHeatTip";

const LEVELS = [0, 1, 2, 3, 4] as const;
const LABELLED_DAYS = [0, 2, 4] as const;
const FIRST_ROW = 2;

export interface HeatmapProps {
  readonly model: HeatModel;
  readonly label: string;
}

function HeatGrid({ model, active }: { readonly model: HeatModel; readonly active: number | null }) {
  return (
    <div className="th-heat-grid" aria-hidden="true">
      {model.months.map((month) => (
        <span
          key={`month-${month.column}`}
          className="th-heat-month"
          style={{ gridColumn: `${month.column} / span ${month.span}` }}
        >
          {month.name}
        </span>
      ))}
      {model.cells.map((cell) => (
        <span
          key={cell.index}
          className="th-heat-cell"
          data-level={cell.level}
          data-index={cell.index}
          data-active={cell.index === active ? "true" : undefined}
          style={{ gridColumn: cell.column, gridRow: cell.row }}
        />
      ))}
      {LABELLED_DAYS.map((day) => (
        <span key={day} className="th-heat-day" style={{ gridRow: day + FIRST_ROW }}>
          {WEEKDAYS[day]}
        </span>
      ))}
    </div>
  );
}

function Legend() {
  return (
    <span className="th-heat-legend" aria-hidden="true">
      less{" "}
      {LEVELS.map((level) => (
        <span key={level} className="th-heat-cell" data-level={level} />
      ))}{" "}
      more
    </span>
  );
}

function MonthTable({ model }: { readonly model: HeatModel }) {
  return (
    <div className="th-sr-only">
      <table>
        <caption>{model.tableCaption}</caption>
        <tbody>
          {model.monthTotals.map((month) => (
            <tr key={month.label}>
              <th scope="row">{month.label}</th>
              <td>{month.total}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function Heatmap({ model, label }: HeatmapProps) {
  const mapRef = useRef<HTMLDivElement>(null);
  const { tip, ...handlers } = useHeatTip(mapRef, model.cells.length);
  const tipText = tip === null ? undefined : model.cells[tip.index]?.tip;
  return (
    <div className="th-heatmap" ref={mapRef} tabIndex={0} role="group" aria-label={label} {...handlers}>
      <HeatGrid model={model} active={tipText === undefined ? null : (tip?.index ?? null)} />
      <div className="th-heat-foot">
        <span>{model.caption}</span>
        <Legend />
      </div>
      <div
        className="th-heat-tip"
        role="status"
        hidden={tipText === undefined}
        style={tip ? { left: tip.left, top: tip.top } : undefined}
      >
        {tipText}
      </div>
      <MonthTable model={model} />
    </div>
  );
}
