import type { ReactNode } from 'react';

interface EmptyStateProps {
  icon?: ReactNode;
  glyph?: string;
  eyebrow?: string;
  title: string | ReactNode;
  description: string;
  actions?: ReactNode;
  footnote?: string;
  className?: string;
}

export function EmptyState({
  icon,
  glyph,
  eyebrow,
  title,
  description,
  actions,
  footnote,
  className = '',
}: EmptyStateProps) {
  return (
    <section className={`cavia-empty-state ${className}`}>
      <div className="empty-state-art">
        {glyph && <span className="empty-glyph" aria-hidden="true">{glyph}</span>}
        {icon && <div className="empty-icon-frame">{icon}</div>}
      </div>

      <div className="empty-state-content">
        {eyebrow && <div className="empty-eyebrow">{eyebrow}</div>}
        <h2 className="empty-title">{title}</h2>
        <p className="empty-desc">{description}</p>

        {actions && <div className="empty-actions">{actions}</div>}

        {footnote && (
          <div className="empty-footnote">
            <span className="footnote-bullet" />
            <span>{footnote}</span>
          </div>
        )}
      </div>
    </section>
  );
}

