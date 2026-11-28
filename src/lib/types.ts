export type Page = 'library' | 'search' | 'imports' | 'settings';

export type RepeatMode = 'off' | 'all' | 'one';

export interface Track {
  id: string;
  title: string;
  artist: string;
  album: string;
  albumArtist?: string;
  durationMs: number;
  artworkUri?: string;
  sourceKind: 'local' | 'provider' | 'remote_sync';
  providerId?: string;
  fileUri?: string;
  releaseYear?: number;
  trackNumber?: number;
  discNumber?: number;
  genre?: string;
  isLiked?: boolean;
  playCount?: number;
  dateAdded?: string;
  queueEntryId?: string;
  contributorId?: string;
  contributorName?: string;
  priorityOverride?: boolean;
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
  artworkUri?: string;
}

export type SearchCategory = 'all' | 'tracks' | 'albums' | 'artists' | 'playlists';

export interface SearchResultsDTO {
  tracks: Track[];
  albums: Album[];
  artists: Artist[];
  playlists: Playlist[];
  query: string;
  category: SearchCategory;
  totalCount: number;
}

export interface PlaybackState {
  generation?: number;
  status: 'idle' | 'loading' | 'playing' | 'paused' | 'stopped' | 'error';
  currentTrack: Track | null;
  currentIndex?: number | null;
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

export interface SyncAccountStatus {
  authenticated: boolean;
  accountId?: string;
  accountEmail?: string;
  deviceId: string;
  syncEnabled: boolean;
  lastSyncedAt?: string;
  pendingOutboxCount: number;
  lastError?: string;
  backendConfigured: boolean;
}

export interface SyncResult {
  pushedCount: number;
  pulledCount: number;
  success: boolean;
  errorMessage?: string;
}

export interface SyncConfigDTO {
  supabaseUrl: string;
  supabaseAnonKey: string;
  isConfigured: boolean;
}

export type PreloadItemStatus =
  | 'ready'
  | 'downloading'
  | 'queued'
  | 'pending_preload'
  | 'failed'
  | 'offline_paused';

export interface PreloadItemView {
  queueEntryId: string;
  trackId: string;
  title: string;
  artist: string;
  contributorName?: string;
  sourceKind: string;
  status: PreloadItemStatus;
  errorMessage?: string;
}

export interface PreloadStatusSnapshot {
  enabled: boolean;
  targetCount: number;
  readyCount: number;
  queuedWindowCount: number;
  latePendingCount: number;
  isDownloading: boolean;
  pausedForPlayback: boolean;
  offlinePaused: boolean;
  activeTitle?: string | null;
  cacheUsedBytes: number;
  cacheLimitBytes: number;
  items: PreloadItemView[];
  lateItems: PreloadItemView[];
}

export interface OfflineFallbackStatus {
  isOffline: boolean;
  outageId: number;
  promptVisible: boolean;
  promptMessage?: string | null;
  fallbackActive: boolean;
  fallbackExhausted: boolean;
  eligibleCachedCount: number;
  playedFallbackCount: number;
  remainingFallbackCount: number;
  reconnectPendingFinish: boolean;
  statusMessage?: string | null;
}

export type ParticipantRole = 'host' | 'guest';

export type QueueEntryStatus =
  | 'pending_preload'
  | 'ready'
  | 'playing'
  | 'played'
  | 'removed';

export interface SharedQueueEntry {
  queueEntryId: string;
  track: Track;
  canonicalProviderId: string;
  contributorId: string;
  contributorName: string;
  role: ParticipantRole;
  status: QueueEntryStatus;
  priorityOverride: boolean;
  manualPosition?: number | null;
  committed: boolean;
  committedRank?: number | null;
  createdAtSeq: number;
}

export interface ParticipantWeightSummary {
  participantId: string;
  displayName: string;
  role: ParticipantRole;
  isConnected: boolean;
  pendingSongs: number;
  penaltyCredits: number;
  weight: number;
  totalAdded: number;
  totalPlayed: number;
}

export interface ConnectSessionSnapshot {
  active: boolean;
  sessionId: string;
  joinCode: string;
  name: string;
  localParticipantId: string;
  localRole: ParticipantRole;
  hostParticipantId: string;
  hostDisplayName: string;
  fairnessAlpha: number;
  preloadTarget: number;
  queueRevision: number;
  currentEntry?: SharedQueueEntry | null;
  sharedPool: SharedQueueEntry[];
  upcomingSequence: SharedQueueEntry[];
  preloadWindow: SharedQueueEntry[];
  pendingPreload: SharedQueueEntry[];
  weights: ParticipantWeightSummary[];
  unsyncedEventCount: number;
  lastSyncError?: string | null;
}

export interface ConnectContributionResult {
  entry: SharedQueueEntry;
  snapshot: ConnectSessionSnapshot;
}

export interface ConnectPlaylistContributionResult {
  acceptedCount: number;
  rejectedDuplicates: number;
  rejectedUnsupported: number;
  pendingPreloadCount: number;
  snapshot: ConnectSessionSnapshot;
}

export interface ConnectSyncResult {
  pushedEvents: number;
  pulledEvents: number;
  skippedDuplicates: number;
  revision: number;
  offline: boolean;
  errorMessage?: string | null;
}

export interface ConnectAccountProfile {
  authenticated: boolean;
  backendConfigured: boolean;
  accountId?: string | null;
  connectId?: string | null;
  displayName: string;
  avatarUrl?: string | null;
  hosting: boolean;
  acceptJoinRequests: boolean;
  showConnectId: boolean;
  activeSessionId?: string | null;
  offline: boolean;
  errorMessage?: string | null;
}

export interface HostLookupResult {
  hostAccountId: string;
  connectId: string;
  displayName: string;
  avatarUrl?: string | null;
  hosting: boolean;
  acceptJoinRequests: boolean;
  canRequest: boolean;
  availabilityStatus: string;
  alreadyApproved: boolean;
  pendingRequest: boolean;
}

export interface ConnectRequestSendOutcome {
  outcome: string;
  requestId?: string | null;
  sessionId?: string | null;
  message: string;
}

export interface ConnectionRequestItem {
  requestId: string;
  sessionId: string;
  requesterAccountId: string;
  requesterConnectId: string;
  requesterDisplayName: string;
  requesterAvatarUrl?: string | null;
  status: 'pending' | 'accepted' | 'declined' | 'ignored' | 'closed' | string;
  createdAt: string;
  respondedAt?: string | null;
}

export interface OutboundRequestItem {
  requestId: string;
  sessionId: string;
  hostAccountId: string;
  hostConnectId: string;
  hostDisplayName: string;
  hostAvatarUrl?: string | null;
  status: 'pending' | 'accepted' | 'declined' | 'unavailable' | 'closed' | string;
  createdAt: string;
  respondedAt?: string | null;
}

export interface IgnoredAccountEntry {
  ignoredAccountId: string;
  connectId: string;
  displayName: string;
  avatarUrl?: string | null;
  createdAt: string;
  expiresAt: string;
}

export interface SavedHostEntry {
  hostAccountId: string;
  hostConnectId: string;
  hostDisplayName: string;
  hostAvatarUrl?: string | null;
  hosting: boolean;
  acceptJoinRequests: boolean;
  availabilityStatus: string;
  savedAt: string;
}

export interface ConnectRespondOutcome {
  requestId: string;
  status: string;
  requesterAccountId: string;
  requesterDisplayName: string;
  sessionId: string;
}

export interface ConnectAccountHubSnapshot {
  profile: ConnectAccountProfile;
  incomingRequests: ConnectionRequestItem[];
  outboundRequests: OutboundRequestItem[];
  ignoredAccounts: IgnoredAccountEntry[];
  savedHosts: SavedHostEntry[];
}

