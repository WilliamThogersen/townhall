import { useCallback, useMemo, useState } from "react";
import { isDesktop } from "../api/base";
import { toMessage } from "../api/errors";
import type { AddRepoInput } from "../api/types";
import { useRepos } from "../data";
import { pickFolder } from "../desktop/bridge";

interface FlowState {
  readonly formOpen: boolean;
  readonly busy: boolean;
  readonly error: string | null;
  readonly focusToken: number;
}

export interface AddRepoFlow extends FlowState {
  readonly desktop: boolean;
  readonly begin: () => void;
  readonly browse: () => Promise<string | null>;
  readonly submit: (input: AddRepoInput) => Promise<void>;
  readonly cancel: () => void;
  readonly dismissError: () => void;
}

const START: FlowState = { formOpen: false, busy: false, error: null, focusToken: 0 };

type AddRepo = ReturnType<typeof useRepos>["add"];
type SetFlow = (update: (state: FlowState) => FlowState) => void;

async function addFrom(input: AddRepoInput, add: AddRepo, setFlow: SetFlow): Promise<void> {
  setFlow((state) => ({ ...state, busy: true, error: null }));
  try {
    await add(input);
    setFlow((state) => ({ ...state, busy: false, formOpen: false }));
  } catch (e) {
    setFlow((state) => ({ ...state, busy: false, error: toMessage(e) }));
  }
}

export function useAddRepo(): AddRepoFlow {
  const { add } = useRepos();
  const [state, setFlow] = useState<FlowState>(START);
  const desktop = isDesktop();

  const begin = useCallback(() => {
    setFlow((current) => ({ ...current, formOpen: true, error: null, focusToken: current.focusToken + 1 }));
  }, []);

  const browse = useCallback(async () => {
    setFlow((current) => ({ ...current, error: null }));
    try {
      return await pickFolder();
    } catch (e) {
      setFlow((current) => ({ ...current, error: toMessage(e) }));
      return null;
    }
  }, []);

  const submit = useCallback((input: AddRepoInput) => addFrom(input, add, setFlow), [add]);
  const cancel = useCallback(() => setFlow((current) => ({ ...current, formOpen: false, error: null })), []);
  const dismissError = useCallback(() => setFlow((current) => ({ ...current, error: null })), []);

  return useMemo(
    () => ({ ...state, desktop, begin, browse, submit, cancel, dismissError }),
    [state, desktop, begin, browse, submit, cancel, dismissError],
  );
}
