import type { FormEvent, KeyboardEvent, ReactNode, RefObject } from "react";
import { Button } from "../../ui/Button";

export const HERO_PLACEHOLDER = "what should we work on? name a repo, or describe the code";

export interface HeroComposerProps {
  readonly value: string;
  readonly onChange: (text: string) => void;
  readonly onSubmit: (text: string) => void;
  readonly busy: boolean;
  readonly tools?: ReactNode;
  readonly inputRef: RefObject<HTMLTextAreaElement | null>;
}

export function HeroComposer({ value, onChange, onSubmit, busy, tools, inputRef }: HeroComposerProps) {
  const canStart = value.trim() !== "" && !busy;
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (canStart) onSubmit(value);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Escape") e.currentTarget.blur();
    if (e.key !== "Enter" || e.shiftKey || e.nativeEvent.isComposing) return;
    e.preventDefault();
    if (canStart) onSubmit(value);
  };
  return (
    <form className="th-composer th-composer--hero" onSubmit={submit}>
      <div className="th-composer-box">
        <span className="th-composer-sign" aria-hidden="true">
          ›
        </span>
        <textarea
          ref={inputRef}
          className="th-composer-input"
          rows={3}
          placeholder={HERO_PLACEHOLDER}
          aria-label="Describe the work"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
        />
        <div className="th-composer-tools">
          {tools}
          <span className="th-help">⏎ start · ⇧⏎ new line</span>
          <Button type="submit" variant="primary" size="sm" disabled={!canStart} busy={busy} busyLabel="starting…">
            start
          </Button>
        </div>
      </div>
    </form>
  );
}
