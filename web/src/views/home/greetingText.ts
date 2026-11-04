export type TimeOfDay = "morning" | "afternoon" | "evening";

export type Clock = () => Date;

export const systemClock: Clock = () => new Date();

const MORNING_FROM = 5;
const AFTERNOON_FROM = 12;
const EVENING_FROM = 18;

export function timeOfDay(now: Date): TimeOfDay {
  const hour = now.getHours();
  if (hour >= MORNING_FROM && hour < AFTERNOON_FROM) return "morning";
  if (hour >= AFTERNOON_FROM && hour < EVENING_FROM) return "afternoon";
  return "evening";
}

export function firstName(userName: string | null | undefined): string | null {
  const first = userName?.trim().split(/\s+/)[0];
  return first ? first : null;
}

export function greetingText(now: Date, userName: string | null | undefined): string {
  const name = firstName(userName);
  const greeting = `good ${timeOfDay(now)}`;
  return name ? `${greeting}, ${name}` : greeting;
}
