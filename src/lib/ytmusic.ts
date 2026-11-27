import { invoke, isTauri } from '@tauri-apps/api/core';

export interface YtMusicTrack {
  videoId: string;
  title: string;
  artist: string;
  album: string;
  durationMs: number;
  artworkUrl?: string;
}

export const isOnlineSearchAvailable = (): boolean => isTauri();

function getSearchLocale(): { region: string; language: string } {
  try {
    const locale = new Intl.Locale(navigator.language || 'en-US');
    return {
      region: locale.region?.slice(0, 2).toUpperCase() || 'US',
      language: locale.language.slice(0, 3).toLowerCase() || 'en',
    };
  } catch {
    return { region: 'US', language: 'en' };
  }
}

export async function searchYouTubeMusic(query: string, requestId: string): Promise<YtMusicTrack[]> {
  if (!isTauri()) return [];
  const trimmed = query.trim();
  if (!trimmed) return [];
  return invoke<YtMusicTrack[]>('ytmusic_search', {
    query: trimmed,
    requestId,
    ...getSearchLocale(),
  });
}

export async function cancelYouTubeMusicSearch(requestId: string): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>('ytmusic_cancel_search', { requestId });
}

export async function fetchRelatedYouTubeMusic(videoId: string): Promise<YtMusicTrack[]> {
  if (!isTauri()) return [];
  return invoke<YtMusicTrack[]>('ytmusic_related', { videoId });
}

export async function playYouTubeMusic(track: YtMusicTrack): Promise<void> {
  return invoke<void>('ytmusic_play', {
    videoId: track.videoId,
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
    artworkUrl: track.artworkUrl ?? null,
  });
}

export function formatOnlineDuration(durationMs: number): string {
  if (!durationMs || durationMs <= 0) return '--:--';
  const totalSeconds = Math.floor(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}
