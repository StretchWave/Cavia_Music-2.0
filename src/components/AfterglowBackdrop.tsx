import { useEffect, useState } from 'react';

interface AfterglowBackdropProps {
  /** Already resolved (asset-converted) artwork URL for the current track. */
  artworkUri?: string;
  isPlaying: boolean;
  /** Off disables the ambient layer entirely (Motion: Off). */
  enabled?: boolean;
}

interface Layers {
  current?: string;
  previous?: string;
}

/**
 * The listening room: a heavily blurred artwork plate behind the whole shell.
 *
 * It is one pre-blurred bitmap per track, cross-fading over 600ms, and it only
 * drifts while audio is actually playing. No artwork means no light source —
 * the ink canvas stays dark rather than inventing an image.
 */
export function AfterglowBackdrop({ artworkUri, isPlaying, enabled = true }: AfterglowBackdropProps) {
  const [layers, setLayers] = useState<Layers>({ current: artworkUri });

  useEffect(() => {
    setLayers((previous) => {
      if (previous.current === artworkUri) return previous;
      return { current: artworkUri, previous: previous.current };
    });
  }, [artworkUri]);

  // Drop the outgoing plate once the cross-fade has finished.
  useEffect(() => {
    if (!layers.previous) return;
    const timer = window.setTimeout(() => {
      setLayers((previous) => ({ current: previous.current }));
    }, 700);
    return () => window.clearTimeout(timer);
  }, [layers.previous]);

  if (!enabled || !layers.current) return null;

  return (
    <div className="afterglow" aria-hidden="true" data-playing={isPlaying ? 'true' : 'false'}>
      {layers.previous && (
        <div
          className="afterglow-layer afterglow-layer-outgoing"
          style={{ backgroundImage: `url("${layers.previous}")` }}
        />
      )}
      <div
        className="afterglow-layer afterglow-layer-current"
        style={{ backgroundImage: `url("${layers.current}")` }}
      />
      <div className="afterglow-veil" />
    </div>
  );
}
