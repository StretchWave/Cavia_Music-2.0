import { invoke } from '@tauri-apps/api/core';
import type { SyncAccountStatus, SyncConfigDTO, SyncResult } from './types';

export async function getSyncStatus(): Promise<SyncAccountStatus> {
  return await invoke<SyncAccountStatus>('sync_get_status');
}

export async function syncSignup(email: string, password: string): Promise<SyncAccountStatus> {
  return await invoke<SyncAccountStatus>('sync_signup', { email, password });
}

export async function syncLogin(email: string, password: string): Promise<SyncAccountStatus> {
  return await invoke<SyncAccountStatus>('sync_login', { email, password });
}

export async function syncLogout(): Promise<void> {
  await invoke('sync_logout');
}

export async function toggleSync(enabled: boolean): Promise<boolean> {
  return await invoke<boolean>('sync_toggle', { enabled });
}

export async function syncNow(): Promise<SyncResult> {
  return await invoke<SyncResult>('sync_now');
}

export async function deleteCloudData(): Promise<void> {
  await invoke('sync_delete_cloud_data');
}

export async function getSyncConfig(): Promise<SyncConfigDTO> {
  return await invoke<SyncConfigDTO>('sync_get_config');
}

export async function setSyncConfig(url: string, anonKey: string): Promise<void> {
  await invoke('sync_set_config', { url, anonKey });
}

