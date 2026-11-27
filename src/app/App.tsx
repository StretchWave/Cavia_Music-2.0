import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  AudioWaveform,
  Library,
  Loader2,
  Radio,
  Search,
  Settings2,
  WifiOff,
  X,
} from 'lucide-react';
import {
  getPlatformHealth,
  isNativeApp,
  type PlatformHealth,
} from '../lib/platform';
import {
  cancelScan,
  createPlaylist,
  deletePlaylist,
  deleteTrack,
  deleteTracks,
  unlikeTracks,
  pickMusicFolder,
  recordRecentPlay,
  suggestNextTrack,
  scanMusicDirectory,
  toggleLikedTrack,
  type ScanProgressEvent,
} from '../lib/library';
import {
  clearQueue,
  enqueueTrack,
  getPlaybackState,
  listenPlaybackPosition,
  listenPlaybackState,
  nextTrack,
  notifyPlaybackPosition,
  playNext,
  playPause,
  playQueueIndex,
  previousTrack,
  removeFromQueue,
  removeFromQueueByIndex,
  reorderQueue,
  retryAudioDevice,
  retryShutdown,
  forceExit,
  type ShutdownErrorPayload,
  seek,
  setPlaybackQueue,
  setPlaybackSpeed,
  setQueue,
  setRepeatMode,
  setShuffle,
  setVolume,
  toggleMute,
} from '../lib/playback';
import type {
  Album,
  Artist,
  ConnectSessionSnapshot,
  OfflineFallbackStatus,
  Page,
  PlaybackState,
  PreloadStatusSnapshot,
  Track,
} from '../lib/types';
import {
  connectGetSession,
  listenConnectSession,
  listenOfflineStatus,
  listenPreloadStatus,
  offlineChooseShuffleCached,
  offlineDeclineFallback,
  offlineExitFallback,
  offlineGetStatus,
  preloadGetStatus,
  preloadRetry,
  preloadSetTarget,
} from '../lib/connect';
import {
  getAlbumTracks,
  getArtistTracks,
  ytMusicTrackToTrack,
} from '../lib/trackMatching';
import { Button } from '../components/Button';
import { Modal } from '../components/Modal';
import { LibraryView } from '../features/library/LibraryView';
import { SearchView } from '../features/search/SearchView';
import { SettingsView } from '../features/settings/SettingsView';
import { ImportsView } from '../features/imports/ImportsView';
import { getImportJobs, listenImportProgress, type ImportJob } from '../lib/spotify';
import { PlayerDock } from '../features/player/PlayerDock';
import { ExpandedPlayer } from '../features/player/ExpandedPlayer';
import { QueueDrawer } from '../features/player/QueueDrawer';
import { LyricsDrawer } from '../features/player/LyricsDrawer';
import { ConnectDrawer } from '../features/connect/ConnectDrawer';
import { AddToPlaylistModal } from '../features/library/AddToPlaylistModal';
import { AfterglowBackdrop } from '../components/AfterglowBackdrop';
import { useAfterglowPalette } from '../lib/useAfterglowPalette';
import { useMotionSetting } from '../lib/motion';
import { formatArtworkUri } from '../lib/library';
import { extractShutdownErrorMessage } from '../lib/diagnostics.js';
import { useLibraryController } from './useLibraryController';
import { useAutomaticSync } from './useAutomaticSync';

const pageLabels: Record<Page, string> = {
  library: 'Library',
  search: 'Search',
  imports: 'Imports',
  settings: 'Settings',
};

type ToastKind = 'success' | 'error' | 'info';

interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

function normalizePage(value: string | null): Page | null {
  switch (value) {
    case 'library':
    case 'search':
    case 'settings':
      return value;
    case 'tracks':
    case 'playlists':
    case 'albums':
    case 'artists':
    case 'liked':
      return 'library';
    default:
      return null;
  }
}

const getInitialPage = (): Page => {
  const fromHash = normalizePage(window.location.hash.replace('#', ''));
  if (fromHash) return fromHash;
  const params = new URLSearchParams(window.location.search);
  return normalizePage(params.get('view')) ?? 'library';
};

