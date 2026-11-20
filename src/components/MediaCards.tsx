import { Disc3, Play } from 'lucide-react';
import { ArtworkImage } from './ArtworkImage';
import { formatArtworkUri } from '../lib/library';
import type { Album, Artist } from '../lib/types';

interface AlbumCardProps {
  album: Album;
  onOpen: (album: Album) => void;
  onPlay?: (album: Album) => void;
  playDisabled?: boolean;
}

export function AlbumCard({ album, onOpen, onPlay, playDisabled = false }: AlbumCardProps) {
  return (
    <article
      className="media-card"
      role="button"
      tabIndex={0}
      aria-label={`Open album ${album.title} by ${album.artist}`}
      onClick={() => onOpen(album)}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          onOpen(album);
        }
      }}
    >
      <div className="media-card-art">
        <ArtworkImage
          src={formatArtworkUri(album.artworkUri)}
          alt={album.title}
          className="media-card-image"
          fallbackIcon={<Disc3 size={26} />}
        />
        <button
          type="button"
          className="media-card-play"
          aria-label={`Play ${album.title}`}
          disabled={playDisabled}
          title={playDisabled ? 'Playback engine unavailable' : `Play ${album.title}`}
          onClick={(event) => {
            event.stopPropagation();
            onPlay?.(album);
          }}
        >
          <Play size={18} fill="currentColor" />
        </button>
      </div>
      <span className="media-card-label">{album.title}</span>
    </article>
  );
}

interface ArtistCardProps {
  artist: Artist;
  onOpen: (artist: Artist) => void;
  onPlay?: (artist: Artist) => void;
  playDisabled?: boolean;
}

export function ArtistCard({ artist, onOpen, onPlay, playDisabled = false }: ArtistCardProps) {
  const initials = artist.name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join('');

  return (
    <article
      className="media-card"
      role="button"
      tabIndex={0}
      aria-label={`Open artist ${artist.name}`}
      onClick={() => onOpen(artist)}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          onOpen(artist);
        }
      }}
    >
      <div className="media-card-art">
        {artist.artworkUri ? (
          <ArtworkImage
            src={formatArtworkUri(artist.artworkUri)}
            alt={artist.name}
            className="media-card-image"
          />
        ) : (
          <div className="media-card-monogram" aria-hidden="true">
            {initials || '—'}
          </div>
        )}
        <button
          type="button"
          className="media-card-play"
          aria-label={`Play ${artist.name}`}
          disabled={playDisabled}
          title={playDisabled ? 'Playback engine unavailable' : `Play ${artist.name}`}
          onClick={(event) => {
            event.stopPropagation();
            onPlay?.(artist);
          }}
        >
          <Play size={18} fill="currentColor" />
        </button>
      </div>
      <span className="media-card-label">{artist.name}</span>
    </article>
  );
}
