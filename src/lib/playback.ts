import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isNativeApp } from './platform';
import type { RepeatMode, Track } from './types';

export interface BackendPlaybackSnapshot {
  generation?: number;
  status: 'idle' | 'loading' | 'playing' | 'paused' | 'stopped' | 'error';
  currentTrack: Track | null;
  positionMs: number;
  durationMs: number;
  volume: number;
  isMuted: boolean;
  repeatMode: RepeatMode;
  isShuffle: boolean;
  playbackSpeed: number;
  queue: Track[];
  errorMessage?: string;
  deviceReady?: boolean;
}

export interface PositionUpdatePayload {
  generation?: number;
  trackId?: string;
  positionMs: number;
  durationMs: number;
}

export async function getPlaybackState(): Promise<BackendPlaybackSnapshot | null> {
  if (!isNativeApp()) return null;
  return invoke<BackendPlaybackSnapshot>('get_playback_state');
}

export async function playTrack(trackId: string): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('play_track', { trackId });
}

export async function play(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('play');
}

export async function pause(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('pause');
}

export async function playPause(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('play_pause');
}

export async function stop(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('stop');
}

export async function seek(positionMs: number): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('seek', { positionMs: Math.round(positionMs) });
}

export async function setVolume(volume: number): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('set_volume', { volume: Math.round(volume) });
}

export async function toggleMute(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('toggle_mute');
}

export async function setPlaybackSpeed(speed: number): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('set_playback_speed', { speed });
}

export async function setRepeatMode(mode: RepeatMode): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('set_repeat_mode', { mode });
}

export async function setShuffle(enabled: boolean): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('set_shuffle', { enabled });
}

export async function nextTrack(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('next_track');
}

export async function previousTrack(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('previous_track');
}

export async function setQueue(trackIds: string[], startTrackId?: string): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('set_queue', { trackIds, startTrackId });
}

export async function addToQueue(trackId: string): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('add_to_queue', { trackId });
}

export async function removeFromQueue(trackId: string): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('remove_from_queue', { trackId });
}

export async function removeFromQueueByIndex(index: number): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('remove_from_queue_by_index', { index });
}

export async function reorderQueue(fromIndex: number, toIndex: number): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('reorder_queue', { fromIndex, toIndex });
}

export async function clearQueue(): Promise<void> {
  if (!isNativeApp()) return;
  return invoke('clear_queue');
}

export async function listenPlaybackState(
  callback: (snapshot: BackendPlaybackSnapshot) => void
): Promise<UnlistenFn> {
  return listen<BackendPlaybackSnapshot>('playback:state-changed', (event) => {
    callback(event.payload);
  });
}

export async function listenPlaybackPosition(
  callback: (pos: PositionUpdatePayload) => void
): Promise<UnlistenFn> {
  return listen<PositionUpdatePayload>('playback:position', (event) => {
    callback(event.payload);
  });
}
