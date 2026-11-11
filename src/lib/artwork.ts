import { invoke } from '@tauri-apps/api/core';

export interface ArtworkCacheSummary {
  totalBytes: number;
  totalFiles: number;
  referencedFiles: number;
  orphanedFiles: number;
  reclaimableBytes: number;
  missingReferencedFiles: number;
  cacheDirExists: boolean;
  cacheDirPath: string;
  inspectionErrors: string[];
  formatCounts: Record<string, number>;
}

export interface ArtworkCleanupResult {
  deletedFiles: number;
  reclaimedBytes: number;
  skippedFiles: number;
  failures: string[];
}

export interface ArtworkRecoveryResult {
  totalChecked: number;
  recoveredTracks: number;
  alreadyValidTracks: number;
  missingAudioFiles: number;
  noEmbeddedArtwork: number;
  failedTracks: number;
  reclaimedOrWrittenBytes: number;
  errors: string[];
}

export interface TrackRecoveryOutcome {
  trackId: string;
  success: boolean;
  status: string;
  newArtworkUri?: string;
  errorMessage?: string;
}

export async function getArtworkCacheSummary(): Promise<ArtworkCacheSummary> {
  return invoke<ArtworkCacheSummary>('get_artwork_cache_summary');
}

export async function cleanOrphanedArtwork(): Promise<ArtworkCleanupResult> {
  return invoke<ArtworkCleanupResult>('clean_orphaned_artwork');
}

export async function recoverMissingArtwork(): Promise<ArtworkRecoveryResult> {
  return invoke<ArtworkRecoveryResult>('recover_missing_artwork');
}

export async function recoverTrackArtwork(trackId: string): Promise<TrackRecoveryOutcome> {
  return invoke<TrackRecoveryOutcome>('recover_track_artwork', { trackId });
}

export function formatBytes(bytes: number): string {
  if (bytes <= 0 || isNaN(bytes)) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${sizes[i]}`;
}

