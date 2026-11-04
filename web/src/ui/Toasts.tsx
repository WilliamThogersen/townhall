import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

export type ToastInput = {
  glyph?: ReactNode;
  title: string;
  text: string;
  context?: string;
  action?: { label: string; onClick: () => void };
};

type Toast = ToastInput & { id: string };

type ToastApi = {
  push: (t: ToastInput) => string;
  dismiss: (id: string) => void;
};

const MAX_TOASTS = 3;
const TOAST_MS = 6000;
const BUTTON_CLASS = "th-btn th-btn--ghost th-btn--sm";

const ToastContext = createContext<ToastApi | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<readonly Toast[]>([]);
  const lastId = useRef(0);

  const dismiss = useCallback((id: string) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const push = useCallback((input: ToastInput) => {
    lastId.current += 1;
    const id = `toast-${lastId.current}`;
    setToasts((current) => [...current, { ...input, id }].slice(-MAX_TOASTS));
    return id;
  }, []);

  const api = useMemo(() => ({ push, dismiss }), [push, dismiss]);

  return (
    <ToastContext value={api}>
      {children}
      <div className="th-toasts" role="status">
        {toasts.map((toast) => (
          <ToastView key={toast.id} toast={toast} onDismiss={dismiss} />
        ))}
      </div>
    </ToastContext>
  );
}

export function useToasts(): ToastApi {
  const api = useContext(ToastContext);
  if (!api) throw new Error("useToasts must be used inside a ToastProvider");
  return api;
}

function useDismissTimer(id: string, paused: boolean, onDismiss: (id: string) => void) {
  const remaining = useRef(TOAST_MS);
  useEffect(() => {
    if (paused) return undefined;
    const startedAt = Date.now();
    const handle = window.setTimeout(() => onDismiss(id), remaining.current);
    return () => {
      window.clearTimeout(handle);
      remaining.current = Math.max(0, remaining.current - (Date.now() - startedAt));
    };
  }, [id, paused, onDismiss]);
}

function ToastView({ toast, onDismiss }: { toast: Toast; onDismiss: (id: string) => void }) {
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  useDismissTimer(toast.id, hovered || focused, onDismiss);

  const runAction = () => {
    toast.action?.onClick();
    onDismiss(toast.id);
  };

  return (
    <div
      className="th-toast"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)}
      onBlur={(event) => setFocused(event.currentTarget.contains(event.relatedTarget))}
    >
      {toast.glyph}
      <span>
        <b>{toast.title}</b> {toast.text}
        {toast.context ? ` · ${toast.context}` : null}
      </span>{" "}
      {toast.action ? (
        <button type="button" className={BUTTON_CLASS} onClick={runAction}>
          {toast.action.label}
        </button>
      ) : (
        <button type="button" className={BUTTON_CLASS} aria-label="Dismiss" onClick={() => onDismiss(toast.id)}>
          ×
        </button>
      )}
    </div>
  );
}
