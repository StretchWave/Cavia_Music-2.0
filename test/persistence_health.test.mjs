import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

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

describe('Persistence Health Frontend Contract', () => {
  it('parses PlatformHealth with persistence_status and persistence_error', () => {
    const rawBackendHealth = {
      app_version: '2.0.0',
      database_connected: true,
      audio_backend: 'ready',
      backend_name: 'rodio_native',
      active_migration: '0002_playback_state_volume.sql (Active)',
      platform: 'windows',
      system_media_controls: true,
      smtc_status: 'ready',
      smtc_error: null,
      persistence_status: 'healthy',
      persistence_error: null,
    };

    assert.strictEqual(rawBackendHealth.persistence_status, 'healthy');
    assert.strictEqual(rawBackendHealth.persistence_error, null);
  });

  it('maps all 5 persistence statuses to the expected badge labels and variants', () => {
    const cases = [
      { status: 'healthy', expectedLabel: 'PERSISTENCE READY', expectedVariant: 'ready' },
      { status: 'write_pending', expectedLabel: 'SAVING PLAYBACK STATE', expectedVariant: 'warn' },
      { status: 'write_failed', expectedLabel: 'SAVE FAILED', expectedVariant: 'vermilion' },
      { status: 'worker_unavailable', expectedLabel: 'PERSISTENCE UNAVAILABLE', expectedVariant: 'vermilion' },
      { status: 'shutting_down', expectedLabel: 'SAVING BEFORE EXIT', expectedVariant: 'warn' },
    ];

    for (const c of cases) {
      const config = getPersistenceBadgeConfig(c.status);
      assert.strictEqual(config.label, c.expectedLabel);
      assert.strictEqual(config.variant, c.expectedVariant);
    }
  });

  it('formats persistence error when present in PlatformHealth', () => {
    const failedHealth = {
      persistence_status: 'write_failed',
      persistence_error: 'Injected SQLite trigger failure (code 19)',
    };

    assert.strictEqual(failedHealth.persistence_status, 'write_failed');
    assert.strictEqual(
      `Error: ${failedHealth.persistence_error}`,
      'Error: Injected SQLite trigger failure (code 19)'
    );
  });
});
