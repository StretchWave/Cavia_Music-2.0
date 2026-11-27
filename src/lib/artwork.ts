import { invoke, isTauri } from '@tauri-apps/api/core';
import { formatBytes } from './diagnostics.js';

export { formatBytes };

const requireNative = () => {
  if (!isTauri()) {
    throw new Error('Artwork cache tools are available in the Cavia desktop app only.');
  }
};

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
  requireNative();
  return invoke<ArtworkCacheSummary>('get_artwork_cache_summary');
}

export async function cleanOrphanedArtwork(): Promise<ArtworkCleanupResult> {
  requireNative();
  return invoke<ArtworkCleanupResult>('clean_orphaned_artwork');
}

export async function recoverMissingArtwork(): Promise<ArtworkRecoveryResult> {
  requireNative();
  return invoke<ArtworkRecoveryResult>('recover_missing_artwork');
}

export async function recoverTrackArtwork(trackId: string): Promise<TrackRecoveryOutcome> {
  requireNative();
  return invoke<TrackRecoveryOutcome>('recover_track_artwork', { trackId });
}
