import { useRef } from "react";

type StepperProps = {
  value: number;
  min: number;
  max: number;
  onChange: (next: number) => void;
  label: string;
};

const clamp = (n: number, min: number, max: number) => Math.min(max, Math.max(min, n));

export function Stepper({ value, min, max, onChange, label }: StepperProps) {
  const fewerRef = useRef<HTMLButtonElement>(null);
  const moreRef = useRef<HTMLButtonElement>(null);

  const step = (delta: -1 | 1) => {
    const next = clamp(value + delta, min, max);
    onChange(next);
    if (delta > 0 && next >= max) fewerRef.current?.focus();
    if (delta < 0 && next <= min) moreRef.current?.focus();
  };

  return (
    <div className="th-stepper" role="group" aria-label={label}>
      <button ref={fewerRef} type="button" aria-label="Fewer" disabled={value <= min} onClick={() => step(-1)}>
        {"−"}
      </button>
      <output aria-live="polite">{value}</output>
      <button ref={moreRef} type="button" aria-label="More" disabled={value >= max} onClick={() => step(1)}>
        +
      </button>
    </div>
  );
}
