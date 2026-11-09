import { useEffect, useRef, type ChangeEvent, type Ref } from "react";
import "./repos.css";

export interface FieldProps {
  readonly label: string;
  readonly value: string;
  readonly placeholder: string;
  readonly className?: string;
  readonly inputRef?: Ref<HTMLInputElement>;
  readonly onChange: (value: string) => void;
}

export function Field({ label, value, placeholder, className, inputRef, onChange }: FieldProps) {
  return (
    <label className={className ? `th-repo-field ${className}` : "th-repo-field"}>
      <span>{label}</span>
      <input
        ref={inputRef}
        className="th-input"
        value={value}
        placeholder={placeholder}
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={(e: ChangeEvent<HTMLInputElement>) => onChange(e.target.value)}
      />
    </label>
  );
}

export function useFocusOn(token: number) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    ref.current?.focus();
  }, [token]);
  return ref;
}
