import { useEffect, useRef } from "react";

export function useReturnFocus(open: boolean) {
  const target = useRef<HTMLButtonElement>(null);
  const pending = useRef(false);
  useEffect(() => {
    if (open || !pending.current) return;
    pending.current = false;
    target.current?.focus();
  }, [open]);
  const arm = () => {
    pending.current = true;
  };
  return { target, arm };
}
