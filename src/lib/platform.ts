import { invoke, isTauri } from '@tauri-apps/api/core';

export type PlatformHealth = {
  app_version: string;
  database_connected: boolean;
  audio_backend: 'ready' | 'device_unavailable' | 'not_integrated' | 'error' | 'unsupported' | 'initializing';
  backend_name?: string;
  active_migration?: string;
  system_media_controls: boolean;
  platform: string;
};

export type LibrarySummary = {
  tracks: number;
  playlists: number;
  liked_tracks: number;
};

export const isNativeApp = (): boolean => isTauri();

export async function getPlatformHealth(): Promise<PlatformHealth> {
  return invoke<PlatformHealth>('app_health');
}

export async function getLibrarySummary(): Promise<LibrarySummary> {
  return invoke<LibrarySummary>('library_summary');
}

export function explainNativeOnly(): string {
  return 'This action is available in the installed Cavia desktop or mobile application.';
}
