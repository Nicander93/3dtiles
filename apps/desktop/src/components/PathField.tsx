import { useId, type ReactNode } from 'react';

type Props = {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onBlur?: () => void;
  placeholder?: string;
  pickLabel?: string;
  onPick?: () => void;
  hint?: ReactNode;
  error?: ReactNode;
  feedback?: ReactNode;
};

export function PathField({
  label,
  value,
  onChange,
  onBlur,
  placeholder,
  pickLabel = '选择目录',
  onPick,
  hint,
  error,
  feedback,
}: Props) {
  const id = useId();
  return (
    <div className="field path-field">
      <label htmlFor={id}>{label}</label>
      <div className="row">
        <input
          id={id}
          className="input"
          value={value}
          title={value || undefined}
          placeholder={placeholder}
          onChange={(e) => onChange(e.target.value)}
          onBlur={onBlur}
        />
        {onPick ? (
          <button className="btn" type="button" onClick={onPick}>
            {pickLabel}
          </button>
        ) : null}
      </div>
      {feedback}
      {error ? <div className="field-error">{error}</div> : null}
      {hint && !error ? <div className="field-hint">{hint}</div> : null}
    </div>
  );
}
