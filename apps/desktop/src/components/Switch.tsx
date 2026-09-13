import type { InputHTMLAttributes, ReactNode } from 'react';

type Props = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  children?: ReactNode;
} & Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'onChange' | 'checked'>;

export function Switch({ checked, onChange, children, id, ...rest }: Props) {
  return (
    <label className="switch" htmlFor={id}>
      <span className="switch__track">
        <input
          {...rest}
          id={id}
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
        />
        <i aria-hidden />
      </span>
      {children}
    </label>
  );
}
