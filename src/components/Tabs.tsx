import type { ReactNode } from 'react';

export interface TabItem<T extends string> {
  id: T;
  label: string;
  count?: number | string;
  icon?: ReactNode;
  disabled?: boolean;
}

interface TabsProps<T extends string> {
  items: TabItem<T>[];
  activeId: T;
  onChange: (id: T) => void;
  variant?: 'underline' | 'pill' | 'segmented';
  className?: string;
}

export function Tabs<T extends string>({
  items,
  activeId,
  onChange,
  variant = 'underline',
  className = '',
}: TabsProps<T>) {
  return (
    <div className={`cavia-tabs tabs-${variant} ${className}`} role="tablist">
      {items.map((tab) => {
        const isActive = tab.id === activeId;
        return (
          <button
            key={tab.id}
            role="tab"
            aria-selected={isActive}
            disabled={tab.disabled}
            className={`tab-item ${isActive ? 'tab-active' : ''}`}
            onClick={() => !tab.disabled && onChange(tab.id)}
          >
            {tab.icon && <span className="tab-icon">{tab.icon}</span>}
            <span className="tab-label">{tab.label}</span>
            {tab.count !== undefined && <span className="tab-count">{tab.count}</span>}
          </button>
        );
      })}
    </div>
  );
}

