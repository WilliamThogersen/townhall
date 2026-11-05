const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export const NOW_AT = 0.76;
export const MAX_TICKS = 8;
export const MIN_SPAN = 10 * MINUTE;

const TICK_STEPS: readonly number[] = [5, 10, 15, 30, 60, 120, 180, 360, 720, 1440].map((m) => m * MINUTE);

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

export interface TimeAxis {
  readonly start: number;
  readonly end: number;
  readonly step: number;
  readonly ticks: number;
  readonly now: number;
  readonly today: boolean;
}

export interface Tick {
  readonly at: number;
  readonly label: string;
}

export interface SpanPercents {
  readonly start: number;
  readonly len: number;
}

export function startOfDay(time: number): number {
  const day = new Date(time);
  day.setHours(0, 0, 0, 0);
  return day.getTime();
}

function floorLocal(time: number, step: number): number {
  const offset = new Date(time).getTimezoneOffset() * MINUTE;
  return Math.floor((time - offset) / step) * step + offset;
}

function axisWith(step: number, earliest: number, now: number): TimeAxis {
  const start = floorLocal(earliest, step);
  const ticks = Math.max(1, Math.ceil((now - start) / NOW_AT / step));
  return { start, end: start + ticks * step, step, ticks, now, today: start >= startOfDay(now) };
}

export function timeAxis(earliest: number, now: number): TimeAxis {
  const from = Math.min(earliest, now - MIN_SPAN);
  const fits = TICK_STEPS.map((step) => axisWith(step, from, now)).find((axis) => axis.ticks <= MAX_TICKS);
  return fits ?? axisWith(Math.ceil((now - from) / NOW_AT / MAX_TICKS / DAY) * DAY, from, now);
}

const round = (value: number) => Math.round(value * 1000) / 1000;

export function percentAt(axis: TimeAxis, time: number): number {
  const fraction = (time - axis.start) / (axis.end - axis.start);
  return round(Math.min(1, Math.max(0, fraction)) * 100);
}

export function spanPercents(axis: TimeAxis, from: number, to: number): SpanPercents {
  const start = percentAt(axis, from);
  return { start, len: round(percentAt(axis, Math.max(from, to)) - start) };
}

const pad = (n: number) => String(n).padStart(2, "0");

export function dateLabel(time: number): string {
  const date = new Date(time);
  return `${date.getDate()} ${MONTHS[date.getMonth()]}`;
}

export function tickLabel(time: number, step: number): string {
  const date = new Date(time);
  const midnight = date.getHours() === 0 && date.getMinutes() === 0;
  if (step >= DAY || (midnight && step >= HOUR)) return dateLabel(time);
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function ticksOf(axis: TimeAxis): readonly Tick[] {
  return Array.from({ length: axis.ticks }, (_, i) => {
    const time = axis.start + i * axis.step;
    return { at: percentAt(axis, time), label: tickLabel(time, axis.step) };
  });
}

export function windowLabel(axis: TimeAxis): string {
  return axis.today ? "today" : `since ${dateLabel(axis.start)}`;
}

export function durationLabel(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / SECOND));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return minutes % 60 ? `${hours}h ${minutes % 60}m` : `${hours}h`;
  const days = Math.floor(hours / 24);
  return hours % 24 ? `${days}d ${hours % 24}h` : `${days}d`;
}
