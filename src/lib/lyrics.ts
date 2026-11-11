import { invoke } from '@tauri-apps/api/core';
import { isNativeApp } from './platform';

export interface LyricsLine {
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
 * Fetches lyrics for the given track ID through the Rust native backend.
 * Checks SQLite cache, embedded metadata tags (ID3 USLT/SYLT, Vorbis, MP4),
 * and LRCLIB in order.
 */
export async function getTrackLyrics(trackId: string): Promise<LyricsResult> {
  if (!isNativeApp()) {
    throw new Error('Native lyrics service is unavailable in browser environment');
  }
  return invoke<LyricsResult>('get_track_lyrics', { trackId });
}

/**
 * Forces a refresh of lyrics for the given track ID, bypassing existing cache.
 */
export async function refreshTrackLyrics(trackId: string): Promise<LyricsResult> {
  if (!isNativeApp()) {
    throw new Error('Native lyrics service is unavailable in browser environment');
  }
  return invoke<LyricsResult>('refresh_track_lyrics', { trackId });
}

