import { useRef, type KeyboardEvent, type ReactNode } from 'react';

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
  ariaLabel?: string;
}

export function Tabs<T extends string>({
  items,
  activeId,
  onChange,
  variant = 'underline',
  className = '',
  ariaLabel,
}: TabsProps<T>) {
  const tabListRef = useRef<HTMLDivElement>(null);

  const handleKeyDown = (e: KeyboardEvent<HTMLButtonElement>, currentIndex: number) => {
    const enabledTabs = items
      .map((item, idx) => ({ item, idx }))
      .filter(({ item }) => !item.disabled);

    if (enabledTabs.length === 0) return;

    const currentPos = enabledTabs.findIndex(({ idx }) => idx === currentIndex);
    let nextPos = -1;

    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      nextPos = (currentPos + 1) % enabledTabs.length;
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      nextPos = (currentPos - 1 + enabledTabs.length) % enabledTabs.length;
    } else if (e.key === 'Home') {
      e.preventDefault();
      nextPos = 0;
    } else if (e.key === 'End') {
      e.preventDefault();
      nextPos = enabledTabs.length - 1;
    }

    if (nextPos !== -1) {
      const target = enabledTabs[nextPos];
      onChange(target.item.id);

      // Focus the newly active tab button
      setTimeout(() => {
        if (tabListRef.current) {
          const buttons = tabListRef.current.querySelectorAll<HTMLButtonElement>('button[role="tab"]');
          buttons[target.idx]?.focus();
        }
      }, 0);
    }
  };

  return (
    <div
      ref={tabListRef}
      className={`cavia-tabs tabs-${variant} ${className}`}
      role="tablist"
      aria-label={ariaLabel ?? 'Navigation tabs'}
    >
      {items.map((tab, idx) => {
        const isActive = tab.id === activeId;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            disabled={tab.disabled}
            className={`tab-item ${isActive ? 'tab-active' : ''}`}
            onClick={() => !tab.disabled && onChange(tab.id)}
            onKeyDown={(e) => handleKeyDown(e, idx)}
          >
            {tab.icon && (
              <span className="tab-icon" aria-hidden="true">
                {tab.icon}
              </span>
            )}
            <span className="tab-label">{tab.label}</span>
            {tab.count !== undefined && <span className="tab-count">{tab.count}</span>}
          </button>
        );
      })}
    </div>
  );
}