export function App() {
  const [page, setPage] = useState<Page>(getInitialPage);
  const [health, setHealth] = useState<PlatformHealth | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [activeImportCount, setActiveImportCount] = useState<number>(0);
  const importJobsMapRef = useRef<Map<string, ImportJob>>(new Map());
  const [requestedPlaylistId, setRequestedPlaylistId] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const countActive = () => {
      let count = 0;
      for (const job of importJobsMapRef.current.values()) {
        if (job.status === 'running' || job.status === 'paused' || job.status === 'queued') {
          count += 1;
        }
      }
      setActiveImportCount(count);
    };

    getImportJobs()
      .then((jobs) => {
        importJobsMapRef.current = new Map(jobs.map((job) => [job.id, job]));
        countActive();
      })
      .catch(console.error);

    listenImportProgress((job) => {
      importJobsMapRef.current.set(job.id, job);
      countActive();
    })
      .then((u) => {
        unlisten = u;
      })
      .catch(console.error);

    return () => {
      unlisten?.();
    };
  }, []);

  // Scan & Import State
  const [isScanning, setIsScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState<ScanProgressEvent | null>(null);

  // UI Drawer & Modal States
  const urlParams = useMemo(() => new URLSearchParams(window.location.search), []);
  const [isExpandedPlayerOpen, setIsExpandedPlayerOpen] = useState(urlParams.get('expanded') === 'true');
  const [isQueueOpen, setIsQueueOpen] = useState(urlParams.get('queue') === 'true');
  const [isLyricsOpen, setIsLyricsOpen] = useState(urlParams.get('lyrics') === 'true');
  const [isConnectOpen, setIsConnectOpen] = useState(false);
  const [shutdownError, setShutdownError] = useState<string | null>(null);
  const [isRetryingShutdown, setIsRetryingShutdown] = useState(false);
  const [preloadStatus, setPreloadStatus] = useState<PreloadStatusSnapshot | null>(null);
  const [offlineStatus, setOfflineStatus] = useState<OfflineFallbackStatus | null>(null);
  const [connectSession, setConnectSession] = useState<ConnectSessionSnapshot | null>(null);
  const [offlineBusy, setOfflineBusy] = useState(false);

  // Add to Playlist modal state
  const [addToPlaylistData, setAddToPlaylistData] = useState<{
    isOpen: boolean;
    tracks: Track[];
    contextLabel?: string;
  }>({
    isOpen: false,
    tracks: [],
  });

  // Appearance preferences
  const [density, setDensityState] = useState<'standard' | 'compact'>(() => {
    try {
      return localStorage.getItem('cavia-density') === 'compact' ? 'compact' : 'standard';
    } catch {
      return 'standard';
    }
  });
  const handleDensityChange = useCallback((next: 'standard' | 'compact') => {
    setDensityState(next);
    try {
      localStorage.setItem('cavia-density', next);
    } catch {
      // Ignore storage failures in restricted contexts
    }
  }, []);
  const [motion, setMotion] = useMotionSetting();

  // Playback state (honest initial state: audio backend is standby until native initialization)
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

  // Transient notifications
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const toastIdRef = useRef(0);
  const toastTimersRef = useRef<Map<number, number>>(new Map());
  const { scheduleAutomaticSync, cancelAutomaticSync } = useAutomaticSync();

  const {
    libraryError,
    tracks,
    setTracks,
    albums,
    artists,
    playlists,
    setPlaylists,
    likedTracks,
    setLikedTracks,
    tracksRef,
    likedTracksRef,
    refreshLibrary,
  } = useLibraryController(scheduleAutomaticSync);

  const dismissToast = useCallback((id: number) => {
    const timer = toastTimersRef.current.get(id);
    if (timer !== undefined) {
      window.clearTimeout(timer);
      toastTimersRef.current.delete(id);
    }
    setToasts((prev) => prev.filter((toast) => toast.id !== id));
  }, []);

  const pushToast = useCallback(
    (kind: ToastKind, message: string) => {
      const id = ++toastIdRef.current;
      setToasts((prev) => [...prev, { id, kind, message }]);
      const timer = window.setTimeout(() => {
        dismissToast(id);
      }, 4000);
      toastTimersRef.current.set(id, timer);
    },
    [dismissToast]
  );

  useEffect(() => {
    return () => {
      for (const timer of toastTimersRef.current.values()) {
        window.clearTimeout(timer);
      }
      toastTimersRef.current.clear();
    };
  }, []);

  const showPlaybackError = useCallback(
    (message: string) => {
      pushToast('error', message);
      setPlayback((prev) => ({ ...prev, errorMessage: message }));
    },
    [pushToast]
  );

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
    let unlistenShutdown: (() => void) | undefined;

    getPlaybackState()
      .then((state) => {
        if (state) {
          notifyPlaybackPosition({
            positionMs: state.positionMs,
            durationMs: state.durationMs,
            trackId: state.currentTrack?.id,
            generation: state.generation,
          });
          setPlayback((prev) => ({
            ...prev,
            status: state.status,
            currentTrack: state.currentTrack,
            currentIndex: state.currentIndex,
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
      notifyPlaybackPosition({
        positionMs: snap.positionMs,
        durationMs: snap.durationMs,
        trackId: snap.currentTrack?.id,
        generation: snap.generation,
      });
      setPlayback((prev) => {
        if (snap.generation !== undefined && prev.generation !== undefined && snap.generation < prev.generation) {
          return prev;
        }
        return {
          ...prev,
          status: snap.status,
          currentTrack: snap.currentTrack,
          currentIndex: snap.currentIndex,
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
      // High-frequency position ticks notify subscriber player components directly,
      // preventing continuous root App and subview re-renders during playback.
      notifyPlaybackPosition(pos);
    }).then((fn) => {
      unlistenPos = fn;
    });

    listen<ShutdownErrorPayload>('app:shutdown-error', (event) => {
      setShutdownError(
        extractShutdownErrorMessage(event.payload) ??
          'Failed to save playback state before exiting.'
      );
    }).then((fn) => {
      unlistenShutdown = fn;
    });

    let unlistenPreload: (() => void) | undefined;
    let unlistenOffline: (() => void) | undefined;
    let unlistenConnect: (() => void) | undefined;

    preloadGetStatus().then((snap) => snap && setPreloadStatus(snap)).catch(console.error);
    offlineGetStatus().then((st) => st && setOfflineStatus(st)).catch(console.error);
    connectGetSession().then((sess) => setConnectSession(sess)).catch(console.error);

    listenPreloadStatus((snap) => setPreloadStatus(snap)).then((fn) => {
      unlistenPreload = fn;
    });
    listenOfflineStatus((st) => setOfflineStatus(st)).then((fn) => {
      unlistenOffline = fn;
    });
    listenConnectSession((sess) => setConnectSession(sess)).then((fn) => {
      unlistenConnect = fn;
    });

    return () => {
      if (unlistenState) unlistenState();
      if (unlistenPos) unlistenPos();
      if (unlistenShutdown) unlistenShutdown();
      if (unlistenPreload) unlistenPreload();
      if (unlistenOffline) unlistenOffline();
      if (unlistenConnect) unlistenConnect();
    };
  }, []);

  const handleChangePreloadTarget = useCallback(
    async (target: number) => {
      try {
        const snap = await preloadSetTarget(target);
        if (snap) setPreloadStatus(snap);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        pushToast('error', `Failed to update preload target: ${msg}`);
      }
    },
    [pushToast]
  );

  const handleRetryPreload = useCallback(async () => {
    try {
      const snap = await preloadRetry();
      if (snap) setPreloadStatus(snap);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      pushToast('error', `Retry preload failed: ${msg}`);
    }
  }, [pushToast]);

  const handleShuffleCachedOffline = useCallback(async () => {
    setOfflineBusy(true);
    try {
      const next = await offlineChooseShuffleCached();
      if (next) setOfflineStatus(next);
      pushToast('info', 'Shuffling verified cached tracks offline.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      pushToast('error', msg);
      const st = await offlineGetStatus().catch(() => null);
      if (st) setOfflineStatus(st);
    } finally {
      setOfflineBusy(false);
    }
  }, [pushToast]);

  const handleDeclineOfflineFallback = useCallback(async () => {
    setOfflineBusy(true);
    try {
      const next = await offlineDeclineFallback();
      if (next) setOfflineStatus(next);
    } catch (err) {
      pushToast('error', err instanceof Error ? err.message : String(err));
    } finally {
      setOfflineBusy(false);
    }
  }, [pushToast]);

  const handleExitOfflineFallback = useCallback(async () => {
    setOfflineBusy(true);
    try {
      const next = await offlineExitFallback();
      if (next) setOfflineStatus(next);
      pushToast('info', 'Restored original queue.');
    } catch (err) {
      pushToast('error', err instanceof Error ? err.message : String(err));
    } finally {
      setOfflineBusy(false);
    }
  }, [pushToast]);

  const handleRetryShutdown = useCallback(async () => {
    try {
      setIsRetryingShutdown(true);
      await retryShutdown();
      setShutdownError(null);
    } catch (err) {
      setShutdownError(String(err));
    } finally {
      setIsRetryingShutdown(false);
    }
  }, []);

  const handleForceExit = useCallback(async () => {
    try {
      await forceExit();
    } catch (err) {
      console.error('Failed to force exit:', err);
    }
  }, []);

  const handleImportFolder = useCallback(async () => {
    try {
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
        pushToast('info', `Import cancelled. Added ${result.newTracks} tracks.`);
      } else {
        pushToast(
          'success',
          `Scan complete: ${result.newTracks} new recordings indexed (${result.duplicates} existing/duplicates skipped).`
        );
      }
    } catch (err) {
      setIsScanning(false);
      pushToast('error', err instanceof Error ? err.message : String(err));
    }
  }, [pushToast, refreshLibrary]);

  const handleCancelScan = useCallback(async () => {
    try {
      await cancelScan();
    } catch (err) {
      console.error('Error cancelling scan:', err);
    }
  }, []);

  const applyLikeState = useCallback((trackId: string, isLiked: boolean) => {
    setTracks((prev) => prev.map((track) => (track.id === trackId ? { ...track, isLiked } : track)));
    setPlayback((prev) =>
      prev.currentTrack?.id === trackId
        ? { ...prev, currentTrack: { ...prev.currentTrack, isLiked } }
        : prev
    );
    setLikedTracks((prev) => {
      if (!isLiked) {
        return prev.filter((track) => track.id !== trackId);
      }
      if (prev.some((track) => track.id === trackId)) {
        return prev.map((track) => (track.id === trackId ? { ...track, isLiked: true } : track));
      }
      const source = tracksRef.current.find((track) => track.id === trackId);
      return source ? [{ ...source, isLiked: true }, ...prev] : prev;
    });
  }, []);

  const handleToggleLike = useCallback(
    async (trackId: string) => {
      const source =
        tracksRef.current.find((track) => track.id === trackId) ??
        likedTracksRef.current.find((track) => track.id === trackId);
      const wasLiked =
        likedTracksRef.current.some((track) => track.id === trackId) || Boolean(source?.isLiked);
      const nextLiked = !wasLiked;

      applyLikeState(trackId, nextLiked);

      try {
        const persistedLiked = await toggleLikedTrack(trackId);
        if (persistedLiked !== nextLiked) {
          applyLikeState(trackId, persistedLiked);
        }
        scheduleAutomaticSync();
      } catch (err) {
        applyLikeState(trackId, wasLiked);
        pushToast('error', err instanceof Error ? err.message : String(err));
      }
    },
    [applyLikeState, pushToast, scheduleAutomaticSync]
  );

  const handleCreatePlaylist = useCallback(
    async (name: string, description: string) => {
      try {
        const created = await createPlaylist(name, description);
        setPlaylists((prev) => [created, ...prev]);
        await refreshLibrary();
      } catch (err) {
        console.error('Error creating playlist in SQLite:', err);
        pushToast('error', 'Could not create playlist.');
      }
    },
    [pushToast, refreshLibrary]
  );

  const handleDeletePlaylist = useCallback(
    async (playlistId: string) => {
      try {
        await deletePlaylist(playlistId);
        setPlaylists((prev) => prev.filter((playlist) => playlist.id !== playlistId));
        await refreshLibrary();
      } catch (err) {
        console.error('Error deleting playlist from SQLite:', err);
        pushToast('error', 'Could not delete playlist.');
      }
    },
    [pushToast, refreshLibrary]
  );

  const handleDeleteTrack = useCallback(
    async (trackId: string) => {
      try {
        await deleteTrack(trackId);
        pushToast('success', 'Track removed from library.');
        await refreshLibrary();
      } catch (err) {
        console.error('Error removing track from library:', err);
        pushToast('error', 'Could not remove track from library.');
      }
    },
    [pushToast, refreshLibrary]
  );

  const handleDeleteTracks = useCallback(
    async (trackIds: string[]) => {
      try {
        const count = await deleteTracks(trackIds);
        pushToast('success', `${count} ${count === 1 ? 'track' : 'tracks'} removed from library.`);
        await refreshLibrary();
      } catch (err) {
        console.error('Error removing tracks from library:', err);
        pushToast('error', 'Could not remove tracks from library.');
      }
    },
    [pushToast, refreshLibrary]
  );

  const handleUnlikeTracks = useCallback(
    async (trackIds: string[]) => {
      try {
        const count = await unlikeTracks(trackIds);
        pushToast('success', `${count} ${count === 1 ? 'track' : 'tracks'} removed from Liked.`);
        await refreshLibrary();
      } catch (err) {
        console.error('Error removing tracks from Liked:', err);
        pushToast('error', 'Could not remove tracks from Liked.');
      }
    },
    [pushToast, refreshLibrary]
  );

  const handleOpenAddToPlaylist = useCallback((track: Track) => {
    setAddToPlaylistData({
      isOpen: true,
      tracks: [track],
      contextLabel: `Track: ${track.title}`,
    });
  }, []);

  const handleOpenAddTracksToPlaylist = useCallback((tracks: Track[]) => {
    if (tracks.length === 0) return;
    setAddToPlaylistData({
      isOpen: true,
      tracks,
      contextLabel: tracks.length === 1 ? `Track: ${tracks[0].title}` : `${tracks.length} tracks`,
    });
  }, []);

  const handleOpenAddAlbumToPlaylist = useCallback(
    (album: Album) => {
      const albumTracks = getAlbumTracks(tracks, album);
      setAddToPlaylistData({
        isOpen: true,
        tracks: albumTracks,
        contextLabel: `Album: ${album.title}`,
      });
    },
    [tracks]
  );

  // Playback transport handlers (native Rust backend authoritative)
  const handleTogglePlay = useCallback(async () => {
    try {
      await playPause();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Play/Pause failed:', msg);
      showPlaybackError(`Playback failed: ${msg}`);
    }
  }, [showPlaybackError]);

  const handleRetryDevice = useCallback(async () => {
    try {
      await retryAudioDevice();
      const nextHealth = await getPlatformHealth();
      setHealth(nextHealth);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Retry audio device failed:', msg);
      showPlaybackError(`Device retry failed: ${msg}`);
    }
  }, [showPlaybackError]);

  const previousStatusRef = useRef<PlaybackState['status']>('idle');
  const activePlayingTrackRef = useRef<Track | null>(null);
  const userStoppedOrRemovedRef = useRef<boolean>(false);

  useEffect(() => {
    if (playback.status === 'playing') {
      activePlayingTrackRef.current = playback.currentTrack;
      userStoppedOrRemovedRef.current = false;
    }
  }, [playback.currentTrack, playback.status]);

  // Search-page history: only tracks that actually started playing are
  // recorded. Search queries are never stored.
  const lastRecordedTrackRef = useRef<string | null>(null);
  const [historyRevision, setHistoryRevision] = useState(0);
  useEffect(() => {
    const track = playback.currentTrack;
    if (!track) return;
    if (playback.status !== 'playing' && playback.status !== 'loading') return;
    if (lastRecordedTrackRef.current === track.id) return;
    lastRecordedTrackRef.current = track.id;
    void recordRecentPlay(track)
      .then(() => setHistoryRevision((revision) => revision + 1))
      .catch((err) => {
        console.error('Failed to record playback history:', err);
      });
  }, [playback.currentTrack, playback.status]);

  useEffect(() => {
    const status = playback.status;
    const previous = previousStatusRef.current;
    previousStatusRef.current = status;

    if (previous !== 'playing' || (status !== 'stopped' && status !== 'idle')) return;

    if (userStoppedOrRemovedRef.current) {
      userStoppedOrRemovedRef.current = false;
      return;
    }

    const finished = activePlayingTrackRef.current;
    if (!finished) return;

    // A local track played on its own continues from library tag similarity.
    // Mixed queues and online provider radio are owned natively by Rust.
    if (finished.sourceKind === 'local' && playback.queue.length <= 1) {
      let cancelled = false;
      void suggestNextTrack(finished.id)
        .then(async (next) => {
          if (cancelled || !next) return;
          await setQueue([next.id], next.id);
        })
        .catch((err) => {
          console.error('Autoplay failed:', err);
        });
      return () => {
        cancelled = true;
      };
    }
  }, [playback.status, playback.currentTrack?.id, playback.queue.length]);

  const handlePlayTrack = useCallback(
    async (trackId: string) => {
      try {
        const list = tracksRef.current;
        const index = list.findIndex((track) => track.id === trackId);
        if (index < 0) return;
        await setPlaybackQueue(list, index, false);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('Play track failed:', msg);
        showPlaybackError(`Playback failed: ${msg}`);
      }
    },
    [showPlaybackError]
  );

  const handlePlayTracks = useCallback(
    async (tracksToPlay: Track[], startIndex = 0, shuffle = false) => {
      try {
        if (tracksToPlay.length === 0) return;
        if (shuffle) {
          await setShuffle(true);
        }
        const safeIndex = Math.max(0, Math.min(startIndex, tracksToPlay.length - 1));
        const enableRadio =
          tracksToPlay.length === 1 && tracksToPlay[0].sourceKind === 'provider';
        await setPlaybackQueue(tracksToPlay, safeIndex, enableRadio);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('Play collection failed:', msg);
        showPlaybackError(`Playback failed: ${msg}`);
      }
    },
    [showPlaybackError]
  );

  const handlePlayAll = useCallback(() => {
    void handlePlayTracks(tracksRef.current, 0, false);
  }, [handlePlayTracks]);

  const handleShuffleAll = useCallback(() => {
    void handlePlayTracks(tracksRef.current, 0, true);
  }, [handlePlayTracks]);

  const handlePlayLikedTrack = useCallback(
    (trackId: string) => {
      const list = likedTracksRef.current;
      const index = list.findIndex((track) => track.id === trackId);
      void handlePlayTracks(list, index >= 0 ? index : 0, false);
    },
    [handlePlayTracks]
  );

  const handlePlayAllLiked = useCallback(() => {
    void handlePlayTracks(likedTracksRef.current, 0, false);
  }, [handlePlayTracks]);

  const handleShuffleAllLiked = useCallback(() => {
    void handlePlayTracks(likedTracksRef.current, 0, true);
  }, [handlePlayTracks]);

  const handlePlayPlaylistTracks = useCallback(
    (playlistTracks: Track[], startIndex = 0, shuffle = false) => {
      void handlePlayTracks(playlistTracks, startIndex, shuffle);
    },
    [handlePlayTracks]
  );

  const handlePlayAlbum = useCallback(
    (album: Album) => {
      void handlePlayTracks(getAlbumTracks(tracksRef.current, album), 0, false);
    },
    [handlePlayTracks]
  );

  const handlePlayArtist = useCallback(
    (artist: Artist) => {
      void handlePlayTracks(getArtistTracks(tracksRef.current, artist), 0, false);
    },
    [handlePlayTracks]
  );

  const handlePlayNext = useCallback(
    async (trackId: string) => {
      try {
        await playNext(trackId);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('Play next failed:', msg);
        showPlaybackError(`Play next failed: ${msg}`);
      }
    },
    [showPlaybackError]
  );

  const handlePlayNextTrack = useCallback(
    async (track: Track) => {
      try {
        await enqueueTrack(track, true);
        pushToast('info', `Queued "${track.title}" to play next.`);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('Play next failed:', msg);
        showPlaybackError(`Play next failed: ${msg}`);
      }
    },
    [pushToast, showPlaybackError]
  );

  const [routeAnnouncement, setRouteAnnouncement] = useState('');

  useEffect(() => {
    if (playback.currentTrack && playback.status === 'playing') {
      document.title = `${playback.currentTrack.title} — ${playback.currentTrack.artist} | Cavia Music`;
    } else {
      document.title = `Cavia Music — ${pageLabels[page]}`;
    }
    setRouteAnnouncement(`Navigated to ${pageLabels[page]}`);
  }, [page, playback.currentTrack, playback.status]);

  useEffect(() => {
    // Fields and native controls own their Space/Enter behavior. Without this
    // guard, pressing Space on a focused button, track row or tab both activates
    // that control and toggles playback.
    const interactiveTargetSelector = [
      'input',
      'textarea',
      'select',
      'button',
      'a[href]',
      'summary',
      '[contenteditable="true"]',
      '[role="button"]',
      '[role="tab"]',
      '[role="menuitem"]',
      '[role="slider"]',
      '[role="row"]',
      '[role="option"]',
    ].join(', ');

    function handleGlobalKeyDown(e: KeyboardEvent) {
      const target = e.target instanceof Element ? e.target : null;
      if (target?.closest(interactiveTargetSelector)) return;

      // Space -> Toggle Play/Pause (only when playback is actually available)
      if (e.code === 'Space') {
        if (health?.audio_backend !== 'ready') return;
        e.preventDefault();
        handleTogglePlay();
      }

      // Cmd/Ctrl + K -> Focus or jump to Search
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setPage('search');
        setTimeout(() => {
          const searchInput = document.querySelector<HTMLInputElement>('.cavia-search-field');
          searchInput?.focus();
        }, 50);
      }

      // Cmd/Ctrl + , -> Settings
      if ((e.metaKey || e.ctrlKey) && e.key === ',') {
        e.preventDefault();
        setPage('settings');
      }
    }

    window.addEventListener('keydown', handleGlobalKeyDown);
    return () => window.removeEventListener('keydown', handleGlobalKeyDown);
  }, [handleTogglePlay, health?.audio_backend]);

  const handleNext = useCallback(async () => {
    try {
      await nextTrack();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Next track failed:', msg);
      showPlaybackError(`Next track failed: ${msg}`);
    }
  }, [showPlaybackError]);

  const handlePrevious = useCallback(async () => {
    try {
      await previousTrack();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Previous track failed:', msg);
      showPlaybackError(`Previous track failed: ${msg}`);
    }
  }, [showPlaybackError]);

  const handleSeek = useCallback(
    async (positionMs: number) => {
      try {
        notifyPlaybackPosition({
          positionMs,
          durationMs: playback.durationMs,
          trackId: playback.currentTrack?.id,
          generation: playback.generation,
        });
        await seek(positionMs, playback.generation);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('Seek failed:', msg);
        showPlaybackError(`Seek failed: ${msg}`);
      }
    },
    [playback.currentTrack?.id, playback.durationMs, playback.generation, showPlaybackError]
  );

  const handleVolume = useCallback(
    async (vol: number) => {
      try {
        await setVolume(vol);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('Set volume failed:', msg);
        showPlaybackError(`Volume change failed: ${msg}`);
      }
    },
    [showPlaybackError]
  );

  const handleToggleMute = useCallback(async () => {
    try {
      await toggleMute();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Toggle mute failed:', msg);
      showPlaybackError(`Toggle mute failed: ${msg}`);
    }
  }, [showPlaybackError]);

  const handleCycleRepeat = useCallback(async () => {
    try {
      const next = playback.repeatMode === 'off' ? 'all' : playback.repeatMode === 'all' ? 'one' : 'off';
      await setRepeatMode(next);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Cycle repeat failed:', msg);
      showPlaybackError(`Repeat mode change failed: ${msg}`);
    }
  }, [playback.repeatMode, showPlaybackError]);

  const handleToggleShuffle = useCallback(async () => {
    try {
      await setShuffle(!playback.isShuffle);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Toggle shuffle failed:', msg);
      showPlaybackError(`Shuffle toggle failed: ${msg}`);
    }
  }, [playback.isShuffle, showPlaybackError]);

  const handleCycleSpeed = useCallback(async () => {
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
  }, [playback.playbackSpeed, showPlaybackError]);

  const handleClearQueue = useCallback(async () => {
    try {
      userStoppedOrRemovedRef.current = true;
      await clearQueue();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Clear queue failed:', msg);
      showPlaybackError(`Clear queue failed: ${msg}`);
    }
  }, [showPlaybackError]);

  const handleRemoveQueueTrack = useCallback(
    async (trackId: string, index?: number) => {
      try {
        if (
          playback.currentTrack?.id === trackId ||
          (typeof index === 'number' && playback.queue[index]?.id === playback.currentTrack?.id)
        ) {
          userStoppedOrRemovedRef.current = true;
        }
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
    },
    [playback.currentTrack?.id, playback.queue, showPlaybackError]
  );

  const handleReorderQueue = useCallback(
    async (fromIndex: number, toIndex: number) => {
      try {
        await reorderQueue(fromIndex, toIndex);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('Reorder queue failed:', msg);
        showPlaybackError(`Reorder queue failed: ${msg}`);
      }
    },
    [showPlaybackError]
  );

  const handleSelectQueueTrack = useCallback(
    async (index: number) => {
      try {
        await playQueueIndex(index);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('Select queue track failed:', msg);
        showPlaybackError(`Playback failed: ${msg}`);
      }
    },
    [showPlaybackError]
  );

  const isBackendReady = health?.audio_backend === 'ready';

  // Afterglow: the current artwork lights the room and tints the shell.
  const nowPlayingArtwork = useMemo(
    () =>
      playback.currentTrack?.artworkUri
        ? formatArtworkUri(playback.currentTrack.artworkUri)
        : undefined,
    [playback.currentTrack?.artworkUri]
  );
  useAfterglowPalette(nowPlayingArtwork);

  const statusLabel = useMemo(() => {
    if (!isNativeApp()) return 'Design preview';
    if (connectionError) return 'Core unavailable';
    if (!health) return 'Connecting';
    if (!health.database_connected) return 'Database offline';
    if (health.audio_backend === 'device_unavailable') return 'No audio device';
    if (health.audio_backend === 'not_integrated') return 'Backend not integrated';
    if (health.audio_backend === 'error') return 'Audio error';
    if (health.audio_backend === 'unsupported') return 'Unsupported platform';
    return 'Local core ready';
  }, [connectionError, health]);

  const statusTone = connectionError
    ? 'error'
    : health?.database_connected && health?.audio_backend === 'ready'
      ? 'ready'
      : 'muted';

  const navItems = [
    { label: 'Library', page: 'library' as const, icon: Library, badge: activeImportCount },
    { label: 'Search', page: 'search' as const, icon: Search },
    { label: 'Settings', page: 'settings' as const, icon: Settings2 },
  ];

  return (
    <div
      className={`cavia-app-frame ${density === 'compact' ? 'density-compact' : ''}`}
      data-motion={motion}
      data-playing={playback.status === 'playing' ? 'true' : 'false'}
    >
      <AfterglowBackdrop
        artworkUri={nowPlayingArtwork}
        isPlaying={playback.status === 'playing'}
        enabled={motion !== 'off'}
      />

      <a href="#main-content" className="skip-link">
        Skip to main content
      </a>

      <div className="sr-only" role="status" aria-live="polite">
        {routeAnnouncement}
      </div>

      <aside className="cavia-sidebar" aria-label="Main navigation">
        <div className="brand-lockup">
          <div className="brand-mark" aria-hidden="true">
            <AudioWaveform size={20} strokeWidth={1.8} />
          </div>
          <span className="brand-name">
            cavia<span>.</span>
          </span>
        </div>

        <nav className="sidebar-nav">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = page === item.page || (page === 'imports' && item.page === 'library');
            return (
              <button
                key={item.page}
                type="button"
                className={`nav-item ${isActive ? 'active' : ''}`}
                aria-current={isActive ? 'page' : undefined}
                onClick={(e) => {
                  if (e.detail > 0) e.currentTarget.blur();
                  setPage(item.page);
                }}
              >
                <Icon size={17} strokeWidth={1.8} />
                <span>{item.label}</span>
                {Boolean(item.badge && item.badge > 0) && (
                  <span className="nav-badge">{item.badge}</span>
                )}
              </button>
            );
          })}
        </nav>
      </aside>

      <main id="main-content" className="cavia-main-shell">
        <header className="cavia-topbar">
          <span className="topbar-page-title">{pageLabels[page]}</span>

          <div className="topbar-actions">
            {offlineStatus?.fallbackActive && (
              <button
                type="button"
                className="topbar-offline-chip"
                onClick={() => setIsQueueOpen(true)}
                title={offlineStatus.statusMessage ?? 'Offline cached playback active'}
              >
                <WifiOff size={13} aria-hidden="true" />
                <span>
                  {offlineStatus.reconnectPendingFinish
                    ? 'Online · finishing track'
                    : `Offline (${offlineStatus.remainingFallbackCount} left)`}
                </span>
              </button>
            )}

            <button
              type="button"
              className={`topbar-connect-btn ${connectSession ? 'is-active' : ''}`}
              onClick={() => setIsConnectOpen(true)}
              title="Open Cavia Connect collaborative session"
              aria-label="Open Cavia Connect"
            >
              <Radio size={14} aria-hidden="true" />
              <span className="topbar-connect-label">
                {connectSession ? connectSession.joinCode : 'Connect'}
              </span>
            </button>

            <span
              className={`core-status-chip status-${statusTone}`}
              title={statusLabel}
              role="status"
              aria-label={`Core status: ${statusLabel}`}
            >
              <span className={`core-status-dot status-${statusTone}`} aria-hidden="true" />
              <span className="core-status-label">{statusLabel}</span>
            </span>
          </div>
        </header>

        {isScanning && scanProgress && page !== 'imports' && (
          <div className="cavia-scan-banner" role="status" aria-live="polite">
            <div className="scan-banner-header">
              <div className="scan-banner-info">
                <Loader2 size={15} className="spin-accent text-vermilion" />
                <strong className="scan-title">
                  {scanProgress.phase === 'discovering'
                    ? 'Discovering audio files…'
                    : `Indexing collection (${scanProgress.processedCount} / ${scanProgress.discoveredCount})`}
                </strong>
                <span className="scan-meta-pill">
                  {scanProgress.newTracksCount} new · {scanProgress.duplicatesCount} skipped
                </span>
              </div>
              <Button variant="outline" size="sm" onClick={handleCancelScan} className="scan-cancel-btn">
                Cancel
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

        <div className="cavia-page-scroll" data-page={page} tabIndex={-1}>
          {page === 'library' && (
            <LibraryView
              connectionError={connectionError}
              libraryError={libraryError}
              onRetryLibrary={refreshLibrary}
              tracks={tracks}
              albums={albums}
              artists={artists}
              playlists={playlists}
              likedTracks={likedTracks}
              density={density}
              isBackendReady={isBackendReady}
              currentTrackId={playback.currentTrack?.id}
              isPlaying={playback.status === 'playing'}
              onImportFolder={handleImportFolder}
              isScanning={isScanning}
              onCreatePlaylist={handleCreatePlaylist}
              onDeletePlaylist={handleDeletePlaylist}
              onDeleteTrack={handleDeleteTrack}
              onDeleteTracks={handleDeleteTracks}
              onUnlikeTracks={handleUnlikeTracks}
              onPlaylistUpdated={refreshLibrary}
              onToggleLike={handleToggleLike}
              onPlayTrack={handlePlayTrack}
              onPlayTracks={handlePlayPlaylistTracks}
              onPlayAll={handlePlayAll}
              onShuffleAll={handleShuffleAll}
              onPlayLikedTrack={handlePlayLikedTrack}
              onPlayAllLiked={handlePlayAllLiked}
              onShuffleAllLiked={handleShuffleAllLiked}
              onPlayAlbum={handlePlayAlbum}
              onPlayArtist={handlePlayArtist}
              onAddToPlaylist={handleOpenAddToPlaylist}
              onAddTracksToPlaylist={handleOpenAddTracksToPlaylist}
              onAddAlbumToPlaylist={handleOpenAddAlbumToPlaylist}
              onPlayNext={handlePlayNext}
              onNavigateToImports={() => setPage('imports')}
              activeImportCount={activeImportCount}
              requestedPlaylistId={requestedPlaylistId}
              onRequestedPlaylistHandled={() => setRequestedPlaylistId(null)}
            />
          )}

          {page === 'imports' && (
            <ImportsView
              onOpenPlaylist={(playlistId) => {
                setRequestedPlaylistId(playlistId);
                setPage('library');
                refreshLibrary();
              }}
              onExit={() => setPage('library')}
              onPlaylistCreated={refreshLibrary}
              onImportFolder={handleImportFolder}
              isScanning={isScanning}
              scanProgress={scanProgress}
              onCancelScan={handleCancelScan}
            />
          )}

          {page === 'search' && (
            <SearchView
              isBackendReady={isBackendReady}
              density={density}
              onPlayTracks={handlePlayPlaylistTracks}
              onToggleLike={handleToggleLike}
              onAddToPlaylist={handleOpenAddToPlaylist}
              onAddAlbumToPlaylist={handleOpenAddAlbumToPlaylist}
              onPlaylistUpdated={refreshLibrary}
              onDeletePlaylist={handleDeletePlaylist}
              onPlayAlbum={handlePlayAlbum}
              onPlayArtist={handlePlayArtist}
              onPlayNext={handlePlayNext}
              onPlayNextTrack={handlePlayNextTrack}
              onPlayOnlineTrack={async (track) =>
                setPlaybackQueue([ytMusicTrackToTrack(track)], 0, true)
              }
              historyRevision={historyRevision}
              allTracks={tracks}
              currentTrackId={playback.currentTrack?.id}
              isPlaying={playback.status === 'playing'}
            />
          )}

          {page === 'settings' && (
            <SettingsView
              health={health}
              density={density}
              onDensityChange={handleDensityChange}
              motion={motion}
              onMotionChange={setMotion}
              preloadStatus={preloadStatus}
              onChangePreloadTarget={handleChangePreloadTarget}
              onSyncConfigurationChanged={(enabled) => {
                if (enabled) {
                  scheduleAutomaticSync(250);
                } else cancelAutomaticSync();
              }}
            />
          )}
        </div>

        <PlayerDock
          playback={playback}
          isBackendReady={isBackendReady}
          audioBackendStatus={health?.audio_backend}
          preloadStatus={preloadStatus}
          offlineStatus={offlineStatus}
          onChangePreloadTarget={handleChangePreloadTarget}
          onRetryPreload={handleRetryPreload}
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
        />
      </main>

      <ExpandedPlayer
        isOpen={isExpandedPlayerOpen}
        onClose={() => setIsExpandedPlayerOpen(false)}
        playback={playback}
        isBackendReady={isBackendReady}
        audioBackendStatus={health?.audio_backend}
        preloadStatus={preloadStatus}
        onChangePreloadTarget={handleChangePreloadTarget}
        onRetryPreload={handleRetryPreload}
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

      <QueueDrawer
        isOpen={isQueueOpen}
        onClose={() => setIsQueueOpen(false)}
        currentTrack={playback.currentTrack}
        currentIndex={playback.currentIndex}
        queue={playback.queue}
        isPlaying={playback.status === 'playing'}
        status={playback.status}
        isBackendReady={isBackendReady}
        preloadStatus={preloadStatus}
        offlineStatus={offlineStatus}
        connectSession={connectSession}
        onChangePreloadTarget={handleChangePreloadTarget}
        onExitOfflineFallback={handleExitOfflineFallback}
        onOpenConnect={() => setIsConnectOpen(true)}
        onClearQueue={handleClearQueue}
        onRemoveTrack={handleRemoveQueueTrack}
        onSelectTrack={(index) => handleSelectQueueTrack(index)}
        onShuffleQueue={handleToggleShuffle}
        onReorderQueue={handleReorderQueue}
      />

      <ConnectDrawer
        isOpen={isConnectOpen}
        onClose={() => setIsConnectOpen(false)}
        session={connectSession}
        isOffline={Boolean(offlineStatus?.isOffline)}
        localTracks={tracks}
        onSessionChanged={setConnectSession}
        onNotify={pushToast}
      />

      <LyricsDrawer
        isOpen={isLyricsOpen}
        onClose={() => setIsLyricsOpen(false)}
        currentTrack={playback.currentTrack}
        playbackPositionMs={playback.positionMs}
        trackDurationMs={playback.durationMs}
        reducedMotion={motion !== 'full'}
        onSeek={handleSeek}
      />

      <AddToPlaylistModal
        isOpen={addToPlaylistData.isOpen}
        onClose={() => setAddToPlaylistData((prev) => ({ ...prev, isOpen: false }))}
        tracksToAdd={addToPlaylistData.tracks}
        contextLabel={addToPlaylistData.contextLabel}
        playlists={playlists}
        onPlaylistUpdated={refreshLibrary}
      />

      <nav className="cavia-mobile-nav" aria-label="Mobile navigation">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = page === item.page || (page === 'imports' && item.page === 'library');
          return (
            <button
              key={item.page}
              type="button"
              className={`mobile-nav-btn ${isActive ? 'active' : ''}`}
              aria-current={isActive ? 'page' : undefined}
              onClick={() => setPage(item.page)}
            >
              <div className="mobile-nav-icon-wrap">
                <Icon size={19} />
                {Boolean(item.badge && item.badge > 0) && (
                  <span className="mobile-nav-badge-dot" />
                )}
              </div>
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="cavia-toast-stack" aria-live="polite">
        {toasts.map((toast) => (
          <div
            key={toast.id}
            className={`cavia-toast toast-${toast.kind}`}
            role="status"
          >
            <span className="toast-dot" aria-hidden="true" />
            <span className="toast-message">{toast.message}</span>
            <button
              type="button"
              className="toast-close"
              onClick={() => dismissToast(toast.id)}
              aria-label="Dismiss notification"
            >
              <X size={14} />
            </button>
          </div>
        ))}
      </div>

      {offlineStatus?.promptVisible && (
        <Modal
          isOpen
          onClose={handleDeclineOfflineFallback}
          title="Internet is unavailable"
        >
          <div className="modal-stack">
            <p className="modal-text">
              {offlineStatus.promptMessage ??
                'Internet is unavailable. Would you like to continue with cached songs?'}
            </p>
            <p className="modal-text modal-text-muted">
              {offlineStatus.eligibleCachedCount > 0
                ? `${offlineStatus.eligibleCachedCount} verified cached and local songs are ready offline without modifying your queue.`
                : 'No verified cached or local songs are available in your library or stream cache yet.'}
            </p>
            <div className="modal-actions-right">
              <Button
                variant="outline"
                disabled={offlineBusy}
                onClick={handleDeclineOfflineFallback}
              >
                Don&apos;t Switch
              </Button>
              <Button
                variant="primary"
                disabled={offlineBusy || offlineStatus.eligibleCachedCount === 0}
                onClick={handleShuffleCachedOffline}
              >
                {offlineBusy ? 'Switching…' : 'Shuffle Cached Songs'}
              </Button>
            </div>
          </div>
        </Modal>
      )}

      {shutdownError && (
        <Modal
          isOpen
          onClose={() => setShutdownError(null)}
          title="Shutdown persistence alert"
        >
          <div className="modal-stack">
            <p className="modal-text">
              Cavia encountered an error flushing your playback state to disk before exiting:
            </p>
            <div className="modal-error-box">{shutdownError}</div>
            <p className="modal-text modal-text-muted">
              Retry persistence to preserve your session, or exit without saving.
            </p>
            <div className="modal-actions-right">
              <Button variant="vermilion-subtle" onClick={handleForceExit}>
                Force exit
              </Button>
              <Button variant="primary" onClick={handleRetryShutdown} disabled={isRetryingShutdown}>
                {isRetryingShutdown ? 'Retrying…' : 'Retry shutdown'}
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </div>
  );
}
