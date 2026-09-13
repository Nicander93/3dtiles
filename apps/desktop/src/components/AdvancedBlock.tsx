import type { ReactNode } from 'react';

type Props = {
  title?: string;
  children: ReactNode;
  open?: boolean;
  onToggle?: (open: boolean) => void;
};

export function AdvancedBlock({ title = '高级设置', children, open, onToggle }: Props) {
  return (
    <details
      className="advanced-block"
      open={open}
      onToggle={(e) => onToggle?.((e.target as HTMLDetailsElement).open)}
    >
      <summary>{title}</summary>
      <div className="advanced-block__body">{children}</div>
    </details>
  );
}
