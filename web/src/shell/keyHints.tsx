import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useState,
  type ReactNode,
} from "react";

export interface KeyHint {
  readonly keys: string;
  readonly label: string;
}

interface Entry {
  readonly id: string;
  readonly hints: readonly KeyHint[];
}

type Register = (id: string, hints: readonly KeyHint[] | null) => void;

const NO_HINTS: readonly KeyHint[] = [];
const RegisterContext = createContext<Register | null>(null);
const CurrentContext = createContext<readonly KeyHint[]>(NO_HINTS);

function withEntry(entries: readonly Entry[], id: string, hints: readonly KeyHint[] | null) {
  if (hints === null) return entries.filter((e) => e.id !== id);
  const known = entries.some((e) => e.id === id);
  return known
    ? entries.map((e) => (e.id === id ? { id, hints } : e))
    : [...entries, { id, hints }];
}

export function KeyHintsProvider({ children }: { readonly children: ReactNode }) {
  const [entries, setEntries] = useState<readonly Entry[]>([]);
  const register = useCallback<Register>(
    (id, hints) => setEntries((prev) => withEntry(prev, id, hints)),
    [],
  );
  const current = entries.at(-1)?.hints ?? NO_HINTS;
  return (
    <RegisterContext value={register}>
      <CurrentContext value={current}>{children}</CurrentContext>
    </RegisterContext>
  );
}

export function useKeyHints(hints: readonly KeyHint[]): void {
  const register = useContext(RegisterContext);
  const id = useId();
  const key = JSON.stringify(hints);
  useEffect(() => {
    if (!register) return;
    register(id, JSON.parse(key) as readonly KeyHint[]);
    return () => register(id, null);
  }, [register, id, key]);
}

export function useCurrentKeyHints(): readonly KeyHint[] {
  return useContext(CurrentContext);
}
