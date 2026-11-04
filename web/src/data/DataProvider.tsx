import { useEffect, useMemo, useState, type ReactNode } from "react";
import { connectEvents, type ConnectEvents } from "../api/events";
import { DataContext } from "./dataContext";
import type { ConnectionStatus } from "./types";

export interface DataProviderProps {
  readonly children: ReactNode;
  readonly connect?: ConnectEvents;
}

export function DataProvider({ children, connect = connectEvents }: DataProviderProps) {
  const [connection, setConnection] = useState<ConnectionStatus>("connecting");
  const [refresh, setRefresh] = useState(0);

  useEffect(() => {
    let openedBefore = false;
    const refetch = () => setRefresh((n) => n + 1);
    const events = connect(
      (event) => {
        if (event.type === "resync") refetch();
      },
      {
        onStatus: (status) => {
          setConnection(status);
          if (status !== "open") return;
          if (openedBefore) refetch();
          openedBefore = true;
        },
      },
    );
    return () => events.close();
  }, [connect]);

  const value = useMemo(() => ({ connection, refresh }), [connection, refresh]);
  return <DataContext value={value}>{children}</DataContext>;
}
