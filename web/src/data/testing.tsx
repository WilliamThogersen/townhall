import { useState, type ReactNode } from "react";
import type { ConnectEvents, ConnectionStatus, EventsOptions } from "../api/events";
import type { ServerEvent } from "../api/types";
import { DataProvider } from "./DataProvider";

export interface FakeEvents {
  readonly connect: ConnectEvents;
  readonly emit: (event: ServerEvent) => void;
  readonly setStatus: (status: ConnectionStatus) => void;
  readonly isConnected: () => boolean;
}

interface Listener {
  readonly onEvent: (event: ServerEvent) => void;
  readonly options: EventsOptions;
}

export function createFakeEvents(initial: ConnectionStatus = "open"): FakeEvents {
  let listener: Listener | null = null;
  const connect: ConnectEvents = (onEvent, options = {}) => {
    const own: Listener = { onEvent, options };
    listener = own;
    options.onStatus?.(initial);
    return {
      close: () => {
        if (listener === own) listener = null;
      },
    };
  };
  return {
    connect,
    emit: (event) => listener?.onEvent(event),
    setStatus: (status) => listener?.options.onStatus?.(status),
    isConnected: () => listener !== null,
  };
}

export interface TestDataProviderProps {
  readonly children: ReactNode;
  readonly events?: FakeEvents;
}

export function TestDataProvider({ children, events }: TestDataProviderProps) {
  const [fallback] = useState(createFakeEvents);
  return <DataProvider connect={(events ?? fallback).connect}>{children}</DataProvider>;
}
