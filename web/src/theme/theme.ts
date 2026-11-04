export type ThemeChoice = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

export interface ThemeState {
  readonly choice: ThemeChoice;
  readonly resolved: ResolvedTheme;
}

export const THEME_STORAGE_KEY = "th-theme";

const DEFAULT_CHOICE: ThemeChoice = "dark";
const THEME_CHOICES: readonly string[] = ["dark", "light", "system"];
const LIGHT_SCHEME_QUERY = "(prefers-color-scheme: light)";

let state: ThemeState | null = null;
let listeners: ReadonlySet<() => void> = new Set();
let watchingSystem = false;

function isThemeChoice(value: unknown): value is ThemeChoice {
  return typeof value === "string" && THEME_CHOICES.includes(value);
}

function lightSchemeQuery(): MediaQueryList | null {
  return typeof window.matchMedia === "function" ? window.matchMedia(LIGHT_SCHEME_QUERY) : null;
}

export function readThemeChoice(): ThemeChoice {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeChoice(stored) ? stored : DEFAULT_CHOICE;
  } catch (error) {
    console.warn("theme choice not read, using dark", error);
    return DEFAULT_CHOICE;
  }
}

export function resolveTheme(choice: ThemeChoice): ResolvedTheme {
  if (choice !== "system") return choice;
  return lightSchemeQuery()?.matches ? "light" : "dark";
}

export function applyTheme(resolved: ResolvedTheme): void {
  document.documentElement.setAttribute("data-theme", resolved);
}

export function getThemeState(): ThemeState {
  if (state === null) {
    const choice = readThemeChoice();
    state = { choice, resolved: resolveTheme(choice) };
  }
  return state;
}

export function subscribeTheme(listener: () => void): () => void {
  listeners = new Set([...listeners, listener]);
  return () => {
    listeners = new Set([...listeners].filter((current) => current !== listener));
  };
}

function commitTheme(choice: ThemeChoice): void {
  state = { choice, resolved: resolveTheme(choice) };
  applyTheme(state.resolved);
  listeners.forEach((listener) => listener());
}

function storeThemeChoice(choice: ThemeChoice): void {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, choice);
  } catch (error) {
    console.warn("theme choice not saved, applying it for this session", error);
  }
}

export function saveThemeChoice(choice: ThemeChoice): void {
  storeThemeChoice(choice);
  commitTheme(choice);
}

function followSystemTheme(): void {
  if (getThemeState().choice === "system") commitTheme("system");
}

function watchSystemTheme(): void {
  if (watchingSystem) return;
  const query = lightSchemeQuery();
  if (!query) return;
  query.addEventListener("change", followSystemTheme);
  watchingSystem = true;
}

export function initTheme(): void {
  commitTheme(readThemeChoice());
  watchSystemTheme();
}
