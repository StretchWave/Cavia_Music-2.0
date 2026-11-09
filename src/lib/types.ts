export type Page =
  | 'library'
  | 'tracks'
  | 'playlists'
  | 'albums'
  | 'artists'
  | 'liked'
  | 'search'
  | 'settings';

export type RepeatMode = 'off' | 'all' | 'one';

export interface Track {
  id: string;
  title: string;
  artist: string;
  album: string;
  albumArtist?: string;
  durationMs: number;
  artworkUri?: string;
  sourceKind: 'local' | 'provider';
  providerId?: string;
  fileUri?: string;
  releaseYear?: number;
  trackNumber?: number;
  discNumber?: number;
  genre?: string;
  isLiked?: boolean;
  playCount?: number;
  dateAdded?: string;
}

export interface Playlist {
  id: string;
  name: string;
  description: string;
  artworkUri?: string;
  sourceProvider?: string;
  trackCount: number;
  createdAt: string;
  tracks?: Track[];
}

export interface Album {
  id: string;
  title: string;
  artist: string;
  year?: number;
  trackCount: number;
  artworkUri?: string;
}

export interface Artist {
  id: string;
  name: string;
  trackCount: number;
  albumCount: number;
}

export interface PlaybackState {
  generation?: number;
  status: 'idle' | 'loading' | 'playing' | 'paused' | 'stopped' | 'error';
  currentTrack: Track | null;
  positionMs: number;
  durationMs: number;
  volume: number; // 0 - 100
  isMuted: boolean;
  repeatMode: RepeatMode;
  isShuffle: boolean;
  playbackSpeed: number;
  queue: Track[];
  errorMessage?: string;
}

export interface ProviderInfo {
  id: string;
  name: string;
  category: 'metadata' | 'lyrics' | 'streaming' | 'local';
  description: string;
  status: 'operational' | 'unconfigured' | 'pending_auth' | 'planned';
  capabilities: {
    catalogueSearch: boolean;
    metadataLookup: boolean;
    playback: boolean;
    caching: boolean;
    lyrics: boolean;
  };
  notes: string;
}

