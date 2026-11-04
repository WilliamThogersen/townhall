import { toMessage } from "../api/errors";
import type { ServerEvent } from "../api/types";
import type { Cell } from "./cell";
import { loadableFor, type Settle } from "./resources";

export interface Loader {
  readonly load: (key: string) => void;
  readonly ensure: (key: string) => void;
  readonly record: (event: ServerEvent) => void;
  readonly abortAll: () => void;
}

interface Flight {
  readonly abort: AbortController;
  readonly replay: readonly ServerEvent[];
  readonly again: boolean;
}

const ERROR_PREFIX = /^error:\s*/i;

export const cleanError = (e: unknown): string => toMessage(e).replace(ERROR_PREFIX, "");

export function createLoader(cell: Cell): Loader {
  const flights = new Map<string, Flight>();

  const finish = (key: string, abort: AbortController, settle: Settle) => {
    const flight = flights.get(key);
    if (abort.signal.aborted || flight?.abort !== abort) return;
    flights.delete(key);
    cell.update((state) => settle(state, flight.replay));
    if (flight.again) load(key);
  };

  function load(key: string) {
    const flight = flights.get(key);
    if (flight) {
      flights.set(key, { ...flight, again: true });
      return;
    }
    const loadable = loadableFor(key);
    const abort = new AbortController();
    flights.set(key, { abort, replay: [], again: false });
    cell.update(loadable.request);
    loadable.fetch(abort.signal).then(
      (settle) => finish(key, abort, settle),
      (e: unknown) => finish(key, abort, (state) => loadable.fail(state, cleanError(e))),
    );
  }

  return {
    load,
    ensure: (key) => {
      const resource = loadableFor(key).read(cell.get());
      const fresh = resource?.settled === true && resource.data !== null;
      if (!flights.has(key) && !fresh) load(key);
    },
    record: (event) =>
      flights.forEach((flight, key) => flights.set(key, { ...flight, replay: [...flight.replay, event] })),
    abortAll: () => {
      flights.forEach((flight) => flight.abort.abort());
      flights.clear();
    },
  };
}
