import { toMessage } from "../api/errors";
import { getSettings, putSetting } from "../api/settings";
import { logWarn } from "../log/report";
import { forgetStoredSetting, importBrowserSettings, type PutSetting } from "./browserImport";
import {
  clampAgentsAtOnce,
  DEFAULT_SETTINGS,
  isSettingValue,
  SETTING_KEYS,
  type SettingKey,
  type Settings,
} from "./settings";

export type SettingsProblemKind = "load" | "save" | "import";

export interface SettingsProblem {
  readonly kind: SettingsProblemKind;
  readonly message: string;
}

export interface SettingsState {
  readonly settings: Settings;
  readonly loading: boolean;
  readonly problem: SettingsProblem | null;
}

export interface SettingsStoreDeps {
  readonly load: () => Promise<Settings>;
  readonly put: PutSetting;
  readonly storage: () => Storage | null;
}

export interface SettingsStore {
  readonly getState: () => SettingsState;
  readonly subscribe: (listener: () => void) => () => void;
  readonly ensureLoaded: () => void;
  readonly reload: () => void;
  readonly update: (patch: Partial<Settings>) => void;
  readonly dismissProblem: () => void;
}

type Phase = "idle" | "loading" | "ready" | "failed";

interface Change {
  readonly key: SettingKey;
  readonly value: Settings[SettingKey];
}

interface Write extends Change {
  readonly id: number;
}

interface Inner {
  readonly phase: Phase;
  readonly confirmed: Settings;
  readonly pending: readonly Write[];
  readonly problem: SettingsProblem | null;
}

interface Cell {
  readonly inner: () => Inner;
  readonly state: () => SettingsState;
  readonly change: (next: (inner: Inner) => Inner) => void;
  readonly subscribe: (listener: () => void) => () => void;
}

const DAEMON: SettingsStoreDeps = {
  load: () => getSettings(),
  put: putSetting,
  storage: () => window.localStorage,
};

function applyWrite(settings: Settings, write: Write): Settings {
  return Object.freeze({ ...settings, [write.key]: write.value });
}

function view(inner: Inner): SettingsState {
  return Object.freeze({
    settings: inner.pending.reduce(applyWrite, inner.confirmed),
    loading: inner.phase === "loading",
    problem: inner.problem,
  });
}

function createCell(): Cell {
  let inner: Inner = { phase: "idle", confirmed: DEFAULT_SETTINGS, pending: [], problem: null };
  let state = view(inner);
  let listeners: ReadonlySet<() => void> = new Set();
  return {
    inner: () => inner,
    state: () => state,
    change: (next) => {
      inner = next(inner);
      state = view(inner);
      listeners.forEach((listener) => listener());
    },
    subscribe: (listener) => {
      listeners = new Set([...listeners, listener]);
      return () => {
        listeners = new Set([...listeners].filter((known) => known !== listener));
      };
    },
  };
}

function storageOf(deps: SettingsStoreDeps): Storage | null {
  try {
    return deps.storage();
  } catch (error) {
    logWarn("web.storage_unavailable", "warn: browser storage is not available", error);
    return null;
  }
}

function clearProblem(problem: SettingsProblem | null, kind: SettingsProblemKind): SettingsProblem | null {
  return problem?.kind === kind ? null : problem;
}

function without(pending: readonly Write[], write: Write): readonly Write[] {
  return pending.filter((known) => known.id !== write.id);
}

async function importStored(cell: Cell, deps: SettingsStoreDeps): Promise<void> {
  const outcome = await importBrowserSettings(cell.inner().confirmed, {
    storage: storageOf(deps),
    put: deps.put,
    onSaved: (confirmed) => cell.change((inner) => ({ ...inner, confirmed })),
  });
  if (outcome.message === null) return;
  const problem: SettingsProblem = {
    kind: "import",
    message: `settings from this browser not imported: ${outcome.message}. townhall tries again next time`,
  };
  cell.change((inner) => ({ ...inner, problem }));
}

async function load(cell: Cell, deps: SettingsStoreDeps): Promise<void> {
  let confirmed: Settings;
  try {
    confirmed = await deps.load();
  } catch (error) {
    const problem: SettingsProblem = { kind: "load", message: `settings not loaded: ${toMessage(error)}` };
    cell.change((inner) => ({ ...inner, phase: "failed", problem }));
    return;
  }
  cell.change((inner) => ({ ...inner, phase: "ready", confirmed, problem: clearProblem(inner.problem, "load") }));
  await importStored(cell, deps);
}

async function save(cell: Cell, deps: SettingsStoreDeps, write: Write): Promise<void> {
  let confirmed: Settings;
  try {
    confirmed = await deps.put(write.key, write.value);
  } catch (error) {
    const problem: SettingsProblem = { kind: "save", message: `settings not saved: ${toMessage(error)}` };
    cell.change((inner) => ({ ...inner, pending: without(inner.pending, write), problem }));
    return;
  }
  cell.change((inner) => ({
    ...inner,
    confirmed,
    pending: without(inner.pending, write),
    problem: clearProblem(inner.problem, "save"),
  }));
  forgetStoredSetting(storageOf(deps), write.key);
}

function changesOf(current: Settings, patch: Partial<Settings>): readonly Change[] {
  return SETTING_KEYS.flatMap((key) => {
    const asked: unknown =
      key === "agentsAtOnce" && typeof patch.agentsAtOnce === "number"
        ? clampAgentsAtOnce(patch.agentsAtOnce)
        : patch[key];
    return asked !== current[key] && isSettingValue(key, asked) ? [{ key, value: asked }] : [];
  });
}

export function createSettingsStore(deps: SettingsStoreDeps = DAEMON): SettingsStore {
  const cell = createCell();
  let queue: Promise<void> = Promise.resolve();
  let lastId = 0;
  const enqueue = (task: () => Promise<void>) => {
    queue = queue.then(task);
  };
  const startLoad = () => {
    cell.change((inner) => ({ ...inner, phase: "loading" }));
    enqueue(() => load(cell, deps));
  };
  const write = (change: Change) => {
    lastId += 1;
    const pending: Write = { ...change, id: lastId };
    cell.change((inner) => ({ ...inner, pending: [...inner.pending, pending] }));
    enqueue(() => save(cell, deps, pending));
  };
  return {
    getState: cell.state,
    subscribe: cell.subscribe,
    ensureLoaded: () => {
      const { phase } = cell.inner();
      if (phase === "idle" || phase === "failed") startLoad();
    },
    reload: () => {
      if (cell.inner().phase !== "loading") startLoad();
    },
    update: (patch) => changesOf(cell.state().settings, patch).forEach(write),
    dismissProblem: () => {
      if (cell.inner().problem !== null) cell.change((inner) => ({ ...inner, problem: null }));
    },
  };
}
