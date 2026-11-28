import { memo, useEffect, useState } from 'react';
import {
  AlertTriangle,
  AudioWaveform,
  CheckCircle2,
  Cloud,
  Database,
  DownloadCloud,
  HardDrive,
  History,
  Info,
  Layers,
  Lock,
  LogOut,
  Minus,
  Music2,
  Palette,
  Plus,
  Radio,
  RefreshCw,
  ShieldBan,
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
import type {
  ConnectAccountHubSnapshot,
  PreloadStatusSnapshot,
  SyncAccountStatus,
} from '../../lib/types';
import {
  connectUnignoreGuest,
  connectUpdateAccountSettings,
  MAX_PRELOAD_TARGET,
} from '../../lib/connect';
import {
  getArtworkCacheSummary,
  cleanOrphanedArtwork,
  recoverMissingArtwork,
  formatBytes,
  type ArtworkCacheSummary,
  type ArtworkCleanupResult,
  type ArtworkRecoveryResult,
} from '../../lib/artwork';
import {
  clearArtworkCache,
  clearLyricsCache,
  clearRecentPlays,
  clearStreamCache,
  getLibraryStorageSummary,
  isNativeApp,
  type LibraryStorageSummary,
} from '../../lib/library';
import { clearCompletedImports } from '../../lib/spotify';
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
  connectHub?: ConnectAccountHubSnapshot | null;
  onConnectHubChanged?: (hub: ConnectAccountHubSnapshot | null) => void;
}

type SettingsSection = 'appearance' | 'audio' | 'library' | 'profile' | 'about';

