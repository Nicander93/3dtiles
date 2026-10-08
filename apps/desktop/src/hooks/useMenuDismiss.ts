import { useEffect, type RefObject } from 'react';

export function useMenuDismiss<T extends HTMLElement>(
  ref: RefObject<T>,
  open: boolean,
  close: () => void,
) {
  useEffect(() => {
    if (!open) return;
    function pointer(event: PointerEvent) {
      if (!ref.current?.contains(event.target as Node)) close();
    }
    function key(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        close();
        ref.current?.querySelector<HTMLButtonElement>('button')?.focus();
      }
    }
    document.addEventListener('pointerdown', pointer);
    document.addEventListener('keydown', key);
    return () => {
      document.removeEventListener('pointerdown', pointer);
      document.removeEventListener('keydown', key);
    };
  }, [ref, open, close]);
}
