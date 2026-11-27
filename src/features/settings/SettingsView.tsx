import { memo, useEffect, useState } from 'react';
import {
  AlertTriangle,
  AudioWaveform,
  CheckCircle2,
  Cloud,
  Database,
  DownloadCloud,
  HardDrive,
  Info,
  Layers,
  Lock,
  LogOut,
  Minus,
  Music2,
  Palette,
  Plus,
  RefreshCw,
  ShieldCheck,
  Sliders,
  Trash2,
  User,
  Volume2,
  Wrench,
} from 'lucide-react';
import { Tabs } from '../../components/Tabs';
import { Badge } from '../../components/Badge';
import { Button } from '../../components/Button';
import { Modal } from '../../components/Modal';
import type { PlatformHealth } from '../../lib/platform';
import type { MotionSetting } from '../../lib/motion';
import type { PreloadStatusSnapshot, SyncAccountStatus, SyncConfigDTO } from '../../lib/types';
import { MAX_PRELOAD_TARGET } from '../../lib/connect';
import {
  getArtworkCacheSummary,
  cleanOrphanedArtwork,
  recoverMissingArtwork,
  formatBytes,
  type ArtworkCacheSummary,
  type ArtworkCleanupResult,
  type ArtworkRecoveryResult,
} from '../../lib/artwork';
import { isNativeApp } from '../../lib/library';
import {
  formatSmtcDiagnostic,
  getPersistenceBadgeConfig,
} from '../../lib/diagnostics.js';
import {
  getSyncStatus,
  syncSignup,
  syncLogin,
  syncLogout,
  toggleSync,
  syncNow,
  deleteCloudData,
  getSyncConfig,
  setSyncConfig as saveSyncConfig,
} from '../../lib/sync';

interface SettingsViewProps {
  health: PlatformHealth | null;
  density: 'standard' | 'compact';
  onDensityChange: (density: 'standard' | 'compact') => void;
  motion: MotionSetting;
  onMotionChange: (motion: MotionSetting) => void;
  onSyncConfigurationChanged: (enabled: boolean) => void;
  preloadStatus?: PreloadStatusSnapshot | null;
  onChangePreloadTarget?: (target: number) => void;
}

type SettingsSection = 'appearance' | 'audio' | 'library' | 'account' | 'about';

const SECTIONS: Array<{ id: SettingsSection; label: string; icon: typeof Palette }> = [
  { id: 'appearance', label: 'Appearance', icon: Palette },
  { id: 'audio', label: 'Audio', icon: Volume2 },
  { id: 'library', label: 'Library', icon: Database },
  { id: 'account', label: 'Account', icon: Cloud },
  { id: 'about', label: 'About', icon: Info },
];