const SECTIONS: Array<{ id: SettingsSection; label: string; icon: typeof Palette }> = [
  { id: 'appearance', label: 'Appearance', icon: Palette },
  { id: 'audio', label: 'Audio', icon: Volume2 },
  { id: 'library', label: 'Library', icon: Database },
  { id: 'profile', label: 'Profile', icon: User },
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
  connectHub,
  onConnectHubChanged,
}: SettingsViewProps) {
  const [activeSection, setActiveSection] = useState<SettingsSection>('appearance');

  // Preload preference memory when toggling off -> on
  const [lastPreloadTarget, setLastPreloadTarget] = useState(10);

  // Library storage & cache management
  const [storageSummary, setStorageSummary] = useState<LibraryStorageSummary | null>(null);
  const [artworkSummary, setArtworkSummary] = useState<ArtworkCacheSummary | null>(null);
  const [libraryLoading, setLibraryLoading] = useState(false);
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [librarySuccessMsg, setLibrarySuccessMsg] = useState<string | null>(null);
  const [isCleaning, setIsCleaning] = useState(false);
  const [isRepairing, setIsRepairing] = useState(false);
  const [showCleanupModal, setShowCleanupModal] = useState(false);
  const [showClearAllDownloadsModal, setShowClearAllDownloadsModal] = useState(false);
  const [showClearAllArtworkModal, setShowClearAllArtworkModal] = useState(false);
  const [cleanupResult, setCleanupResult] = useState<ArtworkCleanupResult | null>(null);
  const [recoveryResult, setRecoveryResult] = useState<ArtworkRecoveryResult | null>(null);

  // Profile, Cloud Sync & Cavia Connect identity
  const [syncStatus, setSyncStatus] = useState<SyncAccountStatus | null>(null);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [profileSuccessMsg, setProfileSuccessMsg] = useState<string | null>(null);
  const [isSyncing, setIsSyncing] = useState(false);
  const [authEmail, setAuthEmail] = useState('');
  const [authPassword, setAuthPassword] = useState('');
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [showDeleteCloudModal, setShowDeleteCloudModal] = useState(false);
  const [isDeletingCloud, setIsDeletingCloud] = useState(false);
  const [connectDisplayNameDraft, setConnectDisplayNameDraft] = useState(
    connectHub?.profile.displayName ?? ''
  );
  const [connectSettingsBusy, setConnectSettingsBusy] = useState(false);

  useEffect(() => {
    if (preloadStatus?.targetCount && preloadStatus.targetCount > 0) {
      setLastPreloadTarget(preloadStatus.targetCount);
    }
  }, [preloadStatus?.targetCount]);

  useEffect(() => {
    if (connectHub?.profile.displayName) {
      setConnectDisplayNameDraft(connectHub.profile.displayName);
    }
  }, [connectHub?.profile.displayName]);

  const showLibraryNotice = (msg: string) => {
    setLibrarySuccessMsg(msg);
    setTimeout(() => setLibrarySuccessMsg(null), 4000);
  };

  const fetchLibraryStorage = async () => {
    setLibraryLoading(true);
    setLibraryError(null);
    try {
      const [storage, artwork] = await Promise.all([
        getLibraryStorageSummary(),
        getArtworkCacheSummary(),
      ]);
      setStorageSummary(storage);
      setArtworkSummary(artwork);
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setLibraryLoading(false);
    }
  };

  const fetchProfileInfo = async () => {
    try {
      const status = await getSyncStatus();
      setSyncStatus(status);
    } catch (e) {
      console.error('Failed to load profile status:', e);
      setProfileError(e instanceof Error ? e.message : String(e));
    }
  };

  useEffect(() => {
    let active = true;
    if (!isNativeApp()) {
      return () => {
        active = false;
      };
    }

    Promise.all([getLibraryStorageSummary(), getArtworkCacheSummary()])
      .then(([storage, artwork]) => {
        if (!active) return;
        setStorageSummary(storage);
        setArtworkSummary(artwork);
      })
      .catch((err) => {
        if (active) setLibraryError(err instanceof Error ? err.message : String(err));
      });

    getSyncStatus()
      .then((status) => {
        if (active) setSyncStatus(status);
      })
      .catch((e) => {
        if (active) setProfileError(e instanceof Error ? e.message : String(e));
      });

    return () => {
      active = false;
    };
  }, []);

  const handleSaveConnectSettings = async (patch: {
    displayName?: string;
    acceptJoinRequests?: boolean;
    showConnectId?: boolean;
  }) => {
    setConnectSettingsBusy(true);
    setProfileError(null);
    try {
      const next = await connectUpdateAccountSettings(patch);
      if (next) onConnectHubChanged?.(next);
    } catch (err) {
      setProfileError(err instanceof Error ? err.message : String(err));
    } finally {
      setConnectSettingsBusy(false);
    }
  };

  const handleSettingsUnignore = async (ignoredAccountId: string) => {
    setConnectSettingsBusy(true);
    setProfileError(null);
    try {
      const next = await connectUnignoreGuest(ignoredAccountId);
      if (next) onConnectHubChanged?.(next);
    } catch (err) {
      setProfileError(err instanceof Error ? err.message : String(err));
    } finally {
      setConnectSettingsBusy(false);
    }
  };

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authEmail.trim() || !authPassword) return;
    const password = authPassword;
    setAuthPassword('');
    setIsAuthenticating(true);
    setProfileError(null);
    try {
      const status = await syncLogin(authEmail.trim(), password);
      setSyncStatus(status);
      onSyncConfigurationChanged(true);
      setProfileSuccessMsg('Signed in. Cloud sync and Cavia Connect are ready.');
      setTimeout(() => setProfileSuccessMsg(null), 3500);
    } catch (err) {
      setProfileError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsAuthenticating(false);
    }
  };

  const handleSignup = async () => {
    if (!authEmail.trim() || !authPassword) return;
    const password = authPassword;
    setAuthPassword('');
    setIsAuthenticating(true);
    setProfileError(null);
    try {
      const status = await syncSignup(authEmail.trim(), password);
      setSyncStatus(status);
      onSyncConfigurationChanged(true);
      setProfileSuccessMsg('Profile created. Cloud sync and Cavia Connect are ready.');
      setTimeout(() => setProfileSuccessMsg(null), 3500);
    } catch (err) {
      setProfileError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsAuthenticating(false);
    }
  };

  const handleLogout = async () => {
    try {
      await syncLogout();
      await fetchProfileInfo();
      onSyncConfigurationChanged(false);
    } catch (err) {
      setProfileError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleToggleSync = async (enabled: boolean) => {
    try {
      await toggleSync(enabled);
      await fetchProfileInfo();
      onSyncConfigurationChanged(enabled);
    } catch (err) {
      setProfileError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSyncNow = async () => {
    setIsSyncing(true);
    setProfileError(null);
    try {
      const result = await syncNow();
      if (result.success) {
        setProfileSuccessMsg(
          `Sync complete: ${result.pushedCount} pushed, ${result.pulledCount} pulled`
        );
        setTimeout(() => setProfileSuccessMsg(null), 4000);
      } else if (result.errorMessage) {
        setProfileError(result.errorMessage);
      }
      await fetchProfileInfo();
    } catch (err) {
      setProfileError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSyncing(false);
    }
  };

  const handleConfirmDeleteCloudData = async () => {
    setIsDeletingCloud(true);
    setProfileError(null);
    try {
      await deleteCloudData();
      setShowDeleteCloudModal(false);
      setProfileSuccessMsg('Cloud data deleted. Local library remains intact.');
      setTimeout(() => setProfileSuccessMsg(null), 4000);
      await fetchProfileInfo();
    } catch (err) {
      setProfileError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsDeletingCloud(false);
    }
  };

  const handleTogglePreloadFeature = (enabled: boolean) => {
    if (!onChangePreloadTarget) return;
    if (enabled) {
      onChangePreloadTarget(Math.max(1, Math.min(MAX_PRELOAD_TARGET, lastPreloadTarget || 10)));
    } else {
      if (preloadStatus?.targetCount && preloadStatus.targetCount > 0) {
        setLastPreloadTarget(preloadStatus.targetCount);
      }
      onChangePreloadTarget(0);
    }
  };

  const handleClearUnusedDownloads = async () => {
    setIsCleaning(true);
    setLibraryError(null);
    try {
      const result = await clearStreamCache(false);
      showLibraryNotice(
        `Removed ${result.deletedFiles} unused cached audio files (${formatBytes(result.reclaimedBytes)} freed).`
      );
      await fetchLibraryStorage();
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCleaning(false);
    }
  };

  const handleConfirmClearAllDownloads = async () => {
    setIsCleaning(true);
    setLibraryError(null);
    try {
      const result = await clearStreamCache(true);
      setShowClearAllDownloadsModal(false);
      showLibraryNotice(
        `Removed ${result.deletedFiles} downloaded audio files (${formatBytes(result.reclaimedBytes)} freed).`
      );
      await fetchLibraryStorage();
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCleaning(false);
    }
  };

  const handleConfirmCleanOrphanedArtwork = async () => {
    setIsCleaning(true);
    setLibraryError(null);
    try {
      const result = await cleanOrphanedArtwork();
      setCleanupResult(result);
      setShowCleanupModal(false);
      await fetchLibraryStorage();
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCleaning(false);
    }
  };

  const handleConfirmClearAllArtwork = async () => {
    setIsCleaning(true);
    setLibraryError(null);
    try {
      const result = await clearArtworkCache();
      setShowClearAllArtworkModal(false);
      showLibraryNotice(
        `Cleared ${result.deletedFiles} cached artwork files (${formatBytes(result.reclaimedBytes)} freed).`
      );
      await fetchLibraryStorage();
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCleaning(false);
    }
  };

  const handleRepairMissingArtwork = async () => {
    setIsRepairing(true);
    setLibraryError(null);
    try {
      const result = await recoverMissingArtwork();
      setRecoveryResult(result);
      await fetchLibraryStorage();
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsRepairing(false);
    }
  };

  const handleClearLyricsCache = async () => {
    setIsCleaning(true);
    setLibraryError(null);
    try {
      const removed = await clearLyricsCache();
      showLibraryNotice(`Cleared ${removed} cached lyrics entries.`);
      await fetchLibraryStorage();
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCleaning(false);
    }
  };

  const handleClearRecentHistory = async () => {
    setIsCleaning(true);
    setLibraryError(null);
    try {
      await clearRecentPlays();
      showLibraryNotice('Cleared recent listening history.');
      await fetchLibraryStorage();
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCleaning(false);
    }
  };

  const handleClearFinishedImports = async () => {
    setIsCleaning(true);
    setLibraryError(null);
    try {
      await clearCompletedImports();
      showLibraryNotice('Cleared completed playlist import records.');
    } catch (err) {
      setLibraryError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCleaning(false);
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
                    <p>Playback state is saved locally off the UI thread.</p>
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
            </div>
          </section>

          <section
            className={`settings-section ${activeSection === 'library' ? 'is-active' : ''}`}
            aria-labelledby="settings-library"
          >
            <h2 id="settings-library" className="settings-section-title">Library</h2>

            {librarySuccessMsg && (
              <div className="sync-alert sync-alert-success">
                <CheckCircle2 size={15} />
                <span>{librarySuccessMsg}</span>
              </div>
            )}

            {libraryError && (
              <div className="cavia-banner banner-error settings-inline-banner">
                <AlertTriangle size={16} className="text-error" />
                <div className="banner-content">
                  <strong>Operation error</strong>
                  <p>{libraryError}</p>
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

            {onChangePreloadTarget && (
              <div className="settings-card">
                <div className="card-head">
                  <DownloadCloud size={18} className="card-icon text-vermilion" />
                  <div>
                    <h3>Preload queue</h3>
                    <p>
                      Preload upcoming tracks sequentially so playback stays resilient through brief drops.
                    </p>
                  </div>
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label htmlFor="library-preload-toggle" className="setting-label">
                      Enable queue preload
                    </label>
                    <span className="setting-desc">
                      When turned off, upcoming streaming tracks are fetched only on demand and the Now Playing pill hides.
                    </span>
                  </div>
                  <input
                    id="library-preload-toggle"
                    type="checkbox"
                    className="cavia-toggle"
                    checked={Boolean(preloadStatus?.enabled && (preloadStatus?.targetCount ?? 0) > 0)}
                    disabled={!preloadStatus}
                    onChange={(e) => handleTogglePreloadFeature(e.target.checked)}
                  />
                </div>

                <div className="setting-status-line" style={{ marginTop: 12 }}>
                  <div className="queue-preload-stepper" aria-label="Queue preload window">
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

            <div className="settings-card">
              <div className="card-head">
                <HardDrive size={18} className="card-icon" />
                <div>
                  <h3>Downloaded audio &amp; stream cache</h3>
                  <p>Preloaded and streamed audio stored locally for instant playback and offline fallback.</p>
                </div>
              </div>

              <div className="sqlite-stats-grid">
                <div className="sqlite-stat-box">
                  <span className="box-label">DOWNLOADED AUDIO</span>
                  <strong className="box-val">
                    {libraryLoading ? 'Scanning…' : formatBytes(storageSummary?.streamCacheBytes ?? 0)}
                  </strong>
                  <span className="box-subval">{storageSummary?.streamCacheFiles ?? 0} files</span>
                </div>
                <div className="sqlite-stat-box">
                  <span className="box-label">QUEUED / PROTECTED</span>
                  <strong className="box-val text-ready">
                    {libraryLoading ? '…' : `${storageSummary?.streamProtectedFiles ?? 0} tracks`}
                  </strong>
                  <span className="box-subval">In active preload window</span>
                </div>
              </div>

              <div className="settings-button-row">
                <Button
                  variant="outline"
                  size="sm"
                  icon={<RefreshCw size={13} className={libraryLoading ? 'cavia-spin' : undefined} />}
                  onClick={fetchLibraryStorage}
                  disabled={libraryLoading || isCleaning || isRepairing}
                >
                  {libraryLoading ? 'Refreshing…' : 'Refresh'}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Trash2 size={13} />}
                  onClick={handleClearUnusedDownloads}
                  disabled={
                    libraryLoading ||
                    isCleaning ||
                    (storageSummary?.streamCacheFiles ?? 0) <= (storageSummary?.streamProtectedFiles ?? 0)
                  }
                >
                  Clear unused cache
                </Button>
                <Button
                  variant={(storageSummary?.streamCacheFiles ?? 0) > 0 ? 'vermilion-subtle' : 'outline'}
                  size="sm"
                  icon={<Trash2 size={13} />}
                  onClick={() => setShowClearAllDownloadsModal(true)}
                  disabled={libraryLoading || isCleaning || (storageSummary?.streamCacheFiles ?? 0) === 0}
                >
                  Remove all downloads
                </Button>
              </div>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <Database size={18} className="card-icon" />
                <div>
                  <h3>Artwork &amp; lyrics cache</h3>
                  <p>Cached cover art and synced lyrics stored on this device.</p>
                </div>
              </div>

              <div className="sqlite-stats-grid">
                <div className="sqlite-stat-box">
                  <span className="box-label">ARTWORK CACHE</span>
                  <strong className="box-val">
                    {libraryLoading ? 'Scanning…' : formatBytes(artworkSummary?.totalBytes ?? 0)}
                  </strong>
                  <span className="box-subval">
                    {artworkSummary?.totalFiles ?? 0} files ({artworkSummary?.orphanedFiles ?? 0} unused)
                  </span>
                </div>
                <div className="sqlite-stat-box">
                  <span className="box-label">CACHED LYRICS</span>
                  <strong className="box-val">
                    {libraryLoading ? '…' : `${storageSummary?.lyricsCacheEntries ?? 0} tracks`}
                  </strong>
                  <span className="box-subval">Refetched automatically if cleared</span>
                </div>
              </div>

              <div className="settings-button-row">
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Trash2 size={13} />}
                  onClick={() => setShowCleanupModal(true)}
                  disabled={libraryLoading || isCleaning || (artworkSummary?.orphanedFiles ?? 0) === 0}
                >
                  Clean orphaned artwork
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Trash2 size={13} />}
                  onClick={() => setShowClearAllArtworkModal(true)}
                  disabled={libraryLoading || isCleaning || (artworkSummary?.totalFiles ?? 0) === 0}
                >
                  Clear artwork cache
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Wrench size={13} />}
                  onClick={handleRepairMissingArtwork}
                  disabled={
                    libraryLoading ||
                    isCleaning ||
                    isRepairing ||
                    (artworkSummary?.missingReferencedFiles ?? 0) === 0
                  }
                >
                  {isRepairing ? 'Repairing…' : 'Repair missing artwork'}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Trash2 size={13} />}
                  onClick={handleClearLyricsCache}
                  disabled={libraryLoading || isCleaning || (storageSummary?.lyricsCacheEntries ?? 0) === 0}
                >
                  Clear lyrics cache
                </Button>
              </div>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <History size={18} className="card-icon" />
                <div>
                  <h3>Listening history &amp; imports</h3>
                  <p>Remove local recently-played records or completed playlist import tasks.</p>
                </div>
              </div>

              <div className="settings-button-row">
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Trash2 size={13} />}
                  onClick={handleClearRecentHistory}
                  disabled={libraryLoading || isCleaning || (storageSummary?.recentPlaysCount ?? 0) === 0}
                >
                  Clear recent history ({storageSummary?.recentPlaysCount ?? 0})
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Trash2 size={13} />}
                  onClick={handleClearFinishedImports}
                  disabled={libraryLoading || isCleaning}
                >
                  Clear finished imports
                </Button>
              </div>
            </div>
          </section>

          <section
            className={`settings-section ${activeSection === 'profile' ? 'is-active' : ''}`}
            aria-labelledby="settings-profile"
          >
            <h2 id="settings-profile" className="settings-section-title">Profile</h2>

            {profileSuccessMsg && (
              <div className="sync-alert sync-alert-success">
                <CheckCircle2 size={15} />
                <span>{profileSuccessMsg}</span>
              </div>
            )}

            {profileError && (
              <div className="sync-alert sync-alert-error">
                <AlertTriangle size={15} />
                <span>{profileError}</span>
              </div>
            )}

            <div className="settings-card">
              <div className="card-head">
                <User size={18} className="card-icon text-vermilion" />
                <div>
                  <h3>Cavia Profile</h3>
                  <p>
                    Sign in to synchronize playlists and likes across devices and unlock Cavia Connect collaborative sessions.
                  </p>
                </div>
              </div>

              {syncStatus && !syncStatus.backendConfigured && (
                <p className="text-muted-xs" role="status">
                  Online profile, cloud sync, and Connect services are unavailable in this build. Local library and offline playback continue working normally.
                </p>
              )}

              {syncStatus?.authenticated ? (
                <div className="settings-stack">
                  <div className="setting-status-line">
                    <Badge variant="ready" size="sm">SIGNED IN</Badge>
                    <span className="text-muted-xs font-mono">
                      {syncStatus.accountEmail || syncStatus.accountId}
                    </span>
                    <Badge variant="vermilion" size="sm">
                      {connectHub?.profile.connectId
                        ? `CONNECT ID: ${connectHub.profile.connectId}`
                        : 'SYNCING ID…'}
                    </Badge>
                  </div>

                  <div className="connect-search-row">
                    <input
                      type="text"
                      className="cavia-text-input"
                      value={connectDisplayNameDraft}
                      onChange={(e) => setConnectDisplayNameDraft(e.target.value)}
                      placeholder="Public Connect display name"
                      maxLength={64}
                    />
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={connectSettingsBusy || !connectDisplayNameDraft.trim()}
                      onClick={() =>
                        handleSaveConnectSettings({ displayName: connectDisplayNameDraft.trim() })
                      }
                    >
                      Save name
                    </Button>
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
                    <label htmlFor="profile-email">Email</label>
                    <input
                      id="profile-email"
                      type="email"
                      className="cavia-text-input"
                      placeholder="you@example.com"
                      value={authEmail}
                      onChange={(e) => setAuthEmail(e.target.value)}
                      disabled={isAuthenticating || (syncStatus != null && !syncStatus.backendConfigured)}
                      required
                    />
                  </div>

                  <div className="form-field">
                    <label htmlFor="profile-password">Password</label>
                    <input
                      id="profile-password"
                      type="password"
                      className="cavia-text-input"
                      placeholder="••••••••"
                      value={authPassword}
                      onChange={(e) => setAuthPassword(e.target.value)}
                      disabled={isAuthenticating || (syncStatus != null && !syncStatus.backendConfigured)}
                      required
                    />
                  </div>

                  <p className="text-muted-xs">
                    Signing in is optional. You can browse local music and play offline anytime without an account.
                  </p>

                  <div className="sync-auth-actions">
                    <Button
                      type="submit"
                      variant="primary"
                      size="sm"
                      disabled={
                        isAuthenticating ||
                        !authEmail.trim() ||
                        !authPassword ||
                        (syncStatus != null && !syncStatus.backendConfigured)
                      }
                    >
                      {isAuthenticating ? 'Signing in…' : 'Sign in'}
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={handleSignup}
                      disabled={
                        isAuthenticating ||
                        !authEmail.trim() ||
                        !authPassword ||
                        (syncStatus != null && !syncStatus.backendConfigured)
                      }
                    >
                      Create account
                    </Button>
                  </div>
                </form>
              )}
            </div>

            {syncStatus?.authenticated && (
              <>
                <div className="settings-card">
                  <div className="card-head">
                    <Cloud size={18} className="card-icon" />
                    <div>
                      <h3>Cloud synchronization</h3>
                      <p>Keep playlists and liked tracks synced across devices. Local audio files stay on your device.</p>
                    </div>
                  </div>

                  <div className="settings-stack">
                    <div className="setting-row">
                      <div className="setting-info">
                        <label htmlFor="cloud-sync-toggle" className="setting-label">
                          Enable cloud sync
                        </label>
                        <span className="setting-desc">
                          Automatically mirror playlist metadata and favorites across signed-in devices.
                        </span>
                      </div>
                      <input
                        id="cloud-sync-toggle"
                        type="checkbox"
                        className="cavia-toggle"
                        checked={syncStatus.syncEnabled}
                        onChange={(e) => handleToggleSync(e.target.checked)}
                        aria-label="Enable cloud synchronization"
                      />
                    </div>

                    <div className="sync-diagnostic-grid">
                      <div className="sync-diag-item">
                        <span className="sync-diag-label">LAST SYNCED</span>
                        <strong className="sync-diag-value">
                          {syncStatus.lastSyncedAt
                            ? new Date(syncStatus.lastSyncedAt).toLocaleString()
                            : 'Never'}
                        </strong>
                      </div>
                      <div className="sync-diag-item">
                        <span className="sync-diag-label">PENDING</span>
                        <strong className="sync-diag-value">{syncStatus.pendingOutboxCount} changes</strong>
                      </div>
                    </div>

                    {syncStatus.lastError && (
                      <div className="sync-alert sync-alert-error">
                        <AlertTriangle size={15} />
                        <span>{syncStatus.lastError}</span>
                      </div>
                    )}

                    <div className="settings-button-row">
                      <Button
                        variant="primary"
                        size="sm"
                        icon={<RefreshCw size={14} className={isSyncing ? 'animate-spin' : ''} />}
                        onClick={handleSyncNow}
                        disabled={isSyncing || !syncStatus.syncEnabled}
                      >
                        {isSyncing ? 'Synchronizing…' : 'Sync now'}
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setShowDeleteCloudModal(true)}
                        style={{ color: 'var(--status-error)', borderColor: 'rgba(223, 88, 73, 0.4)' }}
                      >
                        Delete cloud data
                      </Button>
                    </div>

                    <div className="sync-boundary-grid">
                      <div className="sync-boundary-box">
                        <strong className="sync-boundary-title ready">
                          <CheckCircle2 size={14} /> Synchronized
                        </strong>
                        <ul className="sync-boundary-list">
                          <li>Playlist titles, descriptions, and order</li>
                          <li>Track metadata snapshots &amp; liked tracks</li>
                          <li>Connect profile &amp; saved hosts</li>
                        </ul>
                      </div>

                      <div className="sync-boundary-box">
                        <strong className="sync-boundary-title warn">
                          <Lock size={14} /> Local only
                        </strong>
                        <ul className="sync-boundary-list">
                          <li>Audio files, file paths &amp; drive letters</li>
                          <li>Artwork &amp; stream caches</li>
                          <li>Playback queue &amp; volume</li>
                        </ul>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="settings-card">
                  <div className="card-head">
                    <Radio size={18} className="card-icon text-vermilion" />
                    <div>
                      <h3>Cavia Connect preferences</h3>
                      <p>
                        Separate controls for accepting guest requests and Now Playing / Bluetooth metadata display.
                      </p>
                    </div>
                  </div>

                  <div className="settings-stack">
                    <div className="setting-row">
                      <div className="setting-info">
                        <label htmlFor="connect-accept-requests" className="setting-label">
                          Accept Join Requests
                        </label>
                        <span className="setting-desc">
                          Disabled state blocks new guest &amp; saved-host requests without disconnecting current guests.
                        </span>
                      </div>
                      <input
                        id="connect-accept-requests"
                        type="checkbox"
                        className="cavia-toggle"
                        checked={Boolean(connectHub?.profile.acceptJoinRequests)}
                        disabled={connectSettingsBusy}
                        onChange={(e) =>
                          handleSaveConnectSettings({ acceptJoinRequests: e.target.checked })
                        }
                      />
                    </div>

                    <div className="setting-row">
                      <div className="setting-info">
                        <label htmlFor="connect-show-id-meta" className="setting-label">
                          Show Connect ID in Now Playing &amp; Bluetooth metadata
                        </label>
                        <span className="setting-desc">
                          Controls Windows SMTC / Android Media3 metadata visibility only; saved hosts can still find you while hosting.
                        </span>
                      </div>
                      <input
                        id="connect-show-id-meta"
                        type="checkbox"
                        className="cavia-toggle"
                        checked={Boolean(connectHub?.profile.showConnectId)}
                        disabled={connectSettingsBusy}
                        onChange={(e) =>
                          handleSaveConnectSettings({ showConnectId: e.target.checked })
                        }
                      />
                    </div>
                  </div>
                </div>

                <div className="settings-card">
                  <div className="card-head">
                    <ShieldBan size={18} className="card-icon" />
                    <div>
                      <h3>Ignored Accounts</h3>
                      <p>
                        Accounts ignored for 24 hours cannot send new join requests to your sessions until the timer expires or you unignore them.
                      </p>
                    </div>
                  </div>

                  {(connectHub?.ignoredAccounts.length ?? 0) === 0 ? (
                    <p className="text-muted-xs">No accounts are currently ignored.</p>
                  ) : (
                    <ul className="connect-pick-list">
                      {(connectHub?.ignoredAccounts ?? []).map((ign) => (
                        <li key={ign.ignoredAccountId} className="connect-pick-row">
                          <div className="queue-item-meta">
                            <strong className="queue-item-title">{ign.displayName}</strong>
                            <span className="queue-item-artist">
                              ID: {ign.connectId} · Expires {new Date(ign.expiresAt).toLocaleString()}
                            </span>
                          </div>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={connectSettingsBusy}
                            onClick={() => handleSettingsUnignore(ign.ignoredAccountId)}
                          >
                            Unignore
                          </Button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </>
            )}
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
            <Button variant="primary" size="sm" icon={<Trash2 size={14} />} onClick={handleConfirmCleanOrphanedArtwork} disabled={isCleaning}>
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
        isOpen={showClearAllArtworkModal}
        onClose={() => setShowClearAllArtworkModal(false)}
        title="Clear all artwork cache"
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setShowClearAllArtworkModal(false)} disabled={isCleaning}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" icon={<Trash2 size={14} />} onClick={handleConfirmClearAllArtwork} disabled={isCleaning}>
              {isCleaning ? 'Clearing…' : 'Clear artwork'}
            </Button>
          </div>
        }
      >
        <div className="modal-stack">
          <p className="modal-text">
            Deletes all cached cover art files ({formatBytes(artworkSummary?.totalBytes ?? 0)}). Embedded artwork on local audio files can be repaired again anytime.
          </p>
        </div>
      </Modal>

      <Modal
        isOpen={showClearAllDownloadsModal}
        onClose={() => setShowClearAllDownloadsModal(false)}
        title="Remove all downloaded audio"
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setShowClearAllDownloadsModal(false)} disabled={isCleaning}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" icon={<Trash2 size={14} />} onClick={handleConfirmClearAllDownloads} disabled={isCleaning}>
              {isCleaning ? 'Removing…' : 'Remove downloads'}
            </Button>
          </div>
        }
      >
        <div className="modal-stack">
          <p className="modal-text">
            Deletes all preloaded and cached streaming audio files ({formatBytes(storageSummary?.streamCacheBytes ?? 0)}) from this device. Your local music files remain untouched.
          </p>
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
