import { useCallback, useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  Album as AlbumIcon,
  AudioWaveform,
  CheckCircle2,
  ChevronDown,
  CircleHelp,
  Disc3,
  Heart,
  Home,
  Library,
  ListMusic,
  Loader2,
  Search,
  Settings2,
  ShieldCheck,
  X,
  XCircle,
} from 'lucide-react';
import {
  getLibrarySummary,
  getPlatformHealth,
  isNativeApp,
  type LibrarySummary,
  type PlatformHealth,
} from '../lib/platform';
import {
  cancelScan,
  createPlaylist,
  deletePlaylist,
  getAlbums,
  getArtists,
  getLikedTracks,
  getPlaylists,
  getTracks,
  pickMusicFolder,
  scanMusicDirectory,
  toggleLikedTrack,
  type ScanProgressEvent,
} from '../lib/library';
import {
  clearQueue,
  getPlaybackState,
  listenPlaybackPosition,
  listenPlaybackState,
  nextTrack,
  playPause,
  playTrack,
  previousTrack,
  removeFromQueue,
  removeFromQueueByIndex,
  reorderQueue,
  retryAudioDevice,
  retryShutdown,
  forceExit,
  seek,
  setPlaybackSpeed,
  setQueue,
  setRepeatMode,
  setShuffle,
  setVolume,
  toggleMute,
} from '../lib/playback';
import type { Album, Artist, Page, PlaybackState, Playlist, Track } from '../lib/types';
import { IconButton } from '../components/IconButton';
import { Button } from '../components/Button';
import { Modal } from '../components/Modal';
import { LibraryView } from '../features/library/LibraryView';
import { TracksView } from '../features/library/TracksView';
import { PlaylistsView } from '../features/library/PlaylistsView';
import { AlbumsView } from '../features/library/AlbumsView';
import { ArtistsView } from '../features/library/ArtistsView';
import { LikedTracksView } from '../features/library/LikedTracksView';
import { SearchView } from '../features/search/SearchView';
import { SettingsView } from '../features/settings/SettingsView';
import { PlayerDock } from '../features/player/PlayerDock';
import { ExpandedPlayer } from '../features/player/ExpandedPlayer';
import { QueueDrawer } from '../features/player/QueueDrawer';
import { LyricsDrawer } from '../features/player/LyricsDrawer';

const pageLabels: Record<Page, string> = {
  library: 'Personal Archive',
  tracks: 'Tracks',
  playlists: 'Playlists',
  albums: 'Albums',
  artists: 'Artists',
  liked: 'Liked Songs',
  search: 'Search & Discovery',
  settings: 'Settings & System',
};

const getInitialPage = (): Page => {
  const hash = window.location.hash.replace('#', '') as Page;
  if (['library', 'tracks', 'playlists', 'albums', 'artists', 'liked', 'search', 'settings'].includes(hash)) {
    return hash;
  }
  const params = new URLSearchParams(window.location.search);
  const v = params.get('view') as Page;
  if (['library', 'tracks', 'playlists', 'albums', 'artists', 'liked', 'search', 'settings'].includes(v)) {
    return v;
  }
  return 'library';
};

