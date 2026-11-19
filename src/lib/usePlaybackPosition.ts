import { useEffect, useState } from 'react';
import {
  getCurrentPlaybackPosition,
  subscribePlaybackPosition,
  type PositionUpdatePayload,
} from './playback';

/**
 * High-frequency position subscriber hook.
 *
 * Decouples playback progress updates (emitted ~4 times per second) from the root
 * application state. This isolates component re-renders strictly to player UI
 * (scrubbers, timecodes, lyrics) without triggering cascading re-renders across
 * large track lists, album grids, or search results.
 */
export function usePlaybackPosition(
  fallbackPositionMs = 0,
  fallbackDurationMs = 0
): PositionUpdatePayload {
  const [position, setPosition] = useState<PositionUpdatePayload>(() => {
    const current = getCurrentPlaybackPosition();
    return {
      positionMs: current.positionMs || fallbackPositionMs,
      durationMs: current.durationMs || fallbackDurationMs,
      trackId: current.trackId,
      generation: current.generation,
    };
  });

  useEffect(() => {
    return subscribePlaybackPosition((pos) => {
      setPosition(pos);
    });
  }, []);

  return position;
}
