import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  AudioWaveform,
  FolderPlus,
  Library,
  Loader2,
  Search,
  Settings2,
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
  notifyPlaybackPosition,
  playNext,
  playPause,
  playTrack,
  previousTrack,
  removeFromQueue,
  removeFromQueueByIndex,
  reorderQueue,
  retryAudioDevice,
  retryShutdown,
  forceExit,
  type ShutdownErrorPayload,
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
import { SearchView } from '../features/search/SearchView';
import { SettingsView } from '../features/settings/SettingsView';
import { PlayerDock } from '../features/player/PlayerDock';
import { ExpandedPlayer } from '../features/player/ExpandedPlayer';
import { QueueDrawer } from '../features/player/QueueDrawer';
import { LyricsDrawer } from '../features/player/LyricsDrawer';
import { AddToPlaylistModal } from '../features/library/AddToPlaylistModal';
import { AfterglowBackdrop } from '../components/AfterglowBackdrop';
import { useAfterglowPalette } from '../lib/useAfterglowPalette';
import { useMotionSetting } from '../lib/motion';
import { formatArtworkUri } from '../lib/library';

const pageLabels: Record<Page, string> = {
  library: 'Library',
  search: 'Search',
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

  // Real library domain state (owned by Rust SQLite)
  const [tracks, setTracks] = useState<Track[]>([]);
  const [albums, setAlbums] = useState<Album[]>([]);
  const [artists, setArtists] = useState<Artist[]>([]);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [likedTracks, setLikedTracks] = useState<Track[]>([]);

  const tracksRef = useRef<Track[]>(tracks);
  const likedTracksRef = useRef<Track[]>(likedTracks);

  useEffect(() => {
    tracksRef.current = tracks;
  }, [tracks]);

  useEffect(() => {
    likedTracksRef.current = likedTracks;
  }, [likedTracks]);

  // Scan & Import State
  const [isScanning, setIsScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState<ScanProgressEvent | null>(null);

  // UI Drawer & Modal States
  const urlParams = useMemo(() => new URLSearchParams(window.location.search), []);
  const [isExpandedPlayerOpen, setIsExpandedPlayerOpen] = useState(urlParams.get('expanded') === 'true');
  const [isQueueOpen, setIsQueueOpen] = useState(urlParams.get('queue') === 'true');
  const [isLyricsOpen, setIsLyricsOpen] = useState(urlParams.get('lyrics') === 'true');
  const [shutdownError, setShutdownError] = useState<string | null>(null);
  const [isRetryingShutdown, setIsRetryingShutdown] = useState(false);

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
  const [density, setDensity] = useState<'standard' | 'compact'>('standard');
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

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((toast) => toast.id !== id));
  }, []);

  const pushToast = useCallback(
    (kind: ToastKind, message: string) => {
      const id = ++toastIdRef.current;
      setToasts((prev) => [...prev, { id, kind, message }]);
      setTimeout(() => dismissToast(id), 4000);
    },
    [dismissToast]
  );

  const showPlaybackError = useCallback(
    (message: string) => {
      pushToast('error', message);
      setPlayback((prev) => ({ ...prev, errorMessage: message }));
    },
    [pushToast]
  );

  // Reusable loader for library collections from Rust SQLite
  const refreshLibrary = useCallback(async () => {
    if (!isNativeApp()) return;
    try {
      const [nextTracks, nextAlbums, nextArtists, nextPlaylists, nextLiked] =
        await Promise.all([
          getTracks(),
          getAlbums(),
          getArtists(),
          getPlaylists(),
          getLikedTracks(),
        ]);
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
      } catch (err) {
        applyLikeState(trackId, wasLiked);
        pushToast('error', err instanceof Error ? err.message : String(err));
      }
    },
    [applyLikeState, pushToast]
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

  const handleOpenAddToPlaylist = useCallback((track: Track) => {
    setAddToPlaylistData({
      isOpen: true,
      tracks: [track],
      contextLabel: `Track: ${track.title}`,
    });
  }, []);

  const handleOpenAddAlbumToPlaylist = useCallback(
    (album: Album) => {
      const albumTracks = tracks.filter(
        (track) =>
          track.album.toLowerCase() === album.title.toLowerCase() &&
          (track.albumArtist?.toLowerCase() === album.artist.toLowerCase() ||
            track.artist.toLowerCase() === album.artist.toLowerCase())
      );
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

  const handlePlayTrack = useCallback(
    async (trackId: string) => {
      try {
        const ids = tracksRef.current.map((track) => track.id);
        if (!ids.includes(trackId)) return;
        await setQueue(ids, trackId);
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
        if (shuffle) await setShuffle(true);
        const startId = tracksToPlay[startIndex]?.id;
        await setQueue(
          tracksToPlay.map((track) => track.id),
          startId
        );
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
      const albumTracks = tracksRef.current.filter(
        (track) =>
          track.album.toLowerCase() === album.title.toLowerCase() &&
          (track.albumArtist?.toLowerCase() === album.artist.toLowerCase() ||
            track.artist.toLowerCase() === album.artist.toLowerCase())
      );
      void handlePlayTracks(albumTracks, 0, false);
    },
    [handlePlayTracks]
  );

  const handlePlayArtist = useCallback(
    (artist: Artist) => {
      const artistTracks = tracksRef.current.filter(
        (track) =>
          track.artist.toLowerCase() === artist.name.toLowerCase() ||
          track.albumArtist?.toLowerCase() === artist.name.toLowerCase()
      );
      void handlePlayTracks(artistTracks, 0, false);
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

  const [routeAnnouncement, setRouteAnnouncement] = useState('');

  useEffect(() => {
    document.title = `Cavia Music — ${pageLabels[page]}`;
    setRouteAnnouncement(`Navigated to ${pageLabels[page]}`);
  }, [page]);

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
    [showPlaybackError]
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
    async (trackId: string) => {
      try {
        await playTrack(trackId);
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
    { label: 'Library', page: 'library' as const, icon: Library },
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
            const isActive = page === item.page;
            return (
              <button
                key={item.page}
                type="button"
                className={`nav-item ${isActive ? 'active' : ''}`}
                aria-current={isActive ? 'page' : undefined}
                onClick={() => setPage(item.page)}
              >
                <Icon size={17} strokeWidth={1.8} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
      </aside>

      <main id="main-content" className="cavia-main-shell">
        <header className="cavia-topbar">
          <span className="topbar-page-title">{pageLabels[page]}</span>

          <div className="topbar-actions">
            <span
              className={`core-status-dot status-${statusTone}`}
              title={statusLabel}
              aria-label={statusLabel}
              role="img"
            />
            <IconButton
              label="Import music folder"
              icon={<FolderPlus size={17} />}
              onClick={handleImportFolder}
              disabled={isScanning}
              title={isScanning ? 'Import already running' : 'Import music folder'}
            />
          </div>
        </header>

        {isScanning && scanProgress && (
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

        <div className="cavia-toast-stack" aria-live="polite">
          {toasts.map((toast) => (
            <div
              key={toast.id}
              className={`cavia-toast toast-${toast.kind}`}
              role={toast.kind === 'error' ? 'alert' : 'status'}
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

        <div className="cavia-page-scroll" key={page} tabIndex={-1}>
          {page === 'library' && (
            <LibraryView
              connectionError={connectionError}
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
              onAddAlbumToPlaylist={handleOpenAddAlbumToPlaylist}
              onPlayNext={handlePlayNext}
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
              allTracks={tracks}
              currentTrackId={playback.currentTrack?.id}
              isPlaying={playback.status === 'playing'}
            />
          )}

          {page === 'settings' && (
            <SettingsView
              health={health}
              density={density}
              onDensityChange={setDensity}
              motion={motion}
              onMotionChange={setMotion}
            />
          )}
        </div>

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
        />
      </main>

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

      <LyricsDrawer
        isOpen={isLyricsOpen}
        onClose={() => setIsLyricsOpen(false)}
        currentTrack={playback.currentTrack}
        playbackPositionMs={playback.positionMs}
        trackDurationMs={playback.durationMs}
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
          const isActive = page === item.page;
          return (
            <button
              key={item.page}
              type="button"
              className={`mobile-nav-btn ${isActive ? 'active' : ''}`}
              aria-current={isActive ? 'page' : undefined}
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
