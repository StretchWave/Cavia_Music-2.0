import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import { Check, Heart, ListPlus, ListStart, MoreHorizontal, Play, Trash2 } from 'lucide-react';
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
  onDeleteTrack?: (track: Track) => void;
  onRemoveFromHistory?: (trackId: string) => void;
  ariaLabel?: string;

  // Manage mode & multi-selection
  manageMode?: boolean;
  selectedTrackIds?: Set<string>;
  onToggleSelectTrack?: (trackId: string, event?: React.MouseEvent) => void;
  onRangeSelectTracks?: (trackId: string) => void;
  onEnterManageMode?: () => void;
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
  onDeleteTrack,
  onRemoveFromHistory,
  ariaLabel = 'Tracks',
  manageMode = false,
  selectedTrackIds,
  onToggleSelectTrack,
  onRangeSelectTracks,
  onEnterManageMode,
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
          <span className="track-col-index">{manageMode ? '' : '#'}</span>
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
        const isSelected = selectedTrackIds ? selectedTrackIds.has(track.id) : false;

        return (
          <div
            key={track.id}
            className={`track-row ${manageMode ? 'track-row-manage' : ''} ${isCurrent ? 'active' : ''} ${isSelected ? 'row-selected' : ''}`}
          >
            {manageMode && (
              <button
                type="button"
                role="checkbox"
                aria-checked={isSelected}
                aria-label={`Select ${track.title}`}
                className={`track-select-checkbox ${isSelected ? 'checked' : ''}`}
                onClick={(event) => {
                  event.stopPropagation();
                  if (event.shiftKey) {
                    onRangeSelectTracks?.(track.id);
                  } else {
                    onToggleSelectTrack?.(track.id, event);
                  }
                }}
              >
                {isSelected && <Check size={11} strokeWidth={3} />}
              </button>
            )}

            <button
              type="button"
              className="track-row-main"
              aria-label={manageMode ? `Toggle selection for ${track.title}` : `Play ${track.title} by ${track.artist || 'Unknown Artist'}`}
              aria-current={isCurrent ? 'true' : undefined}
              onClick={(event: React.MouseEvent) => {
                if (manageMode) {
                  if (event.shiftKey) {
                    onRangeSelectTracks?.(track.id);
                  } else {
                    onToggleSelectTrack?.(track.id, event);
                  }
                } else if ((event.shiftKey || event.ctrlKey || event.metaKey) && onToggleSelectTrack) {
                  event.preventDefault();
                  event.stopPropagation();
                  onEnterManageMode?.();
                  if (event.shiftKey) {
                    onRangeSelectTracks?.(track.id);
                  } else {
                    onToggleSelectTrack?.(track.id, event);
                  }
                } else {
                  onPlayTrack?.(track.id);
                }
              }}
            >
              {!manageMode && (
                <span className="track-col-index" aria-hidden="true">
                  {isCurrent && isPlaying ? (
                    <span className="eq-bounce">
                      <span />
                      <span />
                      <span />
                    </span>
                  ) : (
                    <>
                      <span className="track-index-number">{track.trackNumber || index + 1}</span>
                      <span className="track-index-play">
                        <Play size={11} fill="currentColor" />
                      </span>
                    </>
                  )}
                </span>
              )}

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
                    disabled={!onToggleLike || track.sourceKind === 'provider'}
                    title={
                      track.sourceKind === 'provider'
                        ? 'Streamed tracks are not stored in your library'
                        : undefined
                    }
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
                    <span>
                      {track.sourceKind === 'provider'
                        ? 'Like (library only)'
                        : isLiked
                          ? 'Remove from Liked'
                          : 'Like'}
                    </span>
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

                  {onRemoveFromHistory && (
                    <button
                      type="button"
                      role="menuitem"
                      className="track-menu-item track-menu-item-danger"
                      onClick={() => {
                        closeMenu(true);
                        onRemoveFromHistory(track.id);
                      }}
                    >
                      <Trash2 size={15} />
                      <span>Remove from history</span>
                    </button>
                  )}

                  {onDeleteTrack && track.sourceKind !== 'provider' && (
                    <button
                      type="button"
                      role="menuitem"
                      className="track-menu-item track-menu-item-danger"
                      onClick={() => {
                        closeMenu(true);
                        onDeleteTrack(track);
                      }}
                    >
                      <Trash2 size={15} />
                      <span>Remove from library</span>
                    </button>
                  )}
                </div>
              )}
            </span>
          </div>
        );
      })}
    </div>
  );
}
