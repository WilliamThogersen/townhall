import { createContext, useContext } from "react";
import type { ConnectionStatus } from "./types";

export interface DataContextValue {
  readonly connection: ConnectionStatus;
  readonly refresh: number;
}

export const DataContext = createContext<DataContextValue | null>(null);

export function useDataContext(): DataContextValue {
  const value = useContext(DataContext);
  if (!value) throw new Error("data hooks need a DataProvider");
  return value;
}

export function useConnection(): ConnectionStatus {
  return useDataContext().connection;
}
