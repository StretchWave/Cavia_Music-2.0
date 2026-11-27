export function formatBytes(bytes: number): string;
export function isValidArtworkFilename(name: string): boolean;
export function getPersistenceBadgeConfig(status?: string): {
  label: string;
  variant: 'ready' | 'warn' | 'vermilion';
};
export function formatSmtcDiagnostic(
  status?: string,
  error?: string | null,
  systemMediaControls?: boolean
): string;
export function extractShutdownErrorMessage(payload: unknown): string | null;
export function detectPlaylistImportSource(url: string): 'spotify' | 'youtube_music' | null;
