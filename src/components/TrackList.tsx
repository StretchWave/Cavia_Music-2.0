import { useEffect, useRef, useState } from 'react';
import { Heart, ListPlus, ListStart, MoreHorizontal } from 'lucide-react';
import { ArtworkImage } from './ArtworkImage';
import { formatArtworkUri } from '../lib/library';
import type { Track } from '../lib/types';

interface TrackListProps {
  tracks: Track[];
  density?: 'standard' | 'compact';
  currentTrackId?: string;
  isPlaying?: boolean;
  isBackendReady?: boolean;
  onPlayTrack?: (trackId: string) => void;
  onToggleLike?: (trackId: string) => void;
  onAddToPlaylist?: (track: Track) => void;
  onPlayNext?: (trackId: string) => void;
  ariaLabel?: string;
}

function formatTime(ms: number): string {
  if (!ms || ms <= 0) return '0:00';
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

export function TrackList({
  tracks,
  density = 'standard',
  currentTrackId,
  isPlaying = false,
  isBackendReady = true,
  onPlayTrack,
  onToggleLike,
  onAddToPlaylist,
  onPlayNext,
  ariaLabel = 'Tracks',
}: TrackListProps) {
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const menuHostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!openMenuId) return;

    function handlePointerDown(event: PointerEvent) {
      if (menuHostRef.current && !menuHostRef.current.contains(event.target as Node)) {
        setOpenMenuId(null);
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') setOpenMenuId(null);
    }

    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [openMenuId]);

  return (
    <div
      className={`track-list track-list-${density}`}
      role="table"
      aria-label={ariaLabel}
      ref={menuHostRef}
    >
      {density === 'standard' && (
        <div className="track-list-head" role="row">
          <span className="track-col-index" role="columnheader">#</span>
          <span className="track-col-title" role="columnheader">Title</span>
          <span className="track-col-album" role="columnheader">Album</span>
          <span className="track-col-duration" role="columnheader">Time</span>
          <span className="track-col-menu" role="columnheader" aria-label="Actions" />
        </div>
      )}

      <div role="rowgroup">
        {tracks.map((track, index) => {
          const isCurrent = currentTrackId === track.id;
          const isMenuOpen = openMenuId === track.id;
          const isLiked = Boolean(track.isLiked);

          return (
            <div
              key={track.id}
              className={`track-row ${isCurrent ? 'active' : ''}`}
              role="row"
              tabIndex={0}
              aria-label={`${track.title} by ${track.artist || 'Unknown Artist'}`}
              onClick={() => onPlayTrack?.(track.id)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onPlayTrack?.(track.id);
                }
              }}
            >
              {density === 'standard' && (
                <span className="track-col-index" role="cell">
                  {isCurrent && isPlaying ? (
                    <span className="eq-bounce" aria-label="Playing">
                      <span />
                      <span />
                      <span />
                    </span>
                  ) : (
                    <span className="track-index-number">{track.trackNumber || index + 1}</span>
                  )}
                </span>
              )}

              <span className="track-col-title" role="cell">
                {density === 'standard' && (
                  <ArtworkImage
                    src={track.artworkUri ? formatArtworkUri(track.artworkUri) : undefined}
                    alt=""
                    className="track-row-art"
                  />
                )}
                <span className="track-meta-cell">
                  <span className="track-title-text">{track.title}</span>
                  <span className="track-artist-text">{track.artist || 'Unknown Artist'}</span>
                </span>
              </span>

              {density === 'standard' && (
                <span className="track-col-album" role="cell">
                  {track.album || 'Unknown Album'}
                </span>
              )}

              <span className="track-col-duration" role="cell">
                {formatTime(track.durationMs)}
              </span>

              <span className="track-col-menu" role="cell">
                <button
                  type="button"
                  className={`track-menu-trigger ${isMenuOpen ? 'open' : ''}`}
                  aria-haspopup="menu"
                  aria-expanded={isMenuOpen}
                  aria-label={`Actions for ${track.title}`}
                  onClick={(event) => {
                    event.stopPropagation();
                    setOpenMenuId(isMenuOpen ? null : track.id);
                  }}
                >
                  <MoreHorizontal size={16} />
                </button>

                {isMenuOpen && (
                  <div
                    className="track-context-menu"
                    role="menu"
                    aria-label={`Actions for ${track.title}`}
                    onClick={(event) => event.stopPropagation()}
                  >
                    <button
                      type="button"
                      role="menuitem"
                      className="track-menu-item"
                      disabled={!onToggleLike}
                      onClick={() => {
                        onToggleLike?.(track.id);
                        setOpenMenuId(null);
                      }}
                    >
                      <Heart
                        size={15}
                        fill={isLiked ? 'var(--accent-vermilion)' : 'none'}
                        color={isLiked ? 'var(--accent-vermilion)' : 'currentColor'}
                      />
                      <span>{isLiked ? 'Remove from Liked' : 'Like'}</span>
                    </button>

                    <button
                      type="button"
                      role="menuitem"
                      className="track-menu-item"
                      disabled={!onAddToPlaylist}
                      onClick={() => {
                        onAddToPlaylist?.(track);
                        setOpenMenuId(null);
                      }}
                    >
                      <ListPlus size={15} />
                      <span>Add to Playlist</span>
                    </button>

                    <button
                      type="button"
                      role="menuitem"
                      className="track-menu-item"
                      disabled={!isBackendReady || !onPlayNext}
                      title={!isBackendReady ? 'Playback engine unavailable' : undefined}
                      onClick={() => {
                        onPlayNext?.(track.id);
                        setOpenMenuId(null);
                      }}
                    >
                      <ListStart size={15} />
                      <span>Play Next</span>
                    </button>
                  </div>
                )}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
