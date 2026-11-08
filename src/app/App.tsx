import { useEffect, useMemo, useState } from 'react';
import {
  Album,
  AudioWaveform,
  ChevronDown,
  CircleHelp,
  Disc3,
  Heart,
  Home,
  Library,
  ListMusic,
  Search,
  Settings2,
  ShieldCheck,
} from 'lucide-react';
import {
  getLibrarySummary,
  getPlatformHealth,
  isNativeApp,
  type LibrarySummary,
  type PlatformHealth,
} from '../lib/platform';
import type { Page, PlaybackState, Playlist, Track } from '../lib/types';
import { IconButton } from '../components/IconButton';
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

  // UI Drawer & Modal States
  const urlParams = useMemo(() => new URLSearchParams(window.location.search), []);
  const [isExpandedPlayerOpen, setIsExpandedPlayerOpen] = useState(urlParams.get('expanded') === 'true');
  const [isQueueOpen, setIsQueueOpen] = useState(urlParams.get('queue') === 'true');
  const [isLyricsOpen, setIsLyricsOpen] = useState(urlParams.get('lyrics') === 'true');

  // Appearance preferences
  const [density, setDensity] = useState<'standard' | 'compact'>('standard');

  // Playback state (honest initial state: audio backend is standby)
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

  // Local playlists (can be created by the user)
  const [playlists, setPlaylists] = useState<Playlist[]>([]);

  // Tracks, albums, artists from real state
  const tracks: Track[] = useMemo(() => [], []);
  const likedTracks: Track[] = useMemo(() => tracks.filter((t) => t.isLiked), [tracks]);

  useEffect(() => {
    let active = true;
    if (!isNativeApp()) return;

    Promise.all([getPlatformHealth(), getLibrarySummary()])
      .then(([nextHealth, nextSummary]) => {
        if (!active) return;
        setHealth(nextHealth);
        setSummary(nextSummary);
      })
      .catch((error: unknown) => {
        if (!active) return;
        setConnectionError(error instanceof Error ? error.message : String(error));
      });

    return () => {
      active = false;
    };
  }, []);

  const isBackendReady = health?.audio_backend === 'ready';

  const statusLabel = useMemo(() => {
    if (!isNativeApp()) return 'DESIGN PREVIEW';
    if (connectionError) return 'CORE UNAVAILABLE';
    if (!health) return 'CONNECTING';
    return health.database_connected ? 'LOCAL CORE READY' : 'DATABASE OFFLINE';
  }, [connectionError, health]);

  const handleCreatePlaylist = (name: string, description: string) => {
    const newPl: Playlist = {
      id: `pl-${Date.now()}`,
      name,
      description,
      trackCount: 0,
      createdAt: new Date().toISOString(),
      tracks: [],
    };
    setPlaylists((prev) => [newPl, ...prev]);
  };

  const nav = [
    { label: 'Archive', page: 'library' as const, icon: Library },
    { label: 'Search', page: 'search' as const, icon: Search },
  ];

  const collections = [
    { label: 'Tracks', page: 'tracks' as const, icon: Disc3, count: summary?.tracks ?? 0 },
    { label: 'Playlists', page: 'playlists' as const, icon: ListMusic, count: playlists.length },
    { label: 'Albums', page: 'albums' as const, icon: Album },
    { label: 'Artists', page: 'artists' as const, icon: AudioWaveform },
    { label: 'Liked Tracks', page: 'liked' as const, icon: Heart, count: summary?.liked_tracks ?? 0 },
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
              <span className="nav-count">{summary?.tracks ?? '0'}</span>
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
            />
          )}

          {page === 'tracks' && (
            <TracksView tracks={tracks} isBackendReady={isBackendReady} />
          )}

          {page === 'playlists' && (
            <PlaylistsView
              playlists={playlists}
              onCreatePlaylist={handleCreatePlaylist}
              isBackendReady={isBackendReady}
            />
          )}

          {page === 'albums' && <AlbumsView albums={[]} />}

          {page === 'artists' && <ArtistsView artists={[]} />}

          {page === 'liked' && (
            <LikedTracksView tracks={likedTracks} isBackendReady={isBackendReady} />
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
          onOpenExpanded={() => setIsExpandedPlayerOpen(true)}
          onOpenQueue={() => setIsQueueOpen(true)}
          onOpenLyrics={() => setIsLyricsOpen(true)}
        />
      </main>

      {/* Expanded Full-Screen Player Modal */}
      <ExpandedPlayer
        isOpen={isExpandedPlayerOpen}
        onClose={() => setIsExpandedPlayerOpen(false)}
        playback={playback}
        isBackendReady={isBackendReady}
        onOpenQueue={() => {
          setIsExpandedPlayerOpen(false);
          setIsQueueOpen(true);
        }}
        onOpenLyrics={() => {
          setIsExpandedPlayerOpen(false);
          setIsLyricsOpen(true);
        }}
      />

      {/* Slide-over Queue Drawer */}
      <QueueDrawer
        isOpen={isQueueOpen}
        onClose={() => setIsQueueOpen(false)}
        currentTrack={playback.currentTrack}
        queue={playback.queue}
        onClearQueue={() => setPlayback((p) => ({ ...p, queue: [] }))}
        onRemoveTrack={(id) =>
          setPlayback((p) => ({ ...p, queue: p.queue.filter((t) => t.id !== id) }))
        }
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
    </div>
  );
}
