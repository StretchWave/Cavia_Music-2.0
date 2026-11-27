import { invoke } from '@tauri-apps/api/core';
import { isNativeApp } from './platform';

export interface LyricsLine {
  timestampMs: number;
  text: string;
  /** Present only when the lyric source supplied explicit inline word times. */
  wordTimings?: LyricsWordTiming[] | null;
}

export interface LyricsWordTiming {
  timestampMs: number;
  text: string;
}

export type LyricsStatus = 'synced' | 'plain' | 'instrumental' | 'not_found';
export type LyricsSource = 'embedded' | 'lrclib';

export interface LyricsResult {
  trackId: string;
  status: LyricsStatus;
  source: LyricsSource | null;
  plainLyrics: string | null;
  syncedLines: LyricsLine[] | null;
  isInstrumental: boolean;
}

/**
 * Metadata for tracks that are not rows in the local library (YouTube Music
 * streams). The Rust resolver needs it because it cannot look the track up in
 * `tracks`, and it never guesses a different recording from an empty query.
 */
export interface LyricsTrackMeta {
  title: string;
  artist: string;
  album?: string | null;
  durationMs?: number | null;
}

/**
 * Fetches lyrics for the given track ID through the Rust native backend.
 * Checks SQLite cache, embedded metadata tags (ID3 USLT/SYLT, Vorbis, MP4),
 * and LRCLIB in order.
 */
export async function getTrackLyrics(
  trackId: string,
  meta?: LyricsTrackMeta | null
): Promise<LyricsResult> {
  if (!isNativeApp()) {
    throw new Error('Native lyrics service is unavailable in browser environment');
  }
  return invoke<LyricsResult>('get_track_lyrics', {
    trackId,
    title: meta?.title ?? null,
    artist: meta?.artist ?? null,
    album: meta?.album ?? null,
    durationMs: meta?.durationMs ?? null,
  });
}

/**
 * Forces a refresh of lyrics for the given track ID, bypassing existing cache.
 */
export async function refreshTrackLyrics(
  trackId: string,
  meta?: LyricsTrackMeta | null
): Promise<LyricsResult> {
  if (!isNativeApp()) {
    throw new Error('Native lyrics service is unavailable in browser environment');
  }
  return invoke<LyricsResult>('refresh_track_lyrics', {
    trackId,
    title: meta?.title ?? null,
    artist: meta?.artist ?? null,
    album: meta?.album ?? null,
    durationMs: meta?.durationMs ?? null,
  });
}

