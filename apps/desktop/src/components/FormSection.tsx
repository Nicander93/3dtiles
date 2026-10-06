import type { ReactNode } from 'react';

type Props = {
  title: string;
  children: ReactNode;
  description?: string;
  columns?: number;
};

export function FormSection({ title, children, description, columns }: Props) {
  return (
    <section className="form-section">
      <div className="form-section__label">
        <h2 className="form-section__title">{title}</h2>
        {description ? <p>{description}</p> : null}
      </div>
      <div className={`form-section__body${columns ? ` form-grid cols-${columns}` : ''}`}>
        {children}
      </div>
    </section>
  );
}
