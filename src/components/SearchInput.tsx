import { forwardRef, useImperativeHandle, useRef, type InputHTMLAttributes, type KeyboardEvent } from 'react';
import { Loader2, Search, X } from 'lucide-react';

interface SearchInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange'> {
  value: string;
  onChange: (value: string) => void;
  onClear?: () => void;
  shortcut?: string;
  placeholder?: string;
  className?: string;
  isLoading?: boolean;
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
    isLoading = false,
    onKeyDown,
    ...props
  },
  forwardedRef
) {
  const inputRef = useRef<HTMLInputElement>(null);

  useImperativeHandle(forwardedRef, () => inputRef.current as HTMLInputElement);

  const handleClear = () => {
    onChange('');
    onClear?.();
    inputRef.current?.focus();
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape' && value) {
      e.stopPropagation();
      handleClear();
    }
    onKeyDown?.(e);
  };

  return (
    <div className={`cavia-search-input-wrap ${disabled ? 'disabled' : ''} ${className}`}>
      {isLoading ? (
        <Loader2 size={18} className="search-icon search-spinner animate-spin" aria-hidden="true" />
      ) : (
        <Search size={18} className="search-icon" aria-hidden="true" />
      )}
      <input
        ref={inputRef}
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={disabled}
        className="cavia-search-field"
        aria-label="Search"
        {...props}
      />
      {value ? (
        <button
          type="button"
          onClick={handleClear}
          className="search-clear-btn"
          aria-label="Clear search query"
        >
          <X size={14} aria-hidden="true" />
        </button>
      ) : (
        shortcut && <kbd className="search-shortcut" aria-hidden="true">{shortcut}</kbd>
      )}
    </div>
  );
});