export function App() {
  const [page, setPage] = useState<Page>(getInitialPage);
  const [health, setHealth] = useState<PlatformHealth | null>(null);
  const [summary, setSummary] = useState<LibrarySummary | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);

  // Real library domain state (owned by Rust SQLite)
  const [tracks, setTracks] = useState<Track[]>([]);
  const [albums, setAlbums] = useState<Album[]>([]);
  const [artists, setArtists] = useState<Artist[]>([]);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [likedTracks, setLikedTracks] = useState<Track[]>([]);

  // Scan & Import State
  const [isScanning, setIsScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState<ScanProgressEvent | null>(null);
  const [scanNotification, setScanNotification] = useState<string | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [playbackError, setPlaybackError] = useState<string | null>(null);

  const showPlaybackError = useCallback((message: string) => {
    setPlaybackError(message);
    setPlayback((prev) => ({ ...prev, errorMessage: message }));
    setTimeout(() => setPlaybackError(null), 6000);
  }, []);

  // UI Drawer & Modal States
  const urlParams = useMemo(() => new URLSearchParams(window.location.search), []);
  const [isExpandedPlayerOpen, setIsExpandedPlayerOpen] = useState(urlParams.get('expanded') === 'true');
  const [isQueueOpen, setIsQueueOpen] = useState(urlParams.get('queue') === 'true');
  const [isLyricsOpen, setIsLyricsOpen] = useState(urlParams.get('lyrics') === 'true');
  const [shutdownError, setShutdownError] = useState<string | null>(null);
  const [isRetryingShutdown, setIsRetryingShutdown] = useState(false);

  // Appearance preferences
  const [density, setDensity] = useState<'standard' | 'compact'>('standard');

  // Playback state (honest initial state: audio backend is standby for Milestone 3)
  const [playback, setPlayback] = useState<PlaybackState>({
    status: 'idle',
    currentTrack: null,
    positionMs: 0,
    durationMs: 0,
    volume: 85,
    isMuted: false,
    repeatMode: 'off',
    isShuffle: false,
    playbackSpeed: 1.0,
    queue: [],
  });

  // Reusable loader for library collections from Rust SQLite
  const refreshLibrary = useCallback(async () => {
    if (!isNativeApp()) return;
    try {
      const [nextSummary, nextTracks, nextAlbums, nextArtists, nextPlaylists, nextLiked] =
        await Promise.all([
          getLibrarySummary(),
          getTracks(),
          getAlbums(),
          getArtists(),
          getPlaylists(),
          getLikedTracks(),
        ]);
      setSummary(nextSummary);
      setTracks(nextTracks);
      setAlbums(nextAlbums);
      setArtists(nextArtists);
      setPlaylists(nextPlaylists);
      setLikedTracks(nextLiked);
    } catch (err) {
      console.error('Error refreshing library from SQLite:', err);
    }
  }, []);

  // Initial load
  useEffect(() => {
    let active = true;
    if (!isNativeApp()) return;

    getPlatformHealth()
      .then((nextHealth) => {
        if (!active) return;
        setHealth(nextHealth);
      })
      .catch((error: unknown) => {
        if (!active) return;
        setConnectionError(error instanceof Error ? error.message : String(error));
      });

    refreshLibrary();

    return () => {
      active = false;
    };
  }, [refreshLibrary]);

  // Listen to scan progress events emitted from Rust core
  useEffect(() => {
    if (!isNativeApp()) return;
    let unlisten: (() => void) | undefined;

    listen<ScanProgressEvent>('library:scan-progress', (event) => {
      setScanProgress(event.payload);
      if (event.payload.phase === 'completed' || event.payload.phase === 'cancelled') {
        setIsScanning(false);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Synchronize playback state and live position with Rust native engine
  useEffect(() => {
    if (!isNativeApp()) return;
    let unlistenState: (() => void) | undefined;
    let unlistenPos: (() => void) | undefined;

    getPlaybackState()
      .then((state) => {
        if (state) {
          setPlayback((prev) => ({
            ...prev,
            status: state.status,
            currentTrack: state.currentTrack,
            positionMs: state.positionMs,
            durationMs: state.durationMs,
            volume: state.volume,
            isMuted: state.isMuted,
            repeatMode: state.repeatMode,
            isShuffle: state.isShuffle,
            playbackSpeed: state.playbackSpeed,
            queue: state.queue,
            errorMessage: state.errorMessage,
            generation: state.generation,
          }));
        }
      })
      .catch((err) => console.error('Failed to get initial playback state:', err));

    listenPlaybackState((snap) => {
      setPlayback((prev) => {
        if (snap.generation !== undefined && prev.generation !== undefined && snap.generation < prev.generation) {
          return prev;
        }
        return {
          ...prev,
          status: snap.status,
          currentTrack: snap.currentTrack,
          positionMs: snap.positionMs,
          durationMs: snap.durationMs,
          volume: snap.volume,
          isMuted: snap.isMuted,
          repeatMode: snap.repeatMode,
          isShuffle: snap.isShuffle,
          playbackSpeed: snap.playbackSpeed,
          queue: snap.queue,
          errorMessage: snap.errorMessage,
          generation: snap.generation,
        };
      });
    }).then((fn) => {
      unlistenState = fn;
    });

    listenPlaybackPosition((pos) => {
      setPlayback((prev) => {
        // Discard stale position events from prior generations, previous tracks, or idle player
        if (pos.generation !== undefined && prev.generation !== undefined && pos.generation < prev.generation) {
          return prev;
        }
        if (pos.trackId && prev.currentTrack?.id && pos.trackId !== prev.currentTrack.id) {
          return prev;
        }
        if (!prev.currentTrack || prev.status === 'stopped' || prev.status === 'idle') {
          return prev;
        }
        return {
          ...prev,
          positionMs: pos.positionMs,
          durationMs: pos.durationMs > 0 ? pos.durationMs : prev.durationMs,
        };
      });
    }).then((fn) => {
      unlistenPos = fn;
    });

    let unlistenShutdown: (() => void) | undefined;
    listen<{ error: string }>('app:shutdown-error', (event) => {
      setShutdownError(event.payload.error);
    }).then((fn) => {
      unlistenShutdown = fn;
    });

    return () => {
      if (unlistenState) unlistenState();
      if (unlistenPos) unlistenPos();
      if (unlistenShutdown) unlistenShutdown();
    };
  }, []);

  const handleRetryShutdown = async () => {
    try {
      setIsRetryingShutdown(true);
      await retryShutdown();
      setShutdownError(null);
    } catch (err) {
      setShutdownError(String(err));
    } finally {
      setIsRetryingShutdown(false);
    }
  };

  const handleForceExit = async () => {
    try {
      await forceExit();
    } catch (err) {
      console.error('Failed to force exit:', err);
    }
  };

  const handleImportFolder = async () => {
    try {
      setScanError(null);
      const selected = await pickMusicFolder();
      if (!selected) return;

      setIsScanning(true);
      setScanProgress({
        phase: 'discovering',
        currentFile: selected,
        discoveredCount: 0,
        processedCount: 0,
        newTracksCount: 0,
        duplicatesCount: 0,
        errorsCount: 0,
      });

      const result = await scanMusicDirectory(selected);
      await refreshLibrary();
      setIsScanning(false);

      if (result.cancelled) {
        setScanNotification(`Import cancelled. Added ${result.newTracks} tracks.`);
      } else {
        setScanNotification(
          `Scan complete: ${result.newTracks} new recordings indexed (${result.duplicates} existing/duplicates skipped).`
        );
      }
      setTimeout(() => setScanNotification(null), 6000);
    } catch (err) {
      setIsScanning(false);
      setScanError(err instanceof Error ? err.message : String(err));
      setTimeout(() => setScanError(null), 8000);
    }
  };

  const handleCancelScan = async () => {
    try {
      await cancelScan();
    } catch (err) {
      console.error('Error cancelling scan:', err);
    }
  };

  const handleToggleLike = async (trackId: string) => {
    try {
      const isLiked = await toggleLikedTrack(trackId);
      setTracks((prev) =>
        prev.map((t) => (t.id === trackId ? { ...t, isLiked } : t))
      );
      await refreshLibrary();
    } catch (err) {
      console.error('Error toggling like:', err);
    }
  };

  const handleCreatePlaylist = async (name: string, description: string) => {
    try {
      const created = await createPlaylist(name, description);
      setPlaylists((prev) => [created, ...prev]);
      await refreshLibrary();
    } catch (err) {
      console.error('Error creating playlist in SQLite:', err);
    }
  };

  const handleDeletePlaylist = async (playlistId: string) => {
    try {
      await deletePlaylist(playlistId);
      setPlaylists((prev) => prev.filter((p) => p.id !== playlistId));
      await refreshLibrary();
    } catch (err) {
      console.error('Error deleting playlist from SQLite:', err);
    }
  };

  // Playback transport handlers (native Rust backend authoritative)
  const handleTogglePlay = async () => {
    try {
      await playPause();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Play/Pause failed:', msg);
      showPlaybackError(`Playback failed: ${msg}`);
    }
  };

  const handleRetryDevice = async () => {
    try {
      await retryAudioDevice();
      const nextHealth = await getPlatformHealth();
      setHealth(nextHealth);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Retry audio device failed:', msg);
      showPlaybackError(`Device retry failed: ${msg}`);
    }
  };

  const handlePlayTrack = async (trackId: string) => {
    try {
      await setQueue(tracks.map((t) => t.id), trackId);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Play track failed:', msg);
      showPlaybackError(`Playback failed: ${msg}`);
    }
  };

  const handlePlayAll = async () => {
    try {
      if (tracks.length > 0) {
        await setQueue(tracks.map((t) => t.id));
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Play all failed:', msg);
      showPlaybackError(`Playback failed: ${msg}`);
    }
  };

  const handleShuffleAll = async () => {
    try {
      if (tracks.length > 0) {
        await setShuffle(true);
        await setQueue(tracks.map((t) => t.id));
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Shuffle all failed:', msg);
      showPlaybackError(`Shuffle all failed: ${msg}`);
    }
  };

  const handlePlayLikedTrack = async (trackId: string) => {
    try {
      await setQueue(likedTracks.map((t) => t.id), trackId);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Play liked track failed:', msg);
      showPlaybackError(`Playback failed: ${msg}`);
    }
  };

  const handlePlayAllLiked = async () => {
    try {
      if (likedTracks.length > 0) {
        await setQueue(likedTracks.map((t) => t.id));
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Play all liked failed:', msg);
      showPlaybackError(`Playback failed: ${msg}`);
    }
  };

  const handleShuffleAllLiked = async () => {
    try {
      if (likedTracks.length > 0) {
        await setShuffle(true);
        await setQueue(likedTracks.map((t) => t.id));
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Shuffle all liked failed:', msg);
      showPlaybackError(`Shuffle all liked failed: ${msg}`);
    }
  };

  const handlePlayPlaylistTracks = async (playlistTracks: Track[], startIndex = 0, shuffle = false) => {
    try {
      if (playlistTracks.length > 0) {
        if (shuffle) await setShuffle(true);
        const startId = playlistTracks[startIndex]?.id;
        await setQueue(playlistTracks.map((t) => t.id), startId);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Play playlist tracks failed:', msg);
      showPlaybackError(`Playback failed: ${msg}`);
    }
  };

  const handleNext = async () => {
    try {
      await nextTrack();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Next track failed:', msg);
      showPlaybackError(`Next track failed: ${msg}`);
    }
  };

  const handlePrevious = async () => {
    try {
      await previousTrack();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Previous track failed:', msg);
      showPlaybackError(`Previous track failed: ${msg}`);
    }
  };

  const handleSeek = async (positionMs: number) => {
    try {
      await seek(positionMs, playback.generation);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Seek failed:', msg);
      showPlaybackError(`Seek failed: ${msg}`);
    }
  };

  const handleVolume = async (vol: number) => {
    try {
      await setVolume(vol);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Set volume failed:', msg);
      showPlaybackError(`Volume change failed: ${msg}`);
    }
  };

  const handleToggleMute = async () => {
    try {
      await toggleMute();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Toggle mute failed:', msg);
      showPlaybackError(`Toggle mute failed: ${msg}`);
    }
  };

  const handleCycleRepeat = async () => {
    try {
      const next = playback.repeatMode === 'off' ? 'all' : playback.repeatMode === 'all' ? 'one' : 'off';
      await setRepeatMode(next);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Cycle repeat failed:', msg);
      showPlaybackError(`Repeat mode change failed: ${msg}`);
    }
  };

  const handleToggleShuffle = async () => {
    try {
      await setShuffle(!playback.isShuffle);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Toggle shuffle failed:', msg);
      showPlaybackError(`Shuffle toggle failed: ${msg}`);
    }
  };

  const handleCycleSpeed = async () => {
    try {
      const speeds = [1.0, 1.25, 1.5, 2.0, 0.75];
      const curIdx = speeds.indexOf(playback.playbackSpeed);
      const nextSpeed = speeds[(curIdx + 1) % speeds.length];
      await setPlaybackSpeed(nextSpeed);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Set speed failed:', msg);
      showPlaybackError(`Playback speed change failed: ${msg}`);
    }
  };

  const handleClearQueue = async () => {
    try {
      await clearQueue();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Clear queue failed:', msg);
      showPlaybackError(`Clear queue failed: ${msg}`);
    }
  };

  const handleRemoveQueueTrack = async (trackId: string, index?: number) => {
    try {
      if (typeof index === 'number') {
        await removeFromQueueByIndex(index);
      } else {
        await removeFromQueue(trackId);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Remove from queue failed:', msg);
      showPlaybackError(`Remove track failed: ${msg}`);
    }
  };

  const handleReorderQueue = async (fromIndex: number, toIndex: number) => {
    try {
      await reorderQueue(fromIndex, toIndex);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Reorder queue failed:', msg);
      showPlaybackError(`Reorder queue failed: ${msg}`);
    }
  };

  const handleSelectQueueTrack = async (trackId: string) => {
    try {
      await playTrack(trackId);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Select queue track failed:', msg);
      showPlaybackError(`Playback failed: ${msg}`);
    }
  };

  const isBackendReady = health?.audio_backend === 'ready';

  const statusLabel = useMemo(() => {
    if (!isNativeApp()) return 'DESIGN PREVIEW';
    if (connectionError) return 'CORE UNAVAILABLE';
    if (!health) return 'CONNECTING';
    if (!health.database_connected) return 'DATABASE OFFLINE';
    if (health.audio_backend === 'device_unavailable') return 'NO AUDIO DEVICE';
    if (health.audio_backend === 'not_integrated') return 'BACKEND NOT INTEGRATED';
    if (health.audio_backend === 'error') return 'AUDIO ERROR';
    if (health.audio_backend === 'unsupported') return 'UNSUPPORTED PLATFORM';
    return 'LOCAL CORE READY';
  }, [connectionError, health]);

  const nav = [
    { label: 'Archive', page: 'library' as const, icon: Library },
    { label: 'Search', page: 'search' as const, icon: Search },
  ];

  const collections = [
    { label: 'Tracks', page: 'tracks' as const, icon: Disc3, count: summary?.tracks ?? tracks.length },
    { label: 'Playlists', page: 'playlists' as const, icon: ListMusic, count: playlists.length },
    { label: 'Albums', page: 'albums' as const, icon: AlbumIcon, count: albums.length },
    { label: 'Artists', page: 'artists' as const, icon: AudioWaveform, count: artists.length },
    { label: 'Liked Tracks', page: 'liked' as const, icon: Heart, count: summary?.liked_tracks ?? likedTracks.length },
  ];

  return (
    <div className={`cavia-app-frame ${density === 'compact' ? 'density-compact' : ''} show-kanji`}>
      {/* Desktop Sidebar Navigation */}
      <aside className="cavia-sidebar" aria-label="Main Navigation">
        <div className="brand-lockup">
          <div className="brand-mark" aria-hidden="true">
            <AudioWaveform size={22} strokeWidth={1.8} />
          </div>
          <div className="brand-copy">
            <span className="brand-name">
              cavia<span>.</span>
            </span>
            <span className="brand-edition">MUSIC / 02</span>
          </div>
        </div>

        <div className="sidebar-scrollable-content">
          <div className="nav-group">
            <div className="nav-caption">WORKSPACE</div>
            {nav.map((item) => {
              const Icon = item.icon;
              const isActive = page === item.page;
              return (
                <button
                  key={item.page}
                  type="button"
                  className={`nav-item ${isActive ? 'active' : ''}`}
                  onClick={() => setPage(item.page)}
                >
                  <Icon size={17} strokeWidth={1.8} />
                  <span>{item.label}</span>
                  {isActive && <span className="active-marker" />}
                </button>
              );
            })}
          </div>

          <div className="nav-group collection-group">
            <div className="nav-caption nav-caption-row">
              <span>YOUR COLLECTION</span>
              <span className="nav-count">{summary?.tracks ?? tracks.length}</span>
            </div>
            {collections.map((item) => {
              const Icon = item.icon;
              const isActive = page === item.page;
              return (
                <button
                  key={item.page}
                  type="button"
                  className={`nav-item ${isActive ? 'active' : ''}`}
                  onClick={() => setPage(item.page)}
                >
                  <Icon size={17} strokeWidth={1.8} />
                  <span>{item.label}</span>
                  {item.count !== undefined && item.count > 0 && (
                    <span className="nav-badge-count">{item.count}</span>
                  )}
                  {isActive && <span className="active-marker" />}
                </button>
              );
            })}
          </div>
        </div>

        <div className="sidebar-bottom-tray">
          <div className="integrity-card">
            <div className="integrity-icon" aria-hidden="true">
              <ShieldCheck size={16} />
            </div>
            <div className="integrity-copy">
              <strong>Local-first by design</strong>
              <span>Your library stays yours.</span>
            </div>
          </div>

          <button
            type="button"
            className={`nav-item ${page === 'settings' ? 'active' : ''}`}
            onClick={() => setPage('settings')}
          >
            <Settings2 size={17} strokeWidth={1.8} />
            <span>Settings</span>
            {page === 'settings' && <span className="active-marker" />}
          </button>

          <div className="sidebar-version-tag">
            CAVIA MUSIC <span>2.0.0 FOUNDATION</span>
          </div>
        </div>
      </aside>

      {/* Main Content Shell */}
      <main className="cavia-main-shell">
        <header className="cavia-topbar" role="banner">
          <div className="breadcrumbs">
            <span className="crumb-brand">CAVIA</span>
            <span className="crumb-slash">/</span>
            <strong className="crumb-current">{pageLabels[page].toUpperCase()}</strong>
          </div>

          <div className="topbar-actions">
            <div
              className={`core-status-pill ${connectionError ? 'status-error' : ''}`}
              title="Native SQLite Core Connection State"
            >
              <span className="status-dot-pulse" />
              <span>{statusLabel}</span>
            </div>

            <span className="topbar-divider" />

            <IconButton
              label="Help and project status"
              icon={<CircleHelp size={17} />}
              disabled
              title="Documentation is in docs/architecture.md"
            />

            <button
              type="button"
              className="profile-button"
              aria-label="Guest session profile"
              title="Local session · No account required"
            >
              <span className="profile-avatar">C</span>
              <span className="profile-name">LOCAL</span>
              <ChevronDown size={13} />
            </button>
          </div>
        </header>

        {/* Global Scan Progress Banner (when scanner is active) */}
        {isScanning && scanProgress && (
          <div className="cavia-scan-banner" role="status" aria-live="polite">
            <div className="scan-banner-header">
              <div className="scan-banner-info">
                <Loader2 size={16} className="spin-accent text-vermilion" />
                <strong className="scan-title">
                  {scanProgress.phase === 'discovering'
                    ? 'Discovering audio files...'
                    : `Indexing collection (${scanProgress.processedCount} / ${scanProgress.discoveredCount})`}
                </strong>
                <span className="scan-meta-pill">
                  {scanProgress.newTracksCount} new · {scanProgress.duplicatesCount} skipped
                </span>
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={handleCancelScan}
                className="scan-cancel-btn"
              >
                Cancel Scan
              </Button>
            </div>

            {scanProgress.discoveredCount > 0 && (
              <div className="scan-progress-bar-track">
                <div
                  className="scan-progress-bar-fill"
                  style={{
                    width: `${Math.min(
                      100,
                      Math.max(4, (scanProgress.processedCount / scanProgress.discoveredCount) * 100)
                    )}%`,
                  }}
                />
              </div>
            )}

            {scanProgress.currentFile && (
              <div className="scan-current-file">
                <code>{scanProgress.currentFile}</code>
              </div>
            )}
          </div>
        )}

        {/* Scan Success Toast */}
        {scanNotification && (
          <div className="cavia-toast toast-success" role="alert">
            <CheckCircle2 size={16} className="text-ready" />
            <span>{scanNotification}</span>
            <button
              type="button"
              className="toast-close"
              onClick={() => setScanNotification(null)}
              aria-label="Dismiss notification"
            >
              <X size={14} />
            </button>
          </div>
        )}

        {/* Scan Error Toast */}
        {scanError && (
          <div className="cavia-toast toast-error" role="alert">
            <XCircle size={16} className="text-danger" />
            <span>{scanError}</span>
            <button
              type="button"
              className="toast-close"
              onClick={() => setScanError(null)}
              aria-label="Dismiss error"
            >
              <X size={14} />
            </button>
          </div>
        )}

        {/* Playback Error Toast */}
        {playbackError && (
          <div className="cavia-toast toast-error" role="alert">
            <XCircle size={16} className="text-danger" />
            <span>{playbackError}</span>
            <button
              type="button"
              className="toast-close"
              onClick={() => setPlaybackError(null)}
              aria-label="Dismiss error"
            >
              <X size={14} />
            </button>
          </div>
        )}

        {/* Dynamic Page Scroll Area */}
        <div className="cavia-page-scroll" key={page} tabIndex={-1}>
          {page === 'library' && (
            <LibraryView
              summary={summary}
              health={health}
              connectionError={connectionError}
              playlists={playlists}
              onNavigate={setPage}
              onOpenCreatePlaylist={() => setPage('playlists')}
              onImportFolder={handleImportFolder}
              isScanning={isScanning}
            />
          )}

          {page === 'tracks' && (
            <TracksView
              tracks={tracks}
              isBackendReady={isBackendReady}
              onToggleLike={handleToggleLike}
              onImportFolder={handleImportFolder}
              isScanning={isScanning}
              onPlayTrack={handlePlayTrack}
              onPlayAll={handlePlayAll}
              onShuffleAll={handleShuffleAll}
              currentTrackId={playback.currentTrack?.id}
              isPlaying={playback.status === 'playing'}
            />
          )}

          {page === 'playlists' && (
            <PlaylistsView
              playlists={playlists}
              onCreatePlaylist={handleCreatePlaylist}
              onDeletePlaylist={handleDeletePlaylist}
              isBackendReady={isBackendReady}
              onPlayTracks={handlePlayPlaylistTracks}
            />
          )}

          {page === 'albums' && (
            <AlbumsView
              albums={albums}
              onImportFolder={handleImportFolder}
              isScanning={isScanning}
            />
          )}

          {page === 'artists' && (
            <ArtistsView
              artists={artists}
              onImportFolder={handleImportFolder}
              isScanning={isScanning}
            />
          )}

          {page === 'liked' && (
            <LikedTracksView
              tracks={likedTracks}
              isBackendReady={isBackendReady}
              onToggleLike={handleToggleLike}
              onPlayTrack={handlePlayLikedTrack}
              onPlayAll={handlePlayAllLiked}
              onShuffleAll={handleShuffleAllLiked}
              currentTrackId={playback.currentTrack?.id}
              isPlaying={playback.status === 'playing'}
            />
          )}

          {page === 'search' && <SearchView />}

          {page === 'settings' && (
            <SettingsView
              health={health}
              density={density}
              onDensityChange={setDensity}
            />
          )}
        </div>

        {/* Persistent Bottom Player Dock */}
        <PlayerDock
          playback={playback}
          isBackendReady={isBackendReady}
          audioBackendStatus={health?.audio_backend}
          onRetryDevice={handleRetryDevice}
          onOpenExpanded={() => setIsExpandedPlayerOpen(true)}
          onOpenQueue={() => setIsQueueOpen(true)}
          onOpenLyrics={() => setIsLyricsOpen(true)}
          onTogglePlay={handleTogglePlay}
          onNext={handleNext}
          onPrevious={handlePrevious}
          onSeek={handleSeek}
          onVolumeChange={handleVolume}
          onToggleMute={handleToggleMute}
          onToggleShuffle={handleToggleShuffle}
          onCycleRepeat={handleCycleRepeat}
          onCycleSpeed={handleCycleSpeed}
          onToggleLike={handleToggleLike}
        />
      </main>

      {/* Expanded Full-Screen Player Modal */}
      <ExpandedPlayer
        isOpen={isExpandedPlayerOpen}
        onClose={() => setIsExpandedPlayerOpen(false)}
        playback={playback}
        isBackendReady={isBackendReady}
        audioBackendStatus={health?.audio_backend}
        onOpenQueue={() => {
          setIsExpandedPlayerOpen(false);
          setIsQueueOpen(true);
        }}
        onOpenLyrics={() => {
          setIsExpandedPlayerOpen(false);
          setIsLyricsOpen(true);
        }}
        onTogglePlay={handleTogglePlay}
        onNext={handleNext}
        onPrevious={handlePrevious}
        onSeek={handleSeek}
        onVolumeChange={handleVolume}
        onToggleMute={handleToggleMute}
        onToggleShuffle={handleToggleShuffle}
        onCycleRepeat={handleCycleRepeat}
        onCycleSpeed={handleCycleSpeed}
        onToggleLike={handleToggleLike}
      />

      {/* Slide-over Queue Drawer */}
      <QueueDrawer
        isOpen={isQueueOpen}
        onClose={() => setIsQueueOpen(false)}
        currentTrack={playback.currentTrack}
        queue={playback.queue}
        isPlaying={playback.status === 'playing'}
        status={playback.status}
        isBackendReady={isBackendReady}
        onClearQueue={handleClearQueue}
        onRemoveTrack={handleRemoveQueueTrack}
        onSelectTrack={handleSelectQueueTrack}
        onShuffleQueue={handleToggleShuffle}
        onReorderQueue={handleReorderQueue}
      />

      {/* Slide-over Lyrics Drawer */}
      <LyricsDrawer
        isOpen={isLyricsOpen}
        onClose={() => setIsLyricsOpen(false)}
        currentTrack={playback.currentTrack}
      />

      {/* Mobile Bottom Navigation Bar */}
      <nav className="cavia-mobile-nav" aria-label="Mobile Navigation">
        {[
          { page: 'library' as const, icon: Home, label: 'Archive' },
          { page: 'search' as const, icon: Search, label: 'Search' },
          { page: 'playlists' as const, icon: ListMusic, label: 'Playlists' },
          { page: 'liked' as const, icon: Heart, label: 'Liked' },
          { page: 'settings' as const, icon: Settings2, label: 'Settings' },
        ].map((item) => {
          const Icon = item.icon;
          const isActive = page === item.page;
          return (
            <button
              key={item.page}
              type="button"
              className={`mobile-nav-btn ${isActive ? 'active' : ''}`}
              onClick={() => setPage(item.page)}
            >
              <Icon size={19} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      {shutdownError && (
        <Modal
          isOpen={true}
          onClose={() => setShutdownError(null)}
          title="Shutdown Persistence Alert"
        >
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
            <p style={{ color: 'var(--cavia-ivory-dim)', fontSize: '14px', lineHeight: '1.5' }}>
              Cavia encountered an error flushing your playback state to disk before exiting:
            </p>
            <div
              style={{
                padding: '12px',
                borderRadius: '6px',
                backgroundColor: 'rgba(239, 68, 68, 0.1)',
                border: '1px solid rgba(239, 68, 68, 0.25)',
                color: '#ef4444',
                fontSize: '13px',
                fontFamily: 'monospace',
                wordBreak: 'break-all',
              }}
            >
              {shutdownError}
            </div>
            <p style={{ color: 'var(--cavia-ivory-muted)', fontSize: '12px' }}>
              You may retry persistence now to preserve your session, or force exit immediately.
            </p>
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px', marginTop: '8px' }}>
              <Button variant="vermilion-subtle" onClick={handleForceExit}>
                Force Exit
              </Button>
              <Button
                variant="primary"
                onClick={handleRetryShutdown}
                disabled={isRetryingShutdown}
              >
                {isRetryingShutdown ? 'Retrying...' : 'Retry Shutdown'}
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </div>
  );
}
