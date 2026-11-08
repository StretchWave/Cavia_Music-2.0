import type { ReactNode } from 'react';

export function IconButton({ label, icon, disabled = false, title }: { label: string; icon: ReactNode; disabled?: boolean; title?: string }) {
  return <button className="icon-button" aria-label={label} title={title ?? label} disabled={disabled}>{icon}</button>;
}
