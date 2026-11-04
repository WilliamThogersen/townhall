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
  readonly submit: (input: AddRepoInput) => Promise<void>;
  readonly cancel: () => void;
  readonly dismissError: () => void;
}

const START: FlowState = { formOpen: false, busy: false, error: null, focusToken: 0 };

type AddRepo = ReturnType<typeof useRepos>["add"];
type SetFlow = (update: (state: FlowState) => FlowState) => void;

async function addFrom(source: () => Promise<AddRepoInput | null>, add: AddRepo, setFlow: SetFlow): Promise<void> {
  setFlow((state) => ({ ...state, busy: true, error: null }));
  try {
    const input = await source();
    if (input !== null) await add(input);
    setFlow((state) => ({ ...state, busy: false, formOpen: false }));
  } catch (e) {
    setFlow((state) => ({ ...state, busy: false, error: toMessage(e) }));
  }
}

async function pickedInput(): Promise<AddRepoInput | null> {
  const path = await pickFolder();
  return path === null ? null : { path };
}

export function useAddRepo(): AddRepoFlow {
  const { add } = useRepos();
  const [state, setFlow] = useState<FlowState>(START);
  const desktop = isDesktop();

  const begin = useCallback(() => {
    if (desktop) {
      void addFrom(pickedInput, add, setFlow);
      return;
    }
    setFlow((current) => ({ ...current, formOpen: true, error: null, focusToken: current.focusToken + 1 }));
  }, [add, desktop]);

  const submit = useCallback((input: AddRepoInput) => addFrom(async () => input, add, setFlow), [add]);
  const cancel = useCallback(() => setFlow((current) => ({ ...current, formOpen: false, error: null })), []);
  const dismissError = useCallback(() => setFlow((current) => ({ ...current, error: null })), []);

  return useMemo(
    () => ({ ...state, desktop, begin, submit, cancel, dismissError }),
    [state, desktop, begin, submit, cancel, dismissError],
  );
}