export const SettingsView = memo(function SettingsView({
  health,
  density,
  onDensityChange,
  motion,
  onMotionChange,
  onSyncConfigurationChanged,
  preloadStatus,
  onChangePreloadTarget,
}: SettingsViewProps) {
  const [activeSection, setActiveSection] = useState<SettingsSection>('appearance');

  // Artwork cache management
  const [artworkSummary, setArtworkSummary] = useState<ArtworkCacheSummary | null>(null);
  const [artworkLoading, setArtworkLoading] = useState(false);
  const [artworkError, setArtworkError] = useState<string | null>(null);
  const [isCleaning, setIsCleaning] = useState(false);
  const [isRepairing, setIsRepairing] = useState(false);
  const [showCleanupModal, setShowCleanupModal] = useState(false);
  const [cleanupResult, setCleanupResult] = useState<ArtworkCleanupResult | null>(null);
  const [recoveryResult, setRecoveryResult] = useState<ArtworkRecoveryResult | null>(null);

  // Cloud sync
  const [syncStatus, setSyncStatus] = useState<SyncAccountStatus | null>(null);
  const [syncConfig, setSyncBackendConfig] = useState<SyncConfigDTO | null>(null);
  const [syncUrl, setSyncUrl] = useState('');
  const [syncAnonKey, setSyncAnonKey] = useState('');
  const [isSavingSyncConfig, setIsSavingSyncConfig] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [syncSuccessMsg, setSyncSuccessMsg] = useState<string | null>(null);
  const [isSyncing, setIsSyncing] = useState(false);
  const [authEmail, setAuthEmail] = useState('');
  const [authPassword, setAuthPassword] = useState('');
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [showDeleteCloudModal, setShowDeleteCloudModal] = useState(false);
  const [isDeletingCloud, setIsDeletingCloud] = useState(false);

  const fetchArtworkSummary = async () => {
    setArtworkLoading(true);
    setArtworkError(null);
    try {
      const summary = await getArtworkCacheSummary();
      setArtworkSummary(summary);
    } catch (err) {
      setArtworkError(err instanceof Error ? err.message : String(err));
    } finally {
      setArtworkLoading(false);
    }
  };

  const fetchSyncInfo = async () => {
    try {
      const status = await getSyncStatus();
      setSyncStatus(status);
    } catch (e) {
      console.error('Failed to load sync info:', e);
      setSyncError(e instanceof Error ? e.message : String(e));
    }
  };

  useEffect(() => {
    let active = true;

    // Cache and sync diagnostics are native-only. The browser design preview
    // has no Tauri backend, so querying it would only throw.
    if (!isNativeApp()) {
      return () => {
        active = false;
      };
    }

    getArtworkCacheSummary()
      .then((summary) => {
        if (active) setArtworkSummary(summary);
      })
      .catch((err) => {
        if (active) setArtworkError(err instanceof Error ? err.message : String(err));
      });

    getSyncStatus()
      .then((status) => {
        if (!active) return;
        setSyncStatus(status);
      })
      .catch((e) => {
        if (!active) return;
        setSyncError(e instanceof Error ? e.message : String(e));
      });

    getSyncConfig()
      .then((config) => {
        if (!active) return;
        setSyncBackendConfig(config);
        setSyncUrl(config.supabaseUrl);
        setSyncAnonKey(config.supabaseAnonKey);
      })
      .catch((e) => {
        if (!active) return;
        setSyncError(e instanceof Error ? e.message : String(e));
      });

    return () => {
      active = false;
    };
  }, []);

  const handleSaveSyncConfig = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!syncUrl.trim() || !syncAnonKey.trim() || isSavingSyncConfig) return;

    setIsSavingSyncConfig(true);
    setSyncError(null);
    try {
      await saveSyncConfig(syncUrl.trim(), syncAnonKey.trim());
      const [config, status] = await Promise.all([getSyncConfig(), getSyncStatus()]);
      setSyncBackendConfig(config);
      setSyncUrl(config.supabaseUrl);
      setSyncAnonKey(config.supabaseAnonKey);
      setSyncStatus(status);
      setSyncSuccessMsg('Sync backend saved');
      setTimeout(() => setSyncSuccessMsg(null), 3000);
    } catch (err) {
      setSyncError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSavingSyncConfig(false);
    }
  };

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authEmail.trim() || !authPassword) return;
    const password = authPassword;
    setAuthPassword('');
    setIsAuthenticating(true);
    setSyncError(null);
    try {
      const status = await syncLogin(authEmail.trim(), password);
      setSyncStatus(status);
      onSyncConfigurationChanged(true);
      setSyncSuccessMsg('Signed in');
      setTimeout(() => setSyncSuccessMsg(null), 3000);
    } catch (err) {
      setSyncError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsAuthenticating(false);
    }
  };

  const handleSignup = async () => {
    if (!authEmail.trim() || !authPassword) return;
    const password = authPassword;
    setAuthPassword('');
    setIsAuthenticating(true);
    setSyncError(null);
    try {
      const status = await syncSignup(authEmail.trim(), password);
      setSyncStatus(status);
      onSyncConfigurationChanged(true);
      setSyncSuccessMsg('Account created');
      setTimeout(() => setSyncSuccessMsg(null), 3000);
    } catch (err) {
      setSyncError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsAuthenticating(false);
    }
  };

  const handleLogout = async () => {
    try {
      await syncLogout();
      await fetchSyncInfo();
      onSyncConfigurationChanged(false);
    } catch (err) {
      setSyncError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleToggleSync = async (enabled: boolean) => {
    try {
      await toggleSync(enabled);
      await fetchSyncInfo();
      onSyncConfigurationChanged(enabled);
    } catch (err) {
      setSyncError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSyncNow = async () => {
    setIsSyncing(true);
    setSyncError(null);
    try {
      const result = await syncNow();
      if (result.success) {
        setSyncSuccessMsg(`Sync complete: ${result.pushedCount} pushed, ${result.pulledCount} pulled`);
        setTimeout(() => setSyncSuccessMsg(null), 4000);
      } else if (result.errorMessage) {
        setSyncError(result.errorMessage);
      }
      await fetchSyncInfo();
    } catch (err) {
      setSyncError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSyncing(false);
    }
  };

  const handleConfirmDeleteCloudData = async () => {
    setIsDeletingCloud(true);
    setSyncError(null);
    try {
      await deleteCloudData();
      setShowDeleteCloudModal(false);
      setSyncSuccessMsg('Cloud data deleted. Local library remains intact.');
      setTimeout(() => setSyncSuccessMsg(null), 4000);
      await fetchSyncInfo();
    } catch (err) {
      setSyncError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsDeletingCloud(false);
    }
  };

  const handleConfirmClean = async () => {
    setIsCleaning(true);
    setArtworkError(null);
    try {
      const result = await cleanOrphanedArtwork();
      setCleanupResult(result);
      setShowCleanupModal(false);
      await fetchArtworkSummary();
    } catch (err) {
      setArtworkError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCleaning(false);
    }
  };

  const handleRepairMissing = async () => {
    setIsRepairing(true);
    setArtworkError(null);
    try {
      const result = await recoverMissingArtwork();
      setRecoveryResult(result);
      await fetchArtworkSummary();
    } catch (err) {
      setArtworkError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsRepairing(false);
    }
  };

  return (
    <section className="cavia-subpage settings-page">
      <header className="settings-header">
        <div className="library-heading">
          <p className="page-eyebrow">Preferences</p>
          <h1 className="library-title">
            Settings<span>.</span>
          </h1>
        </div>
      </header>

      <div className="settings-layout">
        <nav className="settings-nav" aria-label="Settings sections">
          {SECTIONS.map((section) => {
            const Icon = section.icon;
            const isActive = activeSection === section.id;
            return (
              <button
                key={section.id}
                type="button"
                className={`settings-nav-item ${isActive ? 'active' : ''}`}
                aria-current={isActive ? 'page' : undefined}
                onClick={() => setActiveSection(section.id)}
              >
                <Icon size={16} />
                <span>{section.label}</span>
              </button>
            );
          })}
        </nav>

        <div className="settings-content">
          <section
            className={`settings-section ${activeSection === 'appearance' ? 'is-active' : ''}`}
            aria-labelledby="settings-appearance"
          >
            <h2 id="settings-appearance" className="settings-section-title">Appearance</h2>

            <div className="settings-card">
              <div className="card-head">
                <Layers size={18} className="card-icon" />
                <div>
                  <h3>Interface density</h3>
                  <p>Adjust track row heights.</p>
                </div>
              </div>
              <div className="setting-control-row">
                <Tabs
                  items={[
                    { id: 'standard', label: 'Standard' },
                    { id: 'compact', label: 'Compact' },
                  ]}
                  activeId={density}
                  onChange={onDensityChange}
                  variant="pill"
                />
              </div>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <Palette size={18} className="card-icon" />
                <div>
                  <h3>Palette</h3>
                  <p>Ink surfaces, bone type, one sodium lamp. Artwork tints the glow, never the text.</p>
                </div>
              </div>
              <div className="palette-strip">
                <span className="palette-swatch" style={{ background: '#08070d' }} title="Ink canvas #08070d" />
                <span className="palette-swatch" style={{ background: '#1e1a2b' }} title="Ink card #1e1a2b" />
                <span className="palette-swatch" style={{ background: '#f6f1e7' }} title="Bone #f6f1e7" />
                <span className="palette-swatch" style={{ background: '#ffb23e' }} title="Sodium #ffb23e" />
                <span className="palette-swatch" style={{ background: '#4fe3c1' }} title="Lagoon #4fe3c1" />
              </div>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <Layers size={18} className="card-icon" />
                <div>
                  <h3>Motion</h3>
                  <p>Vinyl spin, sleeve pull and page transitions follow this setting.</p>
                </div>
              </div>
              <div className="setting-control-row">
                <Tabs
                  items={[
                    { id: 'full', label: 'Full' },
                    { id: 'reduced', label: 'Reduced' },
                    { id: 'off', label: 'Off' },
                  ]}
                  activeId={motion}
                  onChange={(id) => onMotionChange(id as MotionSetting)}
                  variant="pill"
                />
              </div>
              <p className="text-muted-xs">
                {motion === 'full'
                  ? 'Every signature animation runs.'
                  : motion === 'reduced'
                    ? 'Spin, drift, tilt and parallax are disabled. Fades stay at 120ms.'
                    : 'All animation is off, including the ambient Afterglow.'}
              </p>
            </div>
          </section>

          <section
            className={`settings-section ${activeSection === 'audio' ? 'is-active' : ''}`}
            aria-labelledby="settings-audio"
          >
            <h2 id="settings-audio" className="settings-section-title">Audio</h2>

            <div className="cavia-banner banner-info">
              <Music2 size={17} className="banner-icon-accent" />
              <div className="banner-content">
                <strong>Playback engine</strong>
                <p>
                  {health?.platform === 'android'
                    ? 'Android uses the native Media3 foreground engine when integrated.'
                    : health?.platform === 'ios'
                      ? 'iOS native AVPlayer backend integration is not enabled in this build.'
                      : 'Controls stay disabled until a real audio backend initializes. Desktop platforms use the native Rodio audio engine.'}
                </p>
              </div>
            </div>

            <div className="settings-grid-two">
              <div className="settings-card">
                <div className="card-head">
                  <Sliders size={18} className="card-icon" />
                  <div>
                    <h3>Backend status</h3>
                    <p>{health?.platform ?? 'Unknown platform'} · {health?.backend_name ?? 'rodio_native'}</p>
                  </div>
                </div>
                <div className="setting-status-line">
                  <Badge
                    variant={
                      health?.audio_backend === 'ready'
                        ? 'ready'
                        : health?.audio_backend === 'error'
                          ? 'warn'
                          : 'outline'
                    }
                    size="md"
                  >
                    {health?.audio_backend === 'ready'
                      ? 'INTEGRATED'
                      : health?.audio_backend === 'device_unavailable'
                        ? 'NO AUDIO DEVICE'
                        : health?.audio_backend === 'not_integrated'
                          ? 'NOT INTEGRATED'
                          : health?.audio_backend === 'error'
                            ? 'INITIALIZATION ERROR'
                            : health?.audio_backend === 'unsupported'
                              ? 'UNSUPPORTED'
                              : health?.audio_backend === 'initializing'
                                ? 'INITIALIZING'
                                : 'STANDBY'}
                  </Badge>
                  <span className="text-muted-xs">
                    {formatSmtcDiagnostic(
                      health?.smtc_status,
                      health?.smtc_error,
                      health?.system_media_controls
                    )}
                  </span>
                </div>
              </div>

              <div className="settings-card">
                <div className="card-head">
                  <Volume2 size={18} className="card-icon" />
                  <div>
                    <h3>Output</h3>
                    <p>
                      {health?.platform === 'android'
                        ? 'Android Media3 audio pipeline.'
                        : health?.platform === 'darwin'
                          ? 'Direct CoreAudio path.'
                          : health?.platform === 'windows'
                            ? 'Direct WASAPI path.'
                            : 'Native audio output path.'}
                    </p>
                  </div>
                </div>
                <div className="setting-status-line">
                  <span className="text-dim-sm">Resampling is not enabled.</span>
                  <Badge variant="neutral" size="sm">STANDBY</Badge>
                </div>
              </div>

              <div className="settings-card">
                <div className="card-head">
                  <Database size={18} className="card-icon" />
                  <div>
                    <h3>Session persistence</h3>
                    <p>Playback state is written to SQLite off the UI thread.</p>
                  </div>
                </div>
                <div className="setting-status-line">
                  {(() => {
                    const persistenceBadge = getPersistenceBadgeConfig(health?.persistence_status);
                    return (
                      <Badge variant={persistenceBadge.variant} size="md">
                        {persistenceBadge.label}
                      </Badge>
                    );
                  })()}
                  <span className="text-muted-xs">
                    {health?.persistence_error ? health.persistence_error : 'Non-blocking WAL writes'}
                  </span>
                </div>
              </div>

              {onChangePreloadTarget && (
                <div className="settings-card">
                  <div className="card-head">
                    <DownloadCloud size={18} className="card-icon" />
                    <div>
                      <h3>Queue preload window</h3>
                      <p>
                        Sequential verified cache for upcoming tracks (0–20). Set to 0 to disable and hide the Now Playing pill.
                      </p>
                    </div>
                  </div>
                  <div className="setting-status-line">
                    <div className="queue-preload-stepper">
                      <button
                        type="button"
                        className="preload-step-btn"
                        disabled={!preloadStatus || preloadStatus.targetCount <= 0}
                        onClick={() =>
                          onChangePreloadTarget(
                            Math.max(0, (preloadStatus?.targetCount ?? 0) - 1)
                          )
                        }
                        aria-label="Decrease preload count"
                      >
                        <Minus size={12} />
                      </button>
                      <span className="queue-preload-num">
                        {preloadStatus?.targetCount ?? 10} tracks
                      </span>
                      <button
                        type="button"
                        className="preload-step-btn"
                        disabled={(preloadStatus?.targetCount ?? 10) >= MAX_PRELOAD_TARGET}
                        onClick={() =>
                          onChangePreloadTarget(
                            Math.min(MAX_PRELOAD_TARGET, (preloadStatus?.targetCount ?? 0) + 1)
                          )
                        }
                        aria-label="Increase preload count"
                      >
                        <Plus size={12} />
                      </button>
                    </div>
                    <span className="text-muted-xs">
                      {preloadStatus?.enabled
                        ? `${preloadStatus.readyCount}/${preloadStatus.targetCount} ready (${formatBytes(preloadStatus.cacheUsedBytes)} used)`
                        : 'Disabled'}
                    </span>
                  </div>
                </div>
              )}
            </div>
          </section>

          <section
            className={`settings-section ${activeSection === 'library' ? 'is-active' : ''}`}
            aria-labelledby="settings-library"
          >
            <h2 id="settings-library" className="settings-section-title">Library</h2>

            <div className="settings-card">
              <div className="card-head">
                <Database size={18} className="card-icon" />
                <div>
                  <h3>SQLite database</h3>
                  <p>Rust-owned connection with forward-only migrations.</p>
                </div>
              </div>
              <div className="sqlite-stats-grid">
                <div className="sqlite-stat-box">
                  <span className="box-label">CONNECTION</span>
                  <strong className="box-val text-ready">
                    {health?.database_connected ? 'CONNECTED' : 'DISCONNECTED'}
                  </strong>
                </div>
                <div className="sqlite-stat-box">
                  <span className="box-label">MIGRATION</span>
                  <strong className="box-val">{health?.active_migration ?? '—'}</strong>
                </div>
                <div className="sqlite-stat-box">
                  <span className="box-label">JOURNAL</span>
                  <strong className="box-val">WAL</strong>
                </div>
              </div>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <HardDrive size={18} className="card-icon" />
                <div>
                  <h3>Artwork cache</h3>
                  <p>Managed local artwork with SHA-256 deduplication.</p>
                </div>
              </div>

              {artworkSummary && !artworkSummary.cacheDirExists && (
                <div className="cavia-banner banner-warn settings-inline-banner">
                  <AlertTriangle size={16} className="text-warn" />
                  <div className="banner-content">
                    <strong>Cache directory inaccessible</strong>
                    <p><code>{artworkSummary.cacheDirPath}</code></p>
                  </div>
                </div>
              )}

              {artworkError && (
                <div className="cavia-banner banner-error settings-inline-banner">
                  <AlertTriangle size={16} className="text-error" />
                  <div className="banner-content">
                    <strong>Operation error</strong>
                    <p>{artworkError}</p>
                  </div>
                </div>
              )}

              {cleanupResult && (
                <div className="cavia-banner banner-info settings-inline-banner">
                  <CheckCircle2 size={16} className="text-ready" />
                  <div className="banner-content">
                    <strong>Cleanup complete</strong>
                    <p>
                      Reclaimed {formatBytes(cleanupResult.reclaimedBytes)} by removing{' '}
                      {cleanupResult.deletedFiles} orphaned files.
                    </p>
                  </div>
                </div>
              )}

              {recoveryResult && (
                <div className="cavia-banner banner-info settings-inline-banner">
                  <CheckCircle2 size={16} className="text-ready" />
                  <div className="banner-content">
                    <strong>Recovery complete</strong>
                    <p>
                      {recoveryResult.recoveredTracks} restored, {recoveryResult.alreadyValidTracks} already
                      valid, {recoveryResult.noEmbeddedArtwork} without embedded artwork.
                    </p>
                  </div>
                </div>
              )}

              <div className="sqlite-stats-grid">
                <div className="sqlite-stat-box">
                  <span className="box-label">CACHE USAGE</span>
                  <strong className="box-val">
                    {artworkLoading ? 'Scanning…' : formatBytes(artworkSummary?.totalBytes ?? 0)}
                  </strong>
                  <span className="box-subval">{artworkSummary?.totalFiles ?? 0} files</span>
                </div>
                <div className="sqlite-stat-box">
                  <span className="box-label">REFERENCED</span>
                  <strong className="box-val text-ready">
                    {artworkLoading ? '…' : `${artworkSummary?.referencedFiles ?? 0} active`}
                  </strong>
                </div>
                <div className="sqlite-stat-box">
                  <span className="box-label">ORPHANED</span>
                  <strong className={`box-val ${(artworkSummary?.orphanedFiles ?? 0) > 0 ? 'text-warn' : 'text-ready'}`}>
                    {artworkLoading ? '…' : `${artworkSummary?.orphanedFiles ?? 0} files`}
                  </strong>
                  <span className="box-subval">{formatBytes(artworkSummary?.reclaimableBytes ?? 0)} reclaimable</span>
                </div>
                <div className="sqlite-stat-box">
                  <span className="box-label">MISSING</span>
                  <strong className={`box-val ${(artworkSummary?.missingReferencedFiles ?? 0) > 0 ? 'text-error' : 'text-ready'}`}>
                    {artworkLoading ? '…' : `${artworkSummary?.missingReferencedFiles ?? 0} tracks`}
                  </strong>
                  <span className="box-subval">Repairable from audio</span>
                </div>
              </div>

              <div className="settings-button-row">
                <Button
                  variant="outline"
                  size="sm"
                  icon={<RefreshCw size={13} className={artworkLoading ? 'cavia-spin' : undefined} />}
                  onClick={fetchArtworkSummary}
                  disabled={artworkLoading || isCleaning || isRepairing}
                >
                  {artworkLoading ? 'Inspecting…' : 'Refresh'}
                </Button>
                <Button
                  variant={(artworkSummary?.orphanedFiles ?? 0) > 0 ? 'primary' : 'outline'}
                  size="sm"
                  icon={<Trash2 size={13} />}
                  onClick={() => setShowCleanupModal(true)}
                  disabled={artworkLoading || isCleaning || isRepairing || (artworkSummary?.orphanedFiles ?? 0) === 0}
                >
                  Clean orphaned
                </Button>
                <Button
                  variant={(artworkSummary?.missingReferencedFiles ?? 0) > 0 ? 'primary' : 'outline'}
                  size="sm"
                  icon={<Wrench size={13} />}
                  onClick={handleRepairMissing}
                  disabled={
                    artworkLoading ||
                    isCleaning ||
                    isRepairing ||
                    (artworkSummary?.missingReferencedFiles ?? 0) === 0
                  }
                >
                  {isRepairing ? 'Repairing…' : 'Repair missing'}
                </Button>
              </div>
            </div>
          </section>

          <section
            className={`settings-section ${activeSection === 'account' ? 'is-active' : ''}`}
            aria-labelledby="settings-account"
          >
            <h2 id="settings-account" className="settings-section-title">Account</h2>

            <div className="cavia-banner banner-info">
              <Cloud size={16} className="banner-icon text-ready" />
              <div className="banner-content">
                <strong>Optional cloud sync</strong>
                <p>Playlists and likes can sync. Audio files and local paths never leave this device.</p>
              </div>
            </div>

            {syncSuccessMsg && (
              <div className="sync-alert sync-alert-success">
                <CheckCircle2 size={15} />
                <span>{syncSuccessMsg}</span>
              </div>
            )}

            {syncError && (
              <div className="sync-alert sync-alert-error">
                <AlertTriangle size={15} />
                <span>{syncError}</span>
              </div>
            )}

            <div className="settings-card">
              <div className="card-head">
                <Cloud size={18} className="card-icon" />
                <div>
                  <h3>Sync backend</h3>
                  <p>Point Cavia at your Supabase project. Only the publishable anon key is accepted.</p>
                </div>
              </div>

              <div className="setting-status-line">
                <Badge variant={syncConfig?.isConfigured ? 'ready' : 'outline'} size="sm">
                  {syncConfig?.isConfigured ? 'CONFIGURED' : 'NOT CONFIGURED'}
                </Badge>
                {syncConfig?.isConfigured && (
                  <span className="text-muted-xs">
                    {syncConfig.supabaseUrl}
                  </span>
                )}
              </div>

              <form onSubmit={handleSaveSyncConfig} className="sync-auth-form">
                <div className="form-field">
                  <label htmlFor="sync-backend-url">Project URL</label>
                  <input
                    id="sync-backend-url"
                    type="url"
                    className="cavia-text-input"
                    placeholder="https://your-project.supabase.co"
                    value={syncUrl}
                    onChange={(e) => setSyncUrl(e.target.value)}
                    disabled={isSavingSyncConfig}
                    required
                  />
                </div>

                <div className="form-field">
                  <label htmlFor="sync-backend-key">Publishable anon key</label>
                  <input
                    id="sync-backend-key"
                    type="password"
                    className="cavia-text-input"
                    placeholder="eyJhbGciOi…"
                    value={syncAnonKey}
                    onChange={(e) => setSyncAnonKey(e.target.value)}
                    disabled={isSavingSyncConfig}
                    autoComplete="off"
                    required
                  />
                </div>

                <p className="text-muted-xs">
                  The anon key is stored in the local app database. Your session refresh token stays in
                  the OS credential manager.
                </p>

                <div className="sync-auth-actions">
                  <Button
                    type="submit"
                    variant="primary"
                    size="sm"
                    disabled={isSavingSyncConfig || !syncUrl.trim() || !syncAnonKey.trim()}
                  >
                    {isSavingSyncConfig ? 'Saving…' : 'Save backend'}
                  </Button>
                </div>
              </form>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <User size={18} className="card-icon text-vermilion" />
                <div>
                  <h3>Cavia account</h3>
                  <p>Sign in to synchronize your library metadata.</p>
                </div>
              </div>

              {syncStatus && !syncStatus.backendConfigured && (
                <p className="text-muted-xs" role="status">
                  Account sync is unavailable right now. Everything else in Cavia keeps working
                  offline.
                </p>
              )}

              {syncStatus?.authenticated ? (
                <div className="settings-stack">
                  <div className="setting-status-line">
                    <Badge variant="ready" size="sm">AUTHENTICATED</Badge>
                    <span className="text-muted-xs font-mono">
                      {syncStatus.accountEmail || syncStatus.accountId}
                    </span>
                  </div>

                  <div className="sync-diagnostic-grid">
                    <div className="sync-diag-item">
                      <span className="sync-diag-label">DEVICE</span>
                      <strong className="sync-diag-value">{syncStatus.deviceId}</strong>
                    </div>
                    <div className="sync-diag-item">
                      <span className="sync-diag-label">TOKEN STORAGE</span>
                      <strong className="sync-diag-value text-ready">OS credential manager</strong>
                    </div>
                  </div>

                  <div>
                    <Button variant="outline" size="sm" icon={<LogOut size={14} />} onClick={handleLogout}>
                      Sign out
                    </Button>
                  </div>
                </div>
              ) : (
                <form onSubmit={handleLogin} className="sync-auth-form">
                  <div className="form-field">
                    <label htmlFor="sync-email">Email</label>
                    <input
                      id="sync-email"
                      type="email"
                      className="cavia-text-input"
                      placeholder="you@example.com"
                      value={authEmail}
                      onChange={(e) => setAuthEmail(e.target.value)}
                      disabled={isAuthenticating}
                      required
                    />
                  </div>

                  <div className="form-field">
                    <label htmlFor="sync-password">Password</label>
                    <input
                      id="sync-password"
                      type="password"
                      className="cavia-text-input"
                      placeholder="••••••••"
                      value={authPassword}
                      onChange={(e) => setAuthPassword(e.target.value)}
                      disabled={isAuthenticating}
                      required
                    />
                  </div>

                  <div className="sync-auth-actions">
                    <Button
                      type="submit"
                      variant="primary"
                      size="sm"
                      disabled={isAuthenticating || !authEmail.trim() || !authPassword}
                    >
                      {isAuthenticating ? 'Signing in…' : 'Sign in'}
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={handleSignup}
                      disabled={isAuthenticating || !authEmail.trim() || !authPassword}
                    >
                      Create account
                    </Button>
                  </div>
                </form>
              )}
            </div>

            <div className="settings-card">
              <div className="card-head">
                <Cloud size={18} className="card-icon" />
                <div>
                  <h3>Synchronization</h3>
                  <p>Playlist structure and favorites only.</p>
                </div>
              </div>

              <div className="settings-stack">
                <div className="setting-row">
                  <div className="setting-info">
                    <label htmlFor="enable-cloud-sync" className="setting-label">
                      Enable sync
                    </label>
                    <span className="setting-desc">Requires a signed-in account.</span>
                  </div>
                  <input
                    id="enable-cloud-sync"
                    type="checkbox"
                    className="cavia-toggle"
                    checked={syncStatus?.syncEnabled ?? false}
                    onChange={(e) => handleToggleSync(e.target.checked)}
                    disabled={!syncStatus?.authenticated}
                    aria-label="Enable cloud synchronization"
                  />
                </div>

                <div className="sync-diagnostic-grid">
                  <div className="sync-diag-item">
                    <span className="sync-diag-label">LAST SYNCED</span>
                    <strong className="sync-diag-value">
                      {syncStatus?.lastSyncedAt ? new Date(syncStatus.lastSyncedAt).toLocaleString() : 'Never'}
                    </strong>
                  </div>
                  <div className="sync-diag-item">
                    <span className="sync-diag-label">PENDING</span>
                    <strong className="sync-diag-value">{syncStatus?.pendingOutboxCount ?? 0} changes</strong>
                  </div>
                </div>

                {syncStatus?.lastError && (
                  <div className="sync-alert sync-alert-error">
                    <AlertTriangle size={15} />
                    <span>{syncStatus.lastError}</span>
                  </div>
                )}

                <div>
                  <Button
                    variant="primary"
                    size="sm"
                    icon={<RefreshCw size={14} className={isSyncing ? 'animate-spin' : ''} />}
                    onClick={handleSyncNow}
                    disabled={isSyncing || !syncStatus?.authenticated || !syncStatus?.syncEnabled}
                  >
                    {isSyncing ? 'Synchronizing…' : 'Sync now'}
                  </Button>
                </div>
              </div>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <ShieldCheck size={18} className="card-icon text-ready" />
                <div>
                  <h3>Data boundary</h3>
                  <p>Metadata syncs. Audio and local paths stay here.</p>
                </div>
              </div>

              <div className="sync-boundary-grid">
                <div className="sync-boundary-box">
                  <strong className="sync-boundary-title ready">
                    <CheckCircle2 size={14} /> Synchronized
                  </strong>
                  <ul className="sync-boundary-list">
                    <li>Playlist titles, descriptions, and order</li>
                    <li>Track metadata snapshots</li>
                    <li>Liked tracks</li>
                    <li>Device identifier</li>
                  </ul>
                </div>

                <div className="sync-boundary-box">
                  <strong className="sync-boundary-title warn">
                    <Lock size={14} /> Local only
                  </strong>
                  <ul className="sync-boundary-list">
                    <li>Audio files and streams</li>
                    <li>File paths and drive letters</li>
                    <li>Artwork cache</li>
                    <li>Playback queue and volume</li>
                  </ul>
                </div>
              </div>

              {syncStatus?.authenticated && (
                <div className="sync-danger-row">
                  <div className="sync-danger-info">
                    <strong className="sync-danger-title">Delete cloud data</strong>
                    <span className="sync-danger-desc">Removes synchronized playlists and likes. Local files stay intact.</span>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setShowDeleteCloudModal(true)}
                    style={{ color: 'var(--status-error)', borderColor: 'rgba(223, 88, 73, 0.4)' }}
                  >
                    Delete
                  </Button>
                </div>
              )}
            </div>

          </section>

          <section
            className={`settings-section ${activeSection === 'about' ? 'is-active' : ''}`}
            aria-labelledby="settings-about"
          >
            <h2 id="settings-about" className="settings-section-title">About</h2>

            <div className="settings-card about-hero-card">
              <div className="about-brand-row">
                <AudioWaveform size={26} className="text-vermilion" />
                <div>
                  <h2>cavia music<span>.</span></h2>
                  <span className="about-edition">
                    VERSION {health?.app_version ?? '—'} · AFTERGLOW
                  </span>
                </div>
              </div>

              <p className="about-text">
                A local-first listening room, built with Tauri 2, React 19, TypeScript, and Rust.
              </p>

              <div className="about-badges">
                <Badge variant="vermilion" size="sm">TAURI 2</Badge>
                <Badge variant="neutral" size="sm">SQLITE</Badge>
                <Badge variant="neutral" size="sm">REACT 19</Badge>
                <Badge variant="outline" size="sm">NO TELEMETRY</Badge>
              </div>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <ShieldCheck size={18} className="card-icon text-ready" />
                <div>
                  <h3>Principles</h3>
                  <p>Non-negotiable product integrity rules.</p>
                </div>
              </div>
              <ul className="principles-list">
                <li>
                  <CheckCircle2 size={14} className="text-ready" />
                  <span>No fabricated tracks, artwork, or playback progress.</span>
                </li>
                <li>
                  <CheckCircle2 size={14} className="text-ready" />
                  <span>Provider secrets live in platform secure storage, never SQLite.</span>
                </li>
                <li>
                  <CheckCircle2 size={14} className="text-ready" />
                  <span>Local audio files remain untouched and offline.</span>
                </li>
                <li>
                  <CheckCircle2 size={14} className="text-ready" />
                  <span>Controls stay disabled when their backend is absent.</span>
                </li>
              </ul>
            </div>
          </section>
        </div>
      </div>

      <Modal
        isOpen={showCleanupModal}
        onClose={() => setShowCleanupModal(false)}
        title="Clean orphaned artwork"
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setShowCleanupModal(false)} disabled={isCleaning}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" icon={<Trash2 size={14} />} onClick={handleConfirmClean} disabled={isCleaning}>
              {isCleaning ? 'Cleaning…' : 'Delete files'}
            </Button>
          </div>
        }
      >
        <div className="modal-stack">
          <p className="modal-text">
            Cached artwork no longer referenced by any track or playlist will be permanently removed.
          </p>
          <div className="modal-stat-row">
            <span>Files to remove</span>
            <strong>{artworkSummary?.orphanedFiles ?? 0}</strong>
          </div>
          <div className="modal-stat-row">
            <span>Reclaimable space</span>
            <strong className="text-ready">{formatBytes(artworkSummary?.reclaimableBytes ?? 0)}</strong>
          </div>
          <p className="modal-text modal-text-muted">Referenced artwork remains intact.</p>
        </div>
      </Modal>

      <Modal
        isOpen={showDeleteCloudModal}
        onClose={() => setShowDeleteCloudModal(false)}
        title="Delete cloud data"
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setShowDeleteCloudModal(false)} disabled={isDeletingCloud}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" icon={<Trash2 size={14} />} onClick={handleConfirmDeleteCloudData} disabled={isDeletingCloud}>
              {isDeletingCloud ? 'Deleting…' : 'Delete cloud data'}
            </Button>
          </div>
        }
      >
        <div className="modal-stack">
          <p className="modal-text">
            This permanently deletes synchronized playlists, tracks, and favorites from your cloud account.
          </p>
          <div className="sync-info-box">
            <strong className="sync-info-title">Local library remains intact.</strong>
            Audio files and local database entries on this device are not affected.
          </div>
        </div>
      </Modal>
    </section>
  );
});
