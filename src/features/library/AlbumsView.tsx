import { Disc3, FolderPlus } from 'lucide-react';
import { EmptyState } from '../../components/EmptyState';
import { Button } from '../../components/Button';
import { formatArtworkUri } from '../../lib/library';
import type { Album } from '../../lib/types';

interface AlbumsViewProps {
  albums: Album[];
  onImportFolder?: () => void;
  isScanning?: boolean;
}

export function AlbumsView({ albums, onImportFolder, isScanning }: AlbumsViewProps) {
  return (
    <section className="cavia-subpage albums-page">
      <div className="subpage-header">
        <div>
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>DISCOGRAPHY ARCHIVE</span>
          </div>
          <h1 className="subpage-title">
            Albums<span>.</span>
          </h1>
          <p className="subpage-subtitle">
            Long-form releases categorized by album metadata and year.
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

      {albums.length > 0 ? (
        <div className="albums-grid">
          {albums.map((album) => {
            const formattedArt = formatArtworkUri(album.artworkUri);
            return (
              <article key={album.id} className="album-card" tabIndex={0} role="button">
                <div className="album-sleeve-frame">
                  {formattedArt ? (
                    <img
                      src={formattedArt}
                      alt={album.title}
                      className="album-artwork-img"
                      loading="lazy"
                    />
                  ) : (
                    <div className="album-artwork-placeholder">
                      <Disc3 size={32} />
                      <span className="album-badge">ALBUM</span>
                    </div>
                  )}
                  <div className="album-shadow-sheen" />
                </div>
                <div className="album-meta">
                  <strong className="album-title">{album.title}</strong>
                  <span className="album-artist">{album.artist}</span>
                  <div className="album-submeta">
                    {album.year && <span>{album.year}</span>}
                    {album.year && <span>·</span>}
                    <span>{album.trackCount} {album.trackCount === 1 ? 'track' : 'tracks'}</span>
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      ) : (
        <EmptyState
          icon={<Disc3 size={28} />}
          glyph="盤"
          eyebrow="YOUR ALBUMS"
          title={<>No albums found<br /><em>in local archive.</em></>}
          description="Albums populate automatically as audio files with ID3, Vorbis, or FLAC tags are imported into your Cavia library."
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
          footnote="Lofty audio metadata parsing extracts album titles, artwork, and release years directly from files."
        />
      )}
    </section>
  );
}
