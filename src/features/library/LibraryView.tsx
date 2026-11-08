import {
  Album,
  AudioWaveform,
  Disc3,
  FolderPlus,
  Heart,
  Library,
  ListMusic,
  Plus,
  Sparkles,
  WifiOff,
} from 'lucide-react';
import { Button } from '../../components/Button';
import { Badge } from '../../components/Badge';
import type { LibrarySummary, PlatformHealth } from '../../lib/platform';
import type { Page, Playlist } from '../../lib/types';

interface LibraryViewProps {
  summary: LibrarySummary | null;
  health: PlatformHealth | null;
  connectionError: string | null;
  playlists: Playlist[];
  onNavigate: (page: Page) => void;
  onOpenCreatePlaylist: () => void;
}

export function LibraryView({
  summary,
  health,
  connectionError,
  playlists,
  onNavigate,
  onOpenCreatePlaylist,
}: LibraryViewProps) {
  return (
    <div className="cavia-library-page">
      {/* Editorial Hero Header */}
      <section className="library-hero-section">
        <div className="hero-copy-block">
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>THE PERSONAL ARCHIVE</span>
          </div>
          <h1 className="hero-main-title">
            Your sound.<br />
            <em>Your space.</em>
          </h1>
          <p className="hero-subtext">
            A quieter, deliberate place for the music that stays with you.
          </p>

          <div className="hero-quick-actions">
            <Button
              variant="primary"
              size="md"
              icon={<FolderPlus size={16} />}
              disabled
              title="Native file staging & Lofty metadata scanning are planned for Milestone 2"
            >
              Add Music Folders <span className="coming-tag">MILESTONE 2</span>
            </Button>

            <Button
              variant="outline"
              size="md"
              icon={<Plus size={16} />}
              onClick={onOpenCreatePlaylist}
            >
              New Playlist
            </Button>
          </div>
        </div>

        <div className="hero-kanji-watermark" aria-hidden="true">
          藏
        </div>
      </section>

      {/* Collection Stats Strip */}
      <section className="collection-stats-strip" aria-label="Collection summary">
        <div className="stat-primary-cell">
          <div className="stat-icon-frame">
            <Library size={20} />
          </div>
          <div className="stat-cell-meta">
            <span className="stat-cell-label">TOTAL INDEXED</span>
            <strong className="stat-cell-value">
              {summary ? summary.tracks.toLocaleString() : '0'}{' '}
              <small>tracks</small>
            </strong>
          </div>
        </div>

        <div className="stat-strip-divider" />

        <div
          className="stat-interactive-cell"
          onClick={() => onNavigate('playlists')}
          tabIndex={0}
          role="button"
          aria-label="View playlists"
        >
          <span className="stat-cell-label">PLAYLISTS</span>
          <strong className="stat-cell-value">{summary?.playlists ?? playlists.length}</strong>
        </div>

        <div className="stat-strip-divider" />

        <div
          className="stat-interactive-cell"
          onClick={() => onNavigate('liked')}
          tabIndex={0}
          role="button"
          aria-label="View liked tracks"
        >
          <span className="stat-cell-label">LIKED SONGS</span>
          <strong className="stat-cell-value">{summary?.liked_tracks ?? '0'}</strong>
        </div>

        <div className="stat-strip-divider" />

        <div className="stat-status-cell">
          <Badge
            variant={health?.database_connected ? 'ready' : 'warn'}
            size="sm"
          >
            {health?.database_connected ? 'SQLITE ONLINE' : 'DATABASE STANDBY'}
          </Badge>
          <span className="stat-hint-text">
            <Sparkles size={13} />
            <span>Local-first · Zero cloud trackers</span>
          </span>
        </div>
      </section>

      {/* Connection error banner if any */}
      {connectionError && (
        <div className="cavia-banner banner-error">
          <WifiOff size={16} />
          <span>Local core communication issue: {connectionError}</span>
        </div>
      )}

      {/* Navigation Filter Shelf */}
      <section className="library-shelf-nav">
        <div className="shelf-chips">
          <button
            type="button"
            className="shelf-chip active"
            onClick={() => onNavigate('library')}
          >
            <Library size={14} />
            <span>Overview</span>
          </button>
          <button
            type="button"
            className="shelf-chip"
            onClick={() => onNavigate('tracks')}
          >
            <Disc3 size={14} />
            <span>Tracks</span>
          </button>
          <button
            type="button"
            className="shelf-chip"
            onClick={() => onNavigate('playlists')}
          >
            <ListMusic size={14} />
            <span>Playlists ({playlists.length})</span>
          </button>
          <button
            type="button"
            className="shelf-chip"
            onClick={() => onNavigate('albums')}
          >
            <Album size={14} />
            <span>Albums</span>
          </button>
          <button
            type="button"
            className="shelf-chip"
            onClick={() => onNavigate('artists')}
          >
            <AudioWaveform size={14} />
            <span>Artists</span>
          </button>
          <button
            type="button"
            className="shelf-chip"
            onClick={() => onNavigate('liked')}
          >
            <Heart size={14} />
            <span>Liked</span>
          </button>
        </div>
      </section>

      {/* Custom Playlists Spotlight if created */}
      {playlists.length > 0 && (
        <section className="library-section">
          <div className="section-title-row">
            <h2>Your Local Playlists</h2>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onNavigate('playlists')}
            >
              View all ({playlists.length})
            </Button>
          </div>

          <div className="playlists-grid">
            {playlists.slice(0, 3).map((pl) => (
              <div
                key={pl.id}
                className="playlist-card"
                onClick={() => onNavigate('playlists')}
                tabIndex={0}
                role="button"
              >
                <div className="playlist-card-art">
                  <Disc3 size={32} />
                </div>
                <div className="playlist-card-body">
                  <h3 className="playlist-card-title">{pl.name}</h3>
                  <span className="playlist-track-badge">
                    {pl.trackCount} tracks
                  </span>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Empty State / Philosophy Banner */}
      <section className="empty-library-showcase">
        <div className="showcase-art-orbit" aria-hidden="true">
          <div className="orbit-ring ring-1" />
          <div className="orbit-ring ring-2" />
          <div className="orbit-ring ring-3" />
          <div className="vinyl-centerpiece">
            <div className="vinyl-groove g-1" />
            <div className="vinyl-groove g-2" />
            <div className="vinyl-groove g-3" />
            <div className="vinyl-label-disc">
              <span className="vinyl-label-kanji">藏</span>
              <span className="vinyl-hole" />
            </div>
          </div>
          <div className="ambient-glint glint-top" />
          <div className="ambient-glint glint-bottom" />
        </div>

        <div className="showcase-text-block">
          <div className="cavia-eyebrow">
            <Sparkles size={13} />
            <span>A BLANK CANVAS, NOT A BLANK EXPERIENCE</span>
          </div>
          <h2 className="showcase-heading">
            Your library is<br />
            <em>yours to shape.</em>
          </h2>
          <p className="showcase-description">
            Cavia doesn't invent a catalogue for you. Bring in your own music and build a personal collection that honors your taste. No artificial recommendation engines, no DRM lockdowns, no telemetry.
          </p>

          <div className="showcase-badges-row">
            <Badge variant="outline" size="sm">
              FLAC & MP3
            </Badge>
            <Badge variant="outline" size="sm">
              HI-RES READY
            </Badge>
            <Badge variant="outline" size="sm">
              LOCAL SQLITE
            </Badge>
          </div>

          <div className="showcase-footnote">
            <span className="footnote-mark" />
            <span>No demo tracks. No fabricated playlists. Just your music.</span>
          </div>
        </div>
      </section>
    </div>
  );
}

