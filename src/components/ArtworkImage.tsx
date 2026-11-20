import { useState, useEffect, type ReactNode } from 'react';
import { Disc3 } from 'lucide-react';

interface ArtworkImageProps {
  src?: string;
  alt: string;
  className?: string;
  fallbackIcon?: ReactNode;
  fallbackText?: string;
  kanji?: string;
  showPlaceholderSpin?: boolean;
}

export function ArtworkImage({
  src,
  alt,
  className = '',
  fallbackIcon,
  fallbackText,
  kanji,
  showPlaceholderSpin = false,
}: ArtworkImageProps) {
  const [hasError, setHasError] = useState(false);

  useEffect(() => {
    setHasError(false);
  }, [src]);

  if (!src || hasError) {
    if (kanji) {
      return (
        <div className={`sleeve-art-placeholder ${className}`} aria-label={alt}>
          <span className="sleeve-kanji" aria-hidden="true">
            {kanji}
          </span>
          {fallbackText && <span className="sleeve-label">{fallbackText}</span>}
        </div>
      );
    }

    return (
      <div className={`artwork-fallback-frame ${className}`} aria-label={alt}>
        {fallbackIcon ?? (
          <Disc3 size={18} className={showPlaceholderSpin ? 'spinning' : ''} aria-hidden="true" />
        )}
      </div>
    );
  }

  return (
    <img
      src={src}
      alt={alt}
      className={className}
      onError={() => setHasError(true)}
      loading="lazy"
      decoding="async"
    />
  );
}
