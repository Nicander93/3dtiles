import type { ReactNode } from 'react';

type Props = {
  title: string;
  children: ReactNode;
};

export function FormSection({ title, children }: Props) {
  return (
    <section className="form-section">
      <h2 className="form-section__title">{title}</h2>
      {children}
    </section>
  );
}
