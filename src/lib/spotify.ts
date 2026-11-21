import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type ImportStatus = 'queued' | 'running' | 'paused' | 'completed' | 'cancelled' | 'failed';

export interface ImportJob {
  id: string;
  playlistId: string;
  playlistName: string;
  total: number;
  processed: number;
  matchedLocal: number;
  matchedOnline: number;
  skipped: number;
  currentTrack?: string | null;
  status: ImportStatus;
  error?: string | null;
  createdAtMs: number;
}

export interface SpotifyImportResult {
  playlistId: string;
  playlistName: string;
  total: number;
  matchedLocal: number;
  matchedOnline: number;
  skipped: string[];
}

export const isSpotifyImportAvailable = (): boolean => isTauri();

/**
 * Starts a fast asynchronous Spotify playlist import. The playlist and all track
 * shells are created in SQLite in under a second; matching against the local library
 * and YouTube Music runs in the background.
 */
export async function startSpotifyImport(url: string): Promise<ImportJob> {
  if (!isTauri()) {
    throw new Error('Spotify import runs in the Cavia desktop app only.');
  }
  return invoke<ImportJob>('spotify_start_import', { url: url.trim() });
}

export async function pauseImport(jobId: string): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>('import_pause', { jobId });
}

export async function resumeImport(jobId: string): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>('import_resume', { jobId });
}

export async function cancelImport(jobId: string): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>('import_cancel', { jobId });
}

export async function getImportJobs(): Promise<ImportJob[]> {
  if (!isTauri()) return [];
  return invoke<ImportJob[]>('import_get_jobs');
}

export async function clearCompletedImports(): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>('import_clear_completed');
}

export async function removeImportJob(jobId: string): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>('import_remove_job', { jobId });
}

export async function cancelAndDeleteImport(jobId: string): Promise<string> {
  if (!isTauri()) return '';
  return invoke<string>('import_cancel_and_delete', { jobId });
}

export function listenImportProgress(callback: (job: ImportJob) => void): Promise<UnlistenFn> {
  return listen<ImportJob>('import://progress', (event) => callback(event.payload));
}

export function listenImportCompleted(callback: (job: ImportJob) => void): Promise<UnlistenFn> {
  return listen<ImportJob>('import://completed', (event) => callback(event.payload));
}

/** Legacy synchronous import command wrapper */
export async function importSpotifyPlaylist(url: string): Promise<SpotifyImportResult> {
  if (!isTauri()) {
    throw new Error('Spotify import runs in the Cavia desktop app only.');
  }
  return invoke<SpotifyImportResult>('spotify_import_playlist', { url: url.trim() });
}
