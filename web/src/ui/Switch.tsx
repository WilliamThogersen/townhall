type SwitchProps = {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
};

export function Switch({ checked, onChange, label }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className="th-switch"
      onClick={() => onChange(!checked)}
    />
  );
}
