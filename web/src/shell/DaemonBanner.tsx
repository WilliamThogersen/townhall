import { useState } from "react";
import { useAgentsContext } from "../agents/AgentsProvider";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";

const DAEMON_PREFIX = "cannot reach the daemon: ";

export function daemonErrorText(message: string): string {
  return message.startsWith(DAEMON_PREFIX) ? message : `${DAEMON_PREFIX}${message}`;
}

export function DaemonBanner() {
  const { error, refresh } = useAgentsContext();
  const [retrying, setRetrying] = useState(false);
  if (error === null) return null;

  const retry = async () => {
    setRetrying(true);
    try {
      await refresh();
    } finally {
      setRetrying(false);
    }
  };

  const action = (
    <Button size="sm" busy={retrying} busyLabel="retrying…" onClick={() => void retry()}>
      retry
    </Button>
  );
  return (
    <Banner level="error" action={action}>
      {daemonErrorText(error)}
    </Banner>
  );
}
