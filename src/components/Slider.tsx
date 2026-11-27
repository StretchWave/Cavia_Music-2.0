import { type ChangeEvent, type CSSProperties, useState, useEffect, useRef } from 'react';

interface SliderProps {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange?: (value: number) => void;
  onCommit?: (value: number) => void;
  disabled?: boolean;
  label: string;
  ariaValueText?: string;
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
  ariaValueText,
  className = '',
  showFill = true,
}: SliderProps) {
  const [internalValue, setInternalValue] = useState(value);
  const internalValueRef = useRef(value);
  const [isInteracting, setIsInteracting] = useState(false);

  useEffect(() => {
    if (!isInteracting) {
      setInternalValue(value);
      internalValueRef.current = value;
    }
  }, [value, isInteracting]);

  const displayVal = isInteracting ? internalValue : value;
  const percentage = Math.max(0, Math.min(100, ((displayVal - min) / (max - min || 1)) * 100));

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    const nextVal = Number(e.target.value);
    internalValueRef.current = nextVal;
    setInternalValue(nextVal);
    onChange?.(nextVal);
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
      onCommit?.(internalValueRef.current);
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
      onCommit?.(internalValueRef.current);
    }
  };

  return (
    <div
      className={`cavia-slider-wrap ${disabled ? 'disabled' : ''} ${isInteracting ? 'is-dragging' : ''} ${className}`}
      style={{ '--slider-fill': `${percentage}%` } as CSSProperties}
    >
      {showFill && <span className="cavia-slider-fill" aria-hidden="true" />}
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
        aria-valuetext={ariaValueText}
        aria-disabled={disabled}
        className="cavia-range-input"
      />
    </div>
  );
}

