import { useCallback, useRef, useState } from "react";
import { toMessage } from "../../api/errors";
import type { PluginPreview } from "../../api/types";
import type { PluginsState } from "../../data";

export type Run = <T>(action: () => Promise<T>) => Promise<T | undefined>;

export interface InstallFlow {
  readonly id: string | null;
  readonly preview: PluginPreview | null;
  readonly error: string | null;
  readonly busy: boolean;
  readonly begin: (id: string) => void;
  readonly install: (enable: boolean) => void;
  readonly cancel: () => void;
}

interface Shown {
  readonly id: string;
  readonly preview: PluginPreview | null;
  readonly error: string | null;
  readonly busy: boolean;
}

export function useInstallFlow(plugins: PluginsState, run: Run): InstallFlow {
  const [shown, setShown] = useState<Shown | null>(null);
  const current = useRef<string | null>(null);
  const { preview, install, patch } = plugins;

  const cancel = useCallback(() => {
    current.current = null;
    setShown(null);
  }, []);

  const begin = useCallback(
    (id: string) => {
      current.current = id;
      setShown({ id, preview: null, error: null, busy: false });
      preview(id).then(
        (answer) => current.current === id && setShown({ id, preview: answer, error: null, busy: false }),
        (e: unknown) => current.current === id && setShown({ id, preview: null, error: toMessage(e), busy: false }),
      );
    },
    [preview],
  );

  const confirm = useCallback(
    async (enable: boolean) => {
      if (shown === null || shown.preview === null) return;
      const { id, preview: answer } = shown;
      setShown({ ...shown, error: null, busy: true });
      try {
        await install(id, answer.permissionsHash);
      } catch (e) {
        if (current.current === id) setShown({ ...shown, error: toMessage(e), busy: false });
        return;
      }
      cancel();
      if (enable) await run(() => patch(id, { enabled: true }));
    },
    [shown, install, patch, run, cancel],
  );

  return {
    id: shown?.id ?? null,
    preview: shown?.preview ?? null,
    error: shown?.error ?? null,
    busy: shown?.busy ?? false,
    begin,
    install: (enable) => void confirm(enable),
    cancel,
  };
}
