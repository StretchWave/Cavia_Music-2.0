import { memo } from 'react';
import {
  AudioWaveform,
  Disc3,
  FolderPlus,
  Heart,
  Library,
  Music2,
  Plus,
  WifiOff,
} from 'lucide-react';
import { Button } from '../../components/Button';
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

export const LibraryView = memo(function LibraryView({
  summary,
  connectionError,
  playlists,
  onNavigate,
  onOpenCreatePlaylist,
  onImportFolder,
  isScanning,
}: LibraryViewProps) {
  const trackCount = summary?.tracks ?? 0;
  const hasTracks = trackCount > 0;

  return (
    <div className="cavia-library-page">
      {/* Editorial Hero Header */}
      <section className="library-hero-section">
        <div className="hero-copy-block">
          <h1 className="hero-main-title">
            Library<span>.</span>
          </h1>
          <p className="hero-subtext">
            {hasTracks
              ? `${trackCount.toLocaleString()} tracks in your personal collection.`
              : 'Add audio files from your local storage to begin.'}
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
        <button
          type="button"
          className="stat-primary-cell stat-interactive-cell"
          onClick={() => onNavigate('tracks')}
          aria-label={`View ${trackCount.toLocaleString()} tracks`}
        >
          <div className="stat-icon-frame">
            <Library size={20} />
          </div>
          <div className="stat-cell-meta">
            <span className="stat-cell-label">TOTAL INDEXED</span>
            <strong className="stat-cell-value">
              {trackCount.toLocaleString()}{' '}
              <small>tracks</small>
            </strong>
          </div>
        </button>

        <div className="stat-strip-divider" />

        <button
          type="button"
          className="stat-interactive-cell"
          onClick={() => onNavigate('playlists')}
          aria-label={`View ${summary?.playlists ?? playlists.length} playlists`}
        >
          <span className="stat-cell-label">PLAYLISTS</span>
          <strong className="stat-cell-value">{summary?.playlists ?? playlists.length}</strong>
        </button>

        <div className="stat-strip-divider" />

        <button
          type="button"
          className="stat-interactive-cell"
          onClick={() => onNavigate('liked')}
          aria-label={`View ${summary?.liked_tracks ?? 0} liked tracks`}
        >
          <span className="stat-cell-label">LIKED SONGS</span>
          <strong className="stat-cell-value">{summary?.liked_tracks ?? '0'}</strong>
        </button>
      </section>

      {/* Connection error banner if any */}
      {connectionError && (
        <div className="cavia-banner banner-error">
          <WifiOff size={16} />
          <span>Local core communication issue: {connectionError}</span>
        </div>
      )}

      {/* Main Browse Section */}
      {hasTracks ? (
        <>
          <section className="library-section" aria-label="Browse Categories">
            <div className="section-title-row">
              <h2>Browse Archive</h2>
            </div>

            <div className="library-browse-grid">
              <button
                type="button"
                className="library-browse-card"
                onClick={() => onNavigate('tracks')}
              >
                <div className="browse-card-icon">
                  <Music2 size={24} />
                </div>
                <div className="browse-card-content">
                  <span className="browse-card-title">Tracks</span>
                  <span className="browse-card-desc">All individual audio files</span>
                </div>
                <span className="browse-card-count">{trackCount.toLocaleString()}</span>
              </button>

              <button
                type="button"
                className="library-browse-card"
                onClick={() => onNavigate('albums')}
              >
                <div className="browse-card-icon">
                  <Disc3 size={24} />
                </div>
                <div className="browse-card-content">
                  <span className="browse-card-title">Albums</span>
                  <span className="browse-card-desc">Full releases & discographies</span>
                </div>
                <span className="browse-card-action">View &rarr;</span>
              </button>

              <button
                type="button"
                className="library-browse-card"
                onClick={() => onNavigate('artists')}
              >
                <div className="browse-card-icon">
                  <AudioWaveform size={24} />
                </div>
                <div className="browse-card-content">
                  <span className="browse-card-title">Artists</span>
                  <span className="browse-card-desc">Performers and creators</span>
                </div>
                <span className="browse-card-action">View &rarr;</span>
              </button>

              <button
                type="button"
                className="library-browse-card"
                onClick={() => onNavigate('liked')}
              >
                <div className="browse-card-icon">
                  <Heart size={24} />
                </div>
                <div className="browse-card-content">
                  <span className="browse-card-title">Liked Songs</span>
                  <span className="browse-card-desc">Pinned favorite recordings</span>
                </div>
                <span className="browse-card-count">{summary?.liked_tracks ?? 0}</span>
              </button>
            </div>
          </section>

          {/* Custom Playlists Spotlight if created */}
          {playlists.length > 0 && (
            <section className="library-section">
              <div className="section-title-row">
                <h2>Local Playlists</h2>
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
        </>
      ) : (
        <section className="empty-library-clean">
          <div className="empty-clean-card">
            <div className="empty-clean-glyph" aria-hidden="true">
              音
            </div>
            <h2>No tracks indexed yet</h2>
            <p>
              Cavia plays audio files directly from your local disks. Select a folder to index your music library.
            </p>
            <Button
              variant="primary"
              size="md"
              icon={<FolderPlus size={16} />}
              onClick={onImportFolder}
              disabled={isScanning}
            >
              {isScanning ? 'Scanning Directory...' : 'Add Music Folders'}
            </Button>
          </div>
        </section>
      )}
    </div>
  );
});
