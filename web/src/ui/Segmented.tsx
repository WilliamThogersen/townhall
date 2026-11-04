export type SegmentedOption<T extends string> = {
  value: T;
  label: string;
  tone?: "danger";
};

type SegmentedProps<T extends string> = {
  name: string;
  label: string;
  value: T;
  options: readonly SegmentedOption<T>[];
  onChange: (next: T) => void;
};

export function Segmented<T extends string>({ name, label, value, options, onChange }: SegmentedProps<T>) {
  return (
    <div className="th-segmented" role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <label key={option.value} data-tone={option.tone}>
          <input
            type="radio"
            name={name}
            value={option.value}
            checked={option.value === value}
            onChange={() => onChange(option.value)}
          />
          <span>{option.label}</span>
        </label>
      ))}
    </div>
  );
}
