import { useEffect, useMemo, type ReactNode } from "react";
import { connectEvents, type ConnectEvents } from "../api/events";
import { DataContext } from "./dataContext";
import { createDataStore } from "./store";

export interface DataProviderProps {
  readonly children: ReactNode;
  readonly connect?: ConnectEvents;
}

export function DataProvider({ children, connect = connectEvents }: DataProviderProps) {
  const store = useMemo(() => createDataStore({ connect }), [connect]);
  useEffect(() => {
    store.start();
    return () => store.stop();
  }, [store]);
  return <DataContext value={store}>{children}</DataContext>;
}
