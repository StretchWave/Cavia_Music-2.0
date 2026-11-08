import { forwardRef, type InputHTMLAttributes } from 'react';
import { Search, X } from 'lucide-react';

interface SearchInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange'> {
  value: string;
  onChange: (value: string) => void;
  onClear?: () => void;
  shortcut?: string;
  placeholder?: string;
  className?: string;
}

export const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(function SearchInput(
  {
    value,
    onChange,
    onClear,
    shortcut = '⌘ K',
    placeholder = 'Search tracks, albums, artists...',
    className = '',
    disabled,
    ...props
  },
  ref
) {
  return (
    <div className={`cavia-search-input-wrap ${disabled ? 'disabled' : ''} ${className}`}>
      <Search size={18} className="search-icon" aria-hidden="true" />
      <input
        ref={ref}
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        className="cavia-search-field"
        aria-label="Search"
        {...props}
      />
      {value ? (
        <button
          type="button"
          onClick={() => {
            onChange('');
            onClear?.();
          }}
          className="search-clear-btn"
          aria-label="Clear search query"
        >
          <X size={14} />
        </button>
      ) : (
        shortcut && <kbd className="search-shortcut">{shortcut}</kbd>
      )}
    </div>
  );
});

