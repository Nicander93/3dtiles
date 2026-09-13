import type { ReactNode } from 'react';

type Props = {
  onReset?: () => void;
  resetLabel?: string;
  primaryLabel: string;
  onPrimary: () => void;
  primaryDisabled?: boolean;
  hint?: ReactNode;
};

export function SubmitBar({
  onReset,
  resetLabel = '重置',
  primaryLabel,
  onPrimary,
  primaryDisabled,
  hint,
}: Props) {
  return (
    <div className="footer-actions">
      {hint ? <div className="muted" style={{ flex: 1 }}>{hint}</div> : <div style={{ flex: 1 }} />}
      {onReset ? (
        <button className="btn" type="button" onClick={onReset}>
          {resetLabel}
        </button>
      ) : null}
      <button
        className="btn btn-primary"
        type="button"
        onClick={onPrimary}
        disabled={primaryDisabled}
      >
        {primaryLabel}
      </button>
    </div>
  );
}
