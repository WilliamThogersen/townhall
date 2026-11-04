import { vi } from "vitest";

export interface FakeColorScheme {
  readonly setLight: (light: boolean) => void;
  readonly listenerCount: () => number;
}

export function stubColorScheme(initiallyLight: boolean): FakeColorScheme {
  let light = initiallyLight;
  let listeners: ReadonlyArray<() => void> = [];
  const lightSchemeQuery = (media: string) => ({
    media,
    get matches() {
      return light;
    },
    addEventListener: (_type: string, listener: () => void) => {
      listeners = [...listeners, listener];
    },
  });
  vi.stubGlobal("matchMedia", vi.fn(lightSchemeQuery));
  return {
    setLight: (next) => {
      light = next;
      listeners.forEach((listener) => listener());
    },
    listenerCount: () => listeners.length,
  };
}
