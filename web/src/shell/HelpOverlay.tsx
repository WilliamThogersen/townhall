import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { trapTab } from "../attachments/focusTrap";
import { Kbd } from "../ui/Kbd";
import { KEY_MAP } from "./keyMap";
import "./shell.css";

export interface HelpOverlayProps {
  readonly onClose: () => void;
}

function useReturnFocusOnClose(): void {
  useEffect(() => {
    const opener = document.activeElement;
    return () => {
      if (opener instanceof HTMLElement) opener.focus();
    };
  }, []);
}

function useCapturedKeys(rootRef: { readonly current: HTMLElement | null }, onClose: () => void): void {
  const latest = useRef(onClose);
  useEffect(() => {
    latest.current = onClose;
  });
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const root = rootRef.current;
      if (root === null) return;
      if (e.key === "Escape") latest.current();
      else if (e.key === "Tab") trapTab(e, root);
      e.stopPropagation();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [rootRef]);
}

export function HelpOverlay({ onClose }: HelpOverlayProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  useReturnFocusOnClose();
  useEffect(() => closeRef.current?.focus(), []);
  useCapturedKeys(rootRef, onClose);
  return createPortal(
    <div className="th-help-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={rootRef} className="th-help-box" role="dialog" aria-modal="true" aria-label="keyboard shortcuts">
        <header className="th-help-head">
          <h2>keyboard shortcuts</h2>
          <button ref={closeRef} type="button" className="th-btn th-btn--ghost th-btn--sm" onClick={onClose}>
            close
          </button>
        </header>
        <div className="th-help-groups">
          {KEY_MAP.map((group) => (
            <section key={group.title} role="group" aria-label={group.title} className="th-help-group">
              <h3 className="th-section">{group.title}</h3>
              <dl>
                {group.entries.map((entry) => (
                  <div key={`${entry.keys} ${entry.label}`} className="th-help-row">
                    <dt>
                      <Kbd>{entry.keys}</Kbd>
                    </dt>
                    <dd>{entry.label}</dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
      </div>
    </div>,
    document.body,
  );
}
