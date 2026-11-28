import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isNativeApp } from './platform';
import type {
  ConnectAccountHubSnapshot,
  ConnectContributionResult,
  ConnectPlaylistContributionResult,
  ConnectRequestSendOutcome,
  ConnectRespondOutcome,
  ConnectSessionSnapshot,
  ConnectSyncResult,
  HostLookupResult,
  OfflineFallbackStatus,
  ParticipantRole,
  PreloadStatusSnapshot,
  Track,
} from './types';

export const MAX_PRELOAD_TARGET = 20;

export async function preloadGetStatus(): Promise<PreloadStatusSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<PreloadStatusSnapshot>('preload_get_status');
}

export async function preloadSetTarget(target: number): Promise<PreloadStatusSnapshot | null> {
  if (!isNativeApp()) return null;
  const clamped = Math.max(0, Math.min(MAX_PRELOAD_TARGET, Math.round(target)));
  return invoke<PreloadStatusSnapshot>('preload_set_target', { target: clamped });
}

export async function preloadRetry(): Promise<PreloadStatusSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<PreloadStatusSnapshot>('preload_retry');
}

export async function listenPreloadStatus(
  callback: (snapshot: PreloadStatusSnapshot) => void
): Promise<UnlistenFn> {
  if (!isNativeApp()) return () => { };
  return listen<PreloadStatusSnapshot>('playback:preload-changed', (event) => {
    callback(event.payload);
  });
}

export async function offlineGetStatus(): Promise<OfflineFallbackStatus | null> {
  if (!isNativeApp()) return null;
  return invoke<OfflineFallbackStatus>('offline_get_status');
}

export async function offlineChooseShuffleCached(): Promise<OfflineFallbackStatus | null> {
  if (!isNativeApp()) return null;
  return invoke<OfflineFallbackStatus>('offline_choose_shuffle_cached');
}

export async function offlineDeclineFallback(): Promise<OfflineFallbackStatus | null> {
  if (!isNativeApp()) return null;
  return invoke<OfflineFallbackStatus>('offline_decline_fallback');
}

export async function offlineExitFallback(): Promise<OfflineFallbackStatus | null> {
  if (!isNativeApp()) return null;
  return invoke<OfflineFallbackStatus>('offline_exit_fallback');
}

export async function offlineProbeReconnect(): Promise<OfflineFallbackStatus | null> {
  if (!isNativeApp()) return null;
  return invoke<OfflineFallbackStatus>('offline_probe_reconnect');
}

export async function listenOfflineStatus(
  callback: (status: OfflineFallbackStatus) => void
): Promise<UnlistenFn> {
  if (!isNativeApp()) return () => { };
  return listen<OfflineFallbackStatus>('playback:offline-changed', (event) => {
    callback(event.payload);
  });
}

export async function connectGetSession(): Promise<ConnectSessionSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSessionSnapshot | null>('connect_get_session');
}

export async function connectCreateSession(
  name: string,
  hostName: string,
  alpha = 1.0
): Promise<ConnectSessionSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSessionSnapshot>('connect_create_session', {
    name,
    hostName,
    alpha,
  });
}

export async function connectJoinSession(
  joinCode: string,
  displayName: string,
  participantId?: string
): Promise<ConnectSessionSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSessionSnapshot>('connect_join_session', {
    joinCode,
    displayName,
    participantId,
  });
}

export async function connectJoinApprovedSession(
  sessionId: string
): Promise<ConnectSessionSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSessionSnapshot>('connect_join_approved_session', { sessionId });
}

export async function connectSetParticipantConnected(
  participantId: string,
  connected: boolean
): Promise<ConnectSessionSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSessionSnapshot>('connect_set_participant_connected', {
    participantId,
    connected,
  });
}

export async function connectContributeTrack(params: {
  track: Track;
  contributorId?: string;
  contributorName?: string;
  role?: ParticipantRole;
  priorityOverride?: boolean;
}): Promise<ConnectContributionResult | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectContributionResult>('connect_contribute_track', params);
}

export async function connectContributeYtmusicPlaylist(params: {
  playlistUrlOrId: string;
  contributorId?: string;
  contributorName?: string;
  role?: ParticipantRole;
}): Promise<ConnectPlaylistContributionResult | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectPlaylistContributionResult>('connect_contribute_ytmusic_playlist', params);
}

export async function connectRemoveEntry(
  queueEntryId: string,
  applyPenalty = true
): Promise<ConnectSessionSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSessionSnapshot>('connect_remove_entry', {
    queueEntryId,
    applyPenalty,
  });
}

export async function connectHostReorder(
  fromIndex: number,
  toIndex: number
): Promise<ConnectSessionSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSessionSnapshot>('connect_host_reorder', {
    fromIndex,
    toIndex,
  });
}

export async function connectSetAlpha(alpha: number): Promise<ConnectSessionSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSessionSnapshot>('connect_set_alpha', { alpha });
}

export async function connectSyncNow(): Promise<ConnectSyncResult | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectSyncResult>('connect_sync_now');
}

export async function connectEndSession(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('connect_end_session');
}

export async function listenConnectSession(
  callback: (snapshot: ConnectSessionSnapshot | null) => void
): Promise<UnlistenFn> {
  if (!isNativeApp()) return () => { };
  return listen<ConnectSessionSnapshot | null>('connect:state-changed', (event) => {
    callback(event.payload);
  });
}

export async function connectGetAccountHub(): Promise<ConnectAccountHubSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectAccountHubSnapshot>('connect_get_account_hub');
}

export async function connectUpdateAccountSettings(params: {
  displayName?: string;
  avatarUrl?: string;
  hosting?: boolean;
  acceptJoinRequests?: boolean;
  showConnectId?: boolean;
}): Promise<ConnectAccountHubSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectAccountHubSnapshot>('connect_update_account_settings', params);
}

export async function connectLookupHost(connectId: string): Promise<HostLookupResult | null> {
  if (!isNativeApp()) return null;
  return invoke<HostLookupResult | null>('connect_lookup_host', { connectId });
}

export async function connectSendJoinRequest(
  hostAccountId: string
): Promise<ConnectRequestSendOutcome | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectRequestSendOutcome>('connect_send_join_request', { hostAccountId });
}

export async function connectRespondRequest(
  requestId: string,
  action: 'accept' | 'decline' | 'ignore_1d'
): Promise<ConnectRespondOutcome | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectRespondOutcome>('connect_respond_request', { requestId, action });
}

export async function connectUnignoreGuest(
  ignoredAccountId: string
): Promise<ConnectAccountHubSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectAccountHubSnapshot>('connect_unignore_guest', { ignoredAccountId });
}

export async function connectSaveHost(
  hostAccountId: string
): Promise<ConnectAccountHubSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectAccountHubSnapshot>('connect_save_host', { hostAccountId });
}

export async function connectRemoveSavedHost(
  hostAccountId: string
): Promise<ConnectAccountHubSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectAccountHubSnapshot>('connect_remove_saved_host', { hostAccountId });
}

export async function connectToSavedHost(
  hostAccountId: string
): Promise<ConnectRequestSendOutcome | null> {
  if (!isNativeApp()) return null;
  return invoke<ConnectRequestSendOutcome>('connect_to_saved_host', { hostAccountId });
}

export async function listenConnectAccountHub(
  callback: (snapshot: ConnectAccountHubSnapshot) => void
): Promise<UnlistenFn> {
  if (!isNativeApp()) return () => { };
  return listen<ConnectAccountHubSnapshot>('connect:account-changed', (event) => {
    callback(event.payload);
  });
}
