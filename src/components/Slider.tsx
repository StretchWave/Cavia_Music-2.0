import { type ChangeEvent, useState, useEffect } from 'react';

interface SliderProps {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange?: (value: number) => void;
  onCommit?: (value: number) => void;
  disabled?: boolean;
  label: string;
  className?: string;
  showFill?: boolean;
}

export function Slider({
  value,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  onCommit,
  disabled = false,
  label,
  className = '',
  showFill = true,
}: SliderProps) {
  const [internalValue, setInternalValue] = useState(value);
  const [isInteracting, setIsInteracting] = useState(false);

  useEffect(() => {
    if (!isInteracting) {
      setInternalValue(value);
    }
  }, [value, isInteracting]);

  const displayVal = isInteracting ? internalValue : value;
  const percentage = Math.max(0, Math.min(100, ((displayVal - min) / (max - min || 1)) * 100));

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    const nextVal = Number(e.target.value);
    setInternalValue(nextVal);
    onChange?.(nextVal);
    if (!onCommit) {
      // If no commit callback provided, treat every change as immediate
    }
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLInputElement>) => {
    if (disabled) return;
    setIsInteracting(true);
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // Ignore if pointer capture fails in non-pointer contexts
    }
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLInputElement>) => {
    if (disabled) return;
    try {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) {
        e.currentTarget.releasePointerCapture(e.pointerId);
      }
    } catch {
      // Ignore
    }
    if (isInteracting) {
      setIsInteracting(false);
      onCommit?.(internalValue);
    }
  };

  const handleKeyUp = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (disabled) return;
    if (
      e.key === 'ArrowLeft' ||
      e.key === 'ArrowRight' ||
      e.key === 'ArrowUp' ||
      e.key === 'ArrowDown' ||
      e.key === 'Home' ||
      e.key === 'End' ||
      e.key === 'PageUp' ||
      e.key === 'PageDown'
    ) {
      onCommit?.(internalValue);
    }
  };

  return (
    <div className={`cavia-slider-wrap ${disabled ? 'disabled' : ''} ${className}`}>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={displayVal}
        onChange={handleChange}
        onPointerDown={handlePointerDown}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
        onKeyUp={handleKeyUp}
        disabled={disabled}
        aria-label={label}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={displayVal}
        aria-disabled={disabled}
        className="cavia-range-input"
        style={{
          background: showFill
            ? disabled
              ? `linear-gradient(to right, rgba(255, 255, 255, 0.15) ${percentage}%, var(--bg-hover) ${percentage}%)`
              : `linear-gradient(to right, var(--accent-vermilion) ${percentage}%, var(--bg-hover) ${percentage}%)`
            : undefined,
        }}
      />
    </div>
  );
}

