import { useEffect, useState } from 'react';
import type { Track } from './types';
import { getTrackLyrics, type LyricsLine, type LyricsResult } from './lyrics';
import { isNativeApp } from './platform';

/**
 * Loads lyrics for the playing track without blocking the player.
 *
 * The drawer owns the interactive experience; this hook exists so the expanded
 * player can show the line that is playing right now. The Rust service caches
 * results, so a second call for the same track is cheap.
 */
export function useTrackLyrics(track: Track | null, enabled: boolean = true): LyricsResult | null {
  const [lyrics, setLyrics] = useState<LyricsResult | null>(null);
  const trackId = track?.id ?? null;
  const trackTitle = track?.title ?? '';
  const trackArtist = track?.artist ?? '';
  const trackAlbum = track?.album ?? '';
  const trackDuration = track?.durationMs ?? null;

  useEffect(() => {
    setLyrics(null);
    if (!enabled || !trackId || !isNativeApp()) return;

    let isSubscribed = true;
    getTrackLyrics(trackId, {
      title: trackTitle,
      artist: trackArtist,
      album: trackAlbum,
      durationMs: trackDuration,
    })
      .then((result) => {
        if (isSubscribed) setLyrics(result);
      })
      .catch(() => {
        if (isSubscribed) setLyrics(null);
      });

    return () => {
      isSubscribed = false;
    };
  }, [enabled, trackId, trackTitle, trackArtist, trackAlbum, trackDuration]);

  return lyrics;
}

/**
 * The line that contains `positionMs`, or null when the track has no synced
 * lyrics. An empty string means the active timestamp has no words (a break or
 * instrumental passage), which callers can render as a rest marker.
 */
export function getActiveLyricLine(
  lyrics: LyricsResult | null,
  positionMs: number
): string | null {
  const lines: LyricsLine[] | null | undefined = lyrics?.syncedLines;
  if (!lines || lines.length === 0 || lyrics?.status !== 'synced') return null;

  // Binary search for the active line at positionMs
  let low = 0;
  let high = lines.length - 1;
  let candidate = -1;
  while (low <= high) {
    const mid = (low + high) >> 1;
    if (lines[mid].timestampMs <= positionMs) {
      candidate = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }

  return candidate >= 0 ? lines[candidate].text : null;
}
