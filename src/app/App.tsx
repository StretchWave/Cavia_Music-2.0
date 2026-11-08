import { useEffect, useMemo, useState } from 'react';
import {
  Album, AudioWaveform, ChevronDown, CircleHelp, Disc3,
  Headphones, Heart, Home, Library, ListMusic, Music2, Search, Settings2,
  ShieldCheck, SlidersHorizontal, Sparkles, WifiOff,
} from 'lucide-react';
import { getLibrarySummary, getPlatformHealth, isNativeApp, type LibrarySummary, type PlatformHealth } from '../lib/platform';
import { IconButton } from '../components/IconButton';
import { EmptyLibrary } from '../features/library/EmptyLibrary';
import { SearchView } from '../features/search/SearchView';
import { SettingsView } from '../features/settings/SettingsView';

type Page = 'library' | 'search' | 'playlists' | 'albums' | 'artists' | 'liked' | 'settings';

const pageLabels: Record<Page, string> = {
  library: 'Your library', search: 'Search', playlists: 'Playlists', albums: 'Albums',
  artists: 'Artists', liked: 'Liked tracks', settings: 'Settings',
};

export function App() {
  const [page, setPage] = useState<Page>('library');
  const [health, setHealth] = useState<PlatformHealth | null>(null);
  const [summary, setSummary] = useState<LibrarySummary | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);

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
    return () => { active = false; };
  }, []);

  const statusLabel = useMemo(() => {
    if (!isNativeApp()) return 'DESIGN PREVIEW';
    if (connectionError) return 'CORE UNAVAILABLE';
    if (!health) return 'CONNECTING';
    return health.database_connected ? 'LOCAL CORE READY' : 'DATABASE OFFLINE';
  }, [connectionError, health]);

  const nav = [
    { label: 'Library', page: 'library' as const, icon: Library },
    { label: 'Search', page: 'search' as const, icon: Search },
  ];
  const collections = [
    { label: 'Playlists', page: 'playlists' as const, icon: ListMusic },
    { label: 'Albums', page: 'albums' as const, icon: Disc3 },
    { label: 'Artists', page: 'artists' as const, icon: AudioWaveform },
    { label: 'Liked tracks', page: 'liked' as const, icon: Heart },
  ];

  return (
    <div className="app-frame">
      <aside className="sidebar">
        <div className="brand-lockup">
          <div className="brand-mark"><AudioWaveform size={21} strokeWidth={1.7} /></div>
          <div className="brand-copy"><span className="brand-name">cavia<span>.</span></span><span className="brand-edition">MUSIC / 02</span></div>
        </div>

        <div className="sidebar-scroll">
          <div className="nav-group">
            <div className="nav-caption">WORKSPACE</div>
            {nav.map((item) => {
              const Icon = item.icon;
              return <button key={item.page} className={`nav-item ${page === item.page ? 'active' : ''}`} onClick={() => setPage(item.page)}>
                <Icon size={17} strokeWidth={1.8} /><span>{item.label}</span>{page === item.page && <span className="active-marker" />}
              </button>;
            })}
          </div>

          <div className="nav-group collection-group">
            <div className="nav-caption nav-caption-row"><span>YOUR COLLECTION</span><span className="nav-count">{summary?.tracks ?? '—'}</span></div>
            {collections.map((item) => {
              const Icon = item.icon;
              return <button key={item.page} className={`nav-item ${page === item.page ? 'active' : ''}`} onClick={() => setPage(item.page)}>
                <Icon size={17} strokeWidth={1.8} /><span>{item.label}</span>{page === item.page && <span className="active-marker" />}
              </button>;
            })}
          </div>
        </div>

        <div className="sidebar-bottom">
          <div className="integrity-card">
            <div className="integrity-icon"><ShieldCheck size={15} /></div>
            <div><strong>Local-first by design</strong><span>Your library stays yours.</span></div>
          </div>
          <button className={`nav-item ${page === 'settings' ? 'active' : ''}`} onClick={() => setPage('settings')}>
            <Settings2 size={17} /><span>Settings</span>{page === 'settings' && <span className="active-marker" />}
          </button>
          <div className="sidebar-version">CAVIA MUSIC <span>2.0.0 FOUNDATION</span></div>
        </div>
      </aside>

      <main className="main-shell">
        <header className="topbar">
          <div className="breadcrumbs"><span> CAVIA </span><span className="crumb-slash">/</span><strong>{pageLabels[page].toUpperCase()}</strong></div>
          <div className="topbar-actions">
            <div className={`core-status ${connectionError ? 'status-error' : ''}`}><span className="status-dot" />{statusLabel}</div>
            <span className="topbar-divider" />
            <IconButton label="Help and project status" icon={<CircleHelp size={17} />} disabled title="Project documentation is being prepared" />
            <button className="profile-button" aria-label="Guest profile"><span className="profile-avatar">G</span><span className="profile-name">GUEST</span><ChevronDown size={13} /></button>
          </div>
        </header>

        <div className="page-scroll" key={page}>
          {page === 'library' && <>
            <section className="page-heading">
              <div><div className="eyebrow"><span className="eyebrow-line" />THE PERSONAL ARCHIVE</div><h1>Your sound.<br /><em>Your space.</em></h1><p className="heading-description">A quieter place for the music that stays with you.</p></div>
              <div className="heading-kanji" aria-hidden="true">藏</div>
            </section>
            <section className="library-summary">
              <div className="summary-main"><div className="summary-icon"><Library size={19} /></div><div><span className="summary-label">YOUR COLLECTION</span><strong>{summary ? summary.tracks.toLocaleString() : '—'} <small>tracks</small></strong></div></div>
              <div className="summary-separator" />
              <div className="summary-stat"><span>PLAYLISTS</span><strong>{summary?.playlists ?? '—'}</strong></div>
              <div className="summary-stat"><span>LIKED TRACKS</span><strong>{summary?.liked_tracks ?? '—'}</strong></div>
              <div className="summary-note"><Sparkles size={14} /><span>Made for listening, not scrolling.</span></div>
            </section>
            {connectionError && <div className="inline-error"><WifiOff size={15} /><span>Could not connect to the local core: {connectionError}</span></div>}
            <EmptyLibrary />
          </>}
          {page === 'search' && <SearchView />}
          {page === 'settings' && <SettingsView health={health} />}
          {['playlists', 'albums', 'artists', 'liked'].includes(page) && <CollectionEmpty page={page} />}
        </div>

        <footer className="player-dock">
          <div className="dock-track"><div className="dock-art"><Music2 size={19} /></div><div className="dock-meta"><strong>Nothing playing</strong><span>Your next favourite starts here.</span></div></div>
          <div className="dock-center"><div className="dock-controls"><IconButton label="Shuffle" icon={<SlidersHorizontal size={16} />} disabled title="Playback engine is not connected yet" /><IconButton label="Previous track" icon={<span className="control-glyph">|◀</span>} disabled title="Playback engine is not connected yet" /><button className="play-button" disabled aria-label="Play"><span /></button><IconButton label="Next track" icon={<span className="control-glyph">▶|</span>} disabled title="Playback engine is not connected yet" /><IconButton label="Queue" icon={<ListMusic size={16} />} disabled title="Queue becomes available with the playback engine" /></div><div className="dock-progress"><span>0:00</span><div className="progress-track"><div /></div><span>0:00</span></div></div>
          <div className="dock-right"><span className="engine-hint"><span className="status-dot muted" />AUDIO ENGINE NOT CONNECTED</span><IconButton label="Volume" icon={<Headphones size={17} />} disabled title="Playback engine is not connected yet" /></div>
        </footer>
      </main>

      <nav className="mobile-nav">
        {[{ page: 'library' as const, icon: Home, label: 'Home' }, { page: 'search' as const, icon: Search, label: 'Search' }, { page: 'liked' as const, icon: Heart, label: 'Liked' }, { page: 'settings' as const, icon: Settings2, label: 'Settings' }].map((item) => {
          const Icon = item.icon;
          return <button key={item.page} className={page === item.page ? 'mobile-active' : ''} onClick={() => setPage(item.page)}><Icon size={19} /><span>{item.label}</span></button>;
        })}
      </nav>
    </div>
  );
}

function CollectionEmpty({ page }: { page: Page }) {
  const icon = page === 'playlists' ? <ListMusic size={22} /> : page === 'albums' ? <Album size={22} /> : page === 'artists' ? <AudioWaveform size={22} /> : <Heart size={22} />;
  return <section className="simple-empty"><div className="simple-empty-icon">{icon}</div><div className="eyebrow">COLLECTION / {page.toUpperCase()}</div><h2>This space is waiting<br />for your music.</h2><p>Nothing is pre-filled. Your collection will be built from music you add.</p></section>;
}
