import { AudioWaveform, FolderPlus } from 'lucide-react';
import { EmptyState } from '../../components/EmptyState';
import { Button } from '../../components/Button';
import type { Artist } from '../../lib/types';

interface ArtistsViewProps {
  artists: Artist[];
  onImportFolder?: () => void;
  isScanning?: boolean;
}

export function ArtistsView({ artists, onImportFolder, isScanning }: ArtistsViewProps) {
  return (
    <section className="cavia-subpage artists-page">
      <div className="subpage-header">
        <div>
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>CREATORS & PERFORMERS</span>
          </div>
          <h1 className="subpage-title">
            Artists<span>.</span>
          </h1>
          <p className="subpage-subtitle">
            All musicians, producers, and performers represented in your collection.
          </p>
        </div>

        <div className="subpage-actions">
          <Button
            variant="outline"
            size="md"
            icon={<FolderPlus size={16} />}
            onClick={onImportFolder}
            disabled={isScanning}
          >
            {isScanning ? 'Scanning...' : 'Import Folder'}
          </Button>
        </div>
      </div>

      {artists.length > 0 ? (
        <div className="artists-grid">
          {artists.map((artist) => (
            <article key={artist.id} className="artist-card" tabIndex={0} role="button">
              <div className="artist-avatar-frame">
                <span className="artist-monogram">
                  {artist.name.slice(0, 2).toUpperCase()}
                </span>
                <div className="artist-avatar-halo" />
              </div>
              <div className="artist-meta">
                <strong className="artist-name">{artist.name}</strong>
                <span className="artist-stats">
                  {artist.albumCount} {artist.albumCount === 1 ? 'album' : 'albums'} · {artist.trackCount} {artist.trackCount === 1 ? 'track' : 'tracks'}
                </span>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <EmptyState
          icon={<AudioWaveform size={28} />}
          glyph="奏"
          eyebrow="YOUR ARTISTS"
          title={<>No artists indexed<br /><em>in local archive.</em></>}
          description="Artists are grouped automatically as you add music to your collection. Artist discographies, release groupings, and metadata are indexed locally."
          actions={
            <Button
              variant="outline"
              size="md"
              icon={<FolderPlus size={16} />}
              onClick={onImportFolder}
              disabled={isScanning}
            >
              {isScanning ? 'Scanning Directory...' : 'Scan Audio Folders'}
            </Button>
          }
          footnote="Cavia never modifies or renames your original audio files during indexing."
        />
      )}
    </section>
  );
}
