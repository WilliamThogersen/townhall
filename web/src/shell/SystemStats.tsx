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
  const cells = [
    { key: "cpu", text: text.cpu, level: levelOf(stats.cpuPercent) },
    { key: "ram", text: text.ram, level: levelOf(share(stats.memoryUsedBytes, stats.memoryTotalBytes)) },
    { key: "disk", text: text.disk, level: levelOf(share(stats.diskUsedBytes, stats.diskTotalBytes)) },
  ];
  return (
    <span className="th-keybar-system">
      {cells.map((cell) => (
        <span key={cell.key} data-level={cell.level}>
          {cell.text}
        </span>
      ))}
    </span>
  );
}
