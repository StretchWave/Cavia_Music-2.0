export function formatBytes(bytes) {
  if (bytes <= 0 || Number.isNaN(bytes)) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export function isValidArtworkFilename(name) {
  if (!name || name.startsWith('.')) return false;
  const lastDot = name.lastIndexOf('.');
  if (lastDot <= 0) return false;
  const ext = name.slice(lastDot + 1).toLowerCase();
  const hash = name.slice(0, lastDot);
  if (hash.length !== 64 || !/^[0-9a-fA-F]+$/.test(hash)) return false;
  return ['jpg', 'jpeg', 'png', 'webp', 'gif'].includes(ext);
}

export function getPersistenceBadgeConfig(status) {
  switch (status) {
    case 'healthy':
      return { label: 'PERSISTENCE READY', variant: 'ready' };
    case 'write_pending':
      return { label: 'SAVING PLAYBACK STATE', variant: 'warn' };
    case 'write_failed':
      return { label: 'SAVE FAILED', variant: 'vermilion' };
    case 'worker_unavailable':
      return { label: 'PERSISTENCE UNAVAILABLE', variant: 'vermilion' };
    case 'shutting_down':
      return { label: 'SAVING BEFORE EXIT', variant: 'warn' };
    default:
      return { label: 'PERSISTENCE READY', variant: 'ready' };
  }
}

export function formatSmtcDiagnostic(status, error, systemMediaControls) {
  if (status === 'ready' || (!status && systemMediaControls)) {
    return 'SMTC ACTIVE';
  }
  if (status === 'error') {
    return `SMTC ERROR: ${error || 'Runtime fault'}`;
  }
  if (status === 'unavailable') {
    return error ? `SMTC UNAVAILABLE: ${error}` : 'SMTC UNAVAILABLE (No active window handle)';
  }
  if (status === 'unsupported') {
    return error ? `SMTC UNSUPPORTED: ${error}` : 'SMTC UNSUPPORTED';
  }
  return error ? `SMTC UNAVAILABLE: ${error}` : 'SMTC UNAVAILABLE';
}

export function extractShutdownErrorMessage(payload) {
  if (payload && typeof payload === 'object' && typeof payload.error === 'string') {
    return payload.error;
  }
  return null;
}

export function detectPlaylistImportSource(url) {
  const trimmed = typeof url === 'string' ? url.trim() : '';
  if (!trimmed) return null;
  if (trimmed.startsWith('spotify:playlist:') || /open\.spotify\.com\/.*playlist\//i.test(trimmed)) {
    return 'spotify';
  }
  try {
    const parsed = new URL(trimmed);
    const host = parsed.hostname.toLowerCase();
    const isYt =
      host === 'music.youtube.com' ||
      host === 'youtube.com' ||
      host === 'www.youtube.com' ||
      host === 'm.youtube.com' ||
      host === 'youtu.be';
    if (isYt && (parsed.searchParams.has('list') || parsed.pathname.startsWith('/browse/'))) {
      return 'youtube_music';
    }
  } catch {
    if (/^(PL|OLAK5uy_|RDCLAK|RDAM|RDEM|UU|FL|LP|VLPL|VLOLAK)[A-Za-z0-9_-]{8,}$/.test(trimmed)) {
      return 'youtube_music';
    }
    if (/^[A-Za-z0-9]{22}$/.test(trimmed)) {
      return 'spotify';
    }
  }
  return null;
}
