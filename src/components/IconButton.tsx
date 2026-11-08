import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  icon: ReactNode;
  variant?: 'ghost' | 'panel' | 'subtle' | 'vermilion' | 'outline';
  size?: 'sm' | 'md' | 'lg';
  active?: boolean;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  {
    label,
    icon,
    variant = 'ghost',
    size = 'md',
    active = false,
    disabled = false,
    title,
    className = '',
    ...props
  },
  ref
) {
  return (
    <button
      ref={ref}
      type="button"
      className={`cavia-icon-btn icon-btn-${variant} icon-btn-${size} ${active ? 'active' : ''} ${className}`}
      aria-label={label}
      title={title ?? label}
      disabled={disabled}
      {...props}
    >
      {icon}
    </button>
  );
});
