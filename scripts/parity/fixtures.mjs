const HOUR_MS = 3_600_000;
const GB = 1024 ** 3;

export function usage(now = Date.now()) {
  return {
    session: { usedPercent: 47, resetsAt: now + 2 * HOUR_MS, ratePerHour: 9 },
    weekly: { usedPercent: 20, resetsAt: now + 72 * HOUR_MS, ratePerHour: 0.6 },
    updatedAt: now,
    error: null,
  };
}

export function system() {
  return {
    cpuPercent: 23,
    memoryUsedBytes: 18.4 * GB,
    memoryTotalBytes: 36 * GB,
    diskUsedBytes: 412 * GB,
    diskTotalBytes: 994 * GB,
  };
}

export function envelope(data) {
  return { success: true, data, error: null };
}

export const OVERRIDES = new Map([
  ["/api/usage", () => envelope(usage())],
  ["/api/system", () => envelope(system())],
]);
