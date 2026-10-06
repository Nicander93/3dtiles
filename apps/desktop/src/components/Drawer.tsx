import { useEffect, useId, useRef, type ReactNode } from 'react';
import { X } from '@phosphor-icons/react';

type Props = {
  title: string;
  open: boolean;
  onClose: () => void;
  children: ReactNode;
};

/** Native modal makes the workspace inert and restores the opener's focus. */
export function Drawer({ title, open, onClose, children }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    if (open && !element.open) element.showModal();
    if (!open && element.open) element.close();
  }, [open]);

  return (
    <dialog
      ref={dialog}
      className="settings-drawer"
      aria-labelledby={titleId}
      onKeyDown={(event) => {
        if (event.key !== 'Tab') return;
        const controls = [
          ...event.currentTarget.querySelectorAll<HTMLElement>(
            'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex="0"]',
          ),
        ].filter((element) => element.getClientRects().length > 0);
        const first = controls[0],
          last = controls[controls.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last?.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first?.focus();
        }
      }}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClick={(event) => {
        if (event.target !== event.currentTarget) return;
        const bounds = event.currentTarget.getBoundingClientRect();
        if (
          event.clientX < bounds.left ||
          event.clientX > bounds.right ||
          event.clientY < bounds.top ||
          event.clientY > bounds.bottom
        )
          onClose();
      }}
    >
      <header className="settings-drawer__head">
        <h2 id={titleId}>{title}</h2>
        <button
          className="btn btn-ghost btn-sm"
          type="button"
          aria-label={`关闭${title}`}
          onClick={onClose}
        >
          <X size={18} />
        </button>
      </header>
      <div className="settings-drawer__body">{children}</div>
      <footer className="settings-drawer__foot">
        <span className="field-hint">参数用于当前任务</span>
        <button className="btn btn-primary" type="button" onClick={onClose}>
          完成
        </button>
      </footer>
    </dialog>
  );
}
