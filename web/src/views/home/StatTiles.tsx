import type { MetricKey } from "./metrics";
import type { TileModel } from "./tiles";

export interface StatTilesProps {
  readonly tiles: readonly TileModel[];
  readonly selected: MetricKey;
  readonly onSelect: (key: MetricKey) => void;
}

export function StatTiles({ tiles, selected, onSelect }: StatTilesProps) {
  return (
    <div className="th-tiles">
      {tiles.map(({ metric, value, delta }) => (
        <button
          key={metric.key}
          type="button"
          className="th-tile"
          aria-pressed={metric.key === selected}
          onClick={() => onSelect(metric.key)}
        >
          <span className="th-tile-label">{metric.label}</span>
          <span className="th-tile-value">{value}</span>
          <span className="th-tile-delta" data-dir={delta.dir}>
            {delta.text}
          </span>
        </button>
      ))}
    </div>
  );
}
