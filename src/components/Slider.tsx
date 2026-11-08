import { type ChangeEvent } from 'react';

interface SliderProps {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (value: number) => void;
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
  disabled = false,
  label,
  className = '',
  showFill = true,
}: SliderProps) {
  const percentage = Math.max(0, Math.min(100, ((value - min) / (max - min || 1)) * 100));

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    onChange(Number(e.target.value));
  };

  return (
    <div className={`cavia-slider-wrap ${disabled ? 'disabled' : ''} ${className}`}>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={handleChange}
        disabled={disabled}
        aria-label={label}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        className="cavia-range-input"
        style={{
          background: showFill
            ? `linear-gradient(to right, var(--accent-vermilion) ${percentage}%, var(--bg-hover) ${percentage}%)`
            : undefined,
        }}
      />
    </div>
  );
}

