import {
  Disc3,
  FolderPlus,
  Library,
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
  onImportFolder?: () => void;
  isScanning?: boolean;
}

export function LibraryView({
  summary,
  health,
  connectionError,
  playlists,
  onNavigate,
  onOpenCreatePlaylist,
  onImportFolder,
  isScanning,
}: LibraryViewProps) {
  const hasTracks = (summary?.tracks ?? 0) > 0;

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
              onClick={onImportFolder}
              disabled={isScanning}
              title="Select a directory to scan and import audio files into your local library"
            >
              {isScanning ? 'Scanning Directory...' : 'Add Music Folders'}
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
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              onNavigate('playlists');
            }
          }}
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
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              onNavigate('liked');
            }
          }}
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
              <article
                key={pl.id}
                className="playlist-card"
                onClick={() => onNavigate('playlists')}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    onNavigate('playlists');
                  }
                }}
                tabIndex={0}
                role="button"
                aria-label={`Open playlist ${pl.name}`}
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
              </article>
            ))}
          </div>
        </section>
      )}

      {/* Empty State / Philosophy Banner or Active Archive Summary */}
      {!hasTracks ? (
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
      ) : (
        <section className="empty-library-showcase populated-showcase">
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
              <span>LOCAL MUSIC REPOSITORY ONLINE</span>
            </div>
            <h2 className="showcase-heading">
              Your sound archive is<br />
              <em>ready for listening.</em>
            </h2>
            <p className="showcase-description">
              {summary?.tracks.toLocaleString()} local recordings indexed directly in SQLite with high-precision tag extraction. Your files remain untouched and completely offline.
            </p>

            <div className="showcase-badges-row">
              <Button
                variant="primary"
                size="sm"
                onClick={() => onNavigate('tracks')}
              >
                Browse All Tracks
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onNavigate('albums')}
              >
                View Albums
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onNavigate('artists')}
              >
                View Artists
              </Button>
            </div>

            <div className="showcase-footnote">
              <span className="footnote-mark" />
              <span>Bit-perfect offline storage · No cloud telemetry.</span>
            </div>
          </div>
        </section>
      )}
    </div>
  );
}

