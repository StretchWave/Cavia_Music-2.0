import { invoke, isTauri, convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { Album, Artist, Playlist, Track } from './types';

export interface ScanProgressEvent {
  phase: 'discovering' | 'scanning' | 'completed' | 'cancelled' | 'error';
  currentFile: string;
  discoveredCount: number;
  processedCount: number;
  newTracksCount: number;
  duplicatesCount: number;
  errorsCount: number;
  errorMessage?: string;
}

export interface ScanResult {
  totalDiscovered: number;
  newTracks: number;
  duplicates: number;
  errors: number;
  cancelled: boolean;
}

export const isNativeApp = (): boolean => isTauri();

export async function pickMusicFolder(): Promise<string | null> {
  if (!isNativeApp()) return null;
  const result = await open({
    directory: true,
    multiple: false,
    title: 'Select Music Directory to Import',
  });
  if (!result) return null;
  return Array.isArray(result) ? result[0] : result;
}

export async function scanMusicDirectory(folderPath: string): Promise<ScanResult> {
  return invoke<ScanResult>('scan_music_directory', { folderPath });
}

export async function cancelScan(): Promise<void> {
  return invoke<void>('cancel_scan');
}

export async function getTracks(): Promise<Track[]> {
  if (!isNativeApp()) return [];
  return invoke<Track[]>('get_tracks');
}

export async function getAlbums(): Promise<Album[]> {
  if (!isNativeApp()) return [];
  return invoke<Album[]>('get_albums');
}

export async function getArtists(): Promise<Artist[]> {
  if (!isNativeApp()) return [];
  return invoke<Artist[]>('get_artists');
}

export async function getLikedTracks(): Promise<Track[]> {
  if (!isNativeApp()) return [];
  return invoke<Track[]>('get_liked_tracks');
}

export async function toggleLikedTrack(trackId: string): Promise<boolean> {
  return invoke<boolean>('toggle_liked_track', { trackId });
}

export async function getPlaylists(): Promise<Playlist[]> {
  if (!isNativeApp()) return [];
  return invoke<Playlist[]>('get_playlists');
}

export async function getPlaylistTracks(playlistId: string): Promise<Track[]> {
  if (!isNativeApp()) return [];
  return invoke<Track[]>('get_playlist_tracks', { playlistId });
}

export async function createPlaylist(name: string, description: string): Promise<Playlist> {
  return invoke<Playlist>('create_playlist', { name, description });
}

export async function deletePlaylist(playlistId: string): Promise<void> {
  return invoke<void>('delete_playlist', { playlistId });
}

export function formatArtworkUri(uri?: string): string | undefined {
  if (!uri) return undefined;
  if (uri.startsWith('http://') || uri.startsWith('https://') || uri.startsWith('data:')) {
    return uri;
  }
  if (isNativeApp()) {
    try {
      return convertFileSrc(uri);
    } catch {
      return uri;
    }
  }
  return uri;
}

