import { num, record } from "./guardKit";
import type { SystemStats } from "./types";

export function parseSystem(value: unknown): SystemStats {
  const r = record(value, "system");
  return {
    cpuPercent: num(r.cpuPercent, "system.cpuPercent"),
    memoryUsedBytes: num(r.memoryUsedBytes, "system.memoryUsedBytes"),
    memoryTotalBytes: num(r.memoryTotalBytes, "system.memoryTotalBytes"),
    diskUsedBytes: num(r.diskUsedBytes, "system.diskUsedBytes"),
    diskTotalBytes: num(r.diskTotalBytes, "system.diskTotalBytes"),
  };
}
