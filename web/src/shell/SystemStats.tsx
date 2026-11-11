import { useEffect, useState } from "react";
import { getSystem } from "../api/system";
import type { SystemStats as Stats } from "../api/types";
import { useDocumentVisible } from "../views/control/useDocumentVisible";

const POLL_MS = 5_000;
const GB = 1024 ** 3;
const WARN = 85;
const HOT = 95;

const gb = (bytes: number): string => {
  const value = bytes / GB;
  return value >= 100 ? String(Math.round(value)) : String(Math.round(value * 10) / 10);
};

export function formatSystem(s: Stats): { cpu: string; ram: string; disk: string } {
  return {
    cpu: `cpu ${Math.round(s.cpuPercent)}%`,
    ram: `ram ${gb(s.memoryUsedBytes)}/${gb(s.memoryTotalBytes)} GB`,
    disk: `disk ${gb(s.diskUsedBytes)}/${gb(s.diskTotalBytes)} GB`,
  };
}

const levelOf = (percent: number): "warn" | "hot" | undefined =>
  percent > HOT ? "hot" : percent > WARN ? "warn" : undefined;

const share = (used: number, total: number): number => (total > 0 ? (used / total) * 100 : 0);

const clamp = (percent: number): number => Math.min(100, Math.max(0, percent));

function useSystemStats(): Stats | null {
  const visible = useDocumentVisible();
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    if (!visible) return;
    const controller = new AbortController();
    const read = () =>
      getSystem(controller.signal).then(
        (next) => setStats(next),
        () => {
          if (!controller.signal.aborted) setStats(null);
        },
      );
    void read();
    const timer = setInterval(() => void read(), POLL_MS);
    return () => {
      controller.abort();
      clearInterval(timer);
    };
  }, [visible]);

  return stats;
}

export function SystemStats() {
  const stats = useSystemStats();
  if (!stats) return null;
  const text = formatSystem(stats);
  const percents = {
    cpu: stats.cpuPercent,
    ram: share(stats.memoryUsedBytes, stats.memoryTotalBytes),
    disk: share(stats.diskUsedBytes, stats.diskTotalBytes),
  };
  const cells = (["cpu", "ram", "disk"] as const).map((key) => ({
    key,
    text: text[key],
    percent: Math.round(clamp(percents[key])),
    level: levelOf(percents[key]),
  }));
  return (
    <span className="th-keybar-system">
      {cells.map((cell) => (
        <span key={cell.key} data-level={cell.level}>
          <span
            className="th-meter"
            role="meter"
            aria-label={`${cell.key} ${cell.percent}%`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={cell.percent}
          >
            <span className="th-meter-fill" style={{ width: `${cell.percent}%` }} />
          </span>
          {cell.text}
        </span>
      ))}
    </span>
  );
}
