import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import { Heart, ListPlus, ListStart, MoreHorizontal, Play } from 'lucide-react';
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
  const listRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);

  const closeMenu = (restoreFocus = false) => {
    setOpenMenuId(null);
    if (restoreFocus) {
      triggerRef.current?.focus();
    }
  };

  useEffect(() => {
    if (!openMenuId) return;

    // The menu mounts with this render; move focus into it once it exists.
    const focusTimer = setTimeout(() => {
      const firstItem = menuRef.current?.querySelector<HTMLButtonElement>(
        '[role="menuitem"]:not([disabled])'
      );
      firstItem?.focus();
    }, 0);

    function handlePointerDown(event: PointerEvent) {
      if (listRef.current && !listRef.current.contains(event.target as Node)) {
        setOpenMenuId(null);
      }
    }

    document.addEventListener('pointerdown', handlePointerDown);
    return () => {
      clearTimeout(focusTimer);
      document.removeEventListener('pointerdown', handlePointerDown);
    };
  }, [openMenuId]);

  const handleMenuKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const items = Array.from(
      menuRef.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not([disabled])') ?? []
    );

    if (event.key === 'Escape') {
      event.preventDefault();
      closeMenu(true);
      return;
    }

    if (event.key === 'Tab') {
      // Let focus leave the menu naturally, but do not leave it orphaned open.
      setOpenMenuId(null);
      return;
    }

    if (items.length === 0) return;

    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const offset = event.key === 'ArrowDown' ? 1 : -1;
      const current = items.indexOf(document.activeElement as HTMLButtonElement);
      const next = (current + offset + items.length) % items.length;
      items[next].focus();
      return;
    }

    if (event.key === 'Home') {
      event.preventDefault();
      items[0].focus();
    } else if (event.key === 'End') {
      event.preventDefault();
      items[items.length - 1].focus();
    }
  };

  return (
    <div
      ref={listRef}
      className={`track-list track-list-${density}`}
      role="group"
      aria-label={ariaLabel}
    >
      {density === 'standard' && (
        <div className="track-list-head" aria-hidden="true">
          <span className="track-col-index">#</span>
          <span className="track-col-title">Title</span>
          <span className="track-col-album">Album</span>
          <span className="track-col-duration">Time</span>
          <span className="track-col-menu" />
        </div>
      )}

      {tracks.map((track, index) => {
        const isCurrent = currentTrackId === track.id;
        const isMenuOpen = openMenuId === track.id;
        const isLiked = Boolean(track.isLiked);

        return (
          <div
            key={track.id}
            className={`track-row ${isCurrent ? 'active' : ''}`}
          >
            <button
              type="button"
              className="track-row-main"
              aria-label={`Play ${track.title} by ${track.artist || 'Unknown Artist'}`}
              aria-current={isCurrent ? 'true' : undefined}
              onClick={() => onPlayTrack?.(track.id)}
            >
              <span className="track-col-index" aria-hidden="true">
                {isCurrent && isPlaying ? (
                  <span className="eq-bounce">
                    <span />
                    <span />
                    <span />
                  </span>
                ) : (
                  <span className="track-index-number">{track.trackNumber || index + 1}</span>
                )}
                <span className="track-index-play">
                  <Play size={11} fill="currentColor" />
                </span>
              </span>

              <span className="track-col-title">
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
                <span className="track-col-album">{track.album || 'Unknown Album'}</span>
              )}

              <span className="track-col-duration">{formatTime(track.durationMs)}</span>
            </button>

            <span className="track-col-menu">
              <button
                type="button"
                className={`track-menu-trigger ${isMenuOpen ? 'open' : ''}`}
                aria-haspopup="menu"
                aria-expanded={isMenuOpen}
                aria-label={`Actions for ${track.title}`}
                ref={(node) => {
                  if (isMenuOpen) triggerRef.current = node;
                }}
                onClick={(event) => {
                  event.stopPropagation();
                  if (isMenuOpen) {
                    closeMenu(true);
                  } else {
                    triggerRef.current = event.currentTarget;
                    setOpenMenuId(track.id);
                  }
                }}
              >
                <MoreHorizontal size={16} />
              </button>

              {isMenuOpen && (
                <div
                  ref={menuRef}
                  className="track-context-menu"
                  role="menu"
                  aria-label={`Actions for ${track.title}`}
                  onKeyDown={handleMenuKeyDown}
                >
                  <button
                    type="button"
                    role="menuitem"
                    className="track-menu-item"
                    disabled={!onToggleLike}
                    onClick={() => {
                      onToggleLike?.(track.id);
                      closeMenu(true);
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
                      closeMenu(true);
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
                      closeMenu(true);
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
  );
}
