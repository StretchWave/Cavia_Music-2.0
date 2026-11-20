import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import {
  ChevronDown,
  ChevronUp,
  Disc3,
  GripVertical,
  ListMusic,
  Play,
  Shuffle,
  Trash2,
  X,
} from 'lucide-react';
import { IconButton } from '../../components/IconButton';
import { Button } from '../../components/Button';
import { Badge } from '../../components/Badge';
import type { Track } from '../../lib/types';

interface QueueDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  currentTrack: Track | null;
  queue: Track[];
  isPlaying?: boolean;
  status?: string;
  isBackendReady?: boolean;
  onClearQueue?: () => void;
  onRemoveTrack?: (id: string, index?: number) => void;
  onSelectTrack?: (id: string) => void;
  onShuffleQueue?: () => void;
  onReorderQueue?: (fromIndex: number, toIndex: number) => void;
}

export function QueueDrawer({
  isOpen,
  onClose,
  currentTrack,
  queue,
  isPlaying = false,
  status = 'idle',
  isBackendReady = true,
  onClearQueue,
  onRemoveTrack,
  onSelectTrack,
  onShuffleQueue,
  onReorderQueue,
}: QueueDrawerProps) {
  const [draggedIndex, setDraggedIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  const drawerRef = useRef<HTMLDivElement>(null);
  const previousActiveElementRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    previousActiveElementRef.current = document.activeElement as HTMLElement | null;
    document.body.style.overflow = 'hidden';

    const timer = setTimeout(() => {
      if (drawerRef.current) {
        const firstFocusable = drawerRef.current.querySelector<HTMLElement>(
          'button:not([disabled]), [tabindex]:not([tabindex="-1"])'
        );
        firstFocusable?.focus();
      }
    }, 50);

    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
        return;
      }

      // Trap focus
      if (e.key === 'Tab' && drawerRef.current) {
        const focusables = Array.from(
          drawerRef.current.querySelectorAll<HTMLElement>(
            'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])'
          )
        ).filter((el) => el.offsetParent !== null);

        if (focusables.length === 0) return;

        const first = focusables[0];
        const last = focusables[focusables.length - 1];

        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown);

    return () => {
      clearTimeout(timer);
      document.body.style.overflow = '';
      window.removeEventListener('keydown', handleKeyDown);
      if (previousActiveElementRef.current && typeof previousActiveElementRef.current.focus === 'function') {
        previousActiveElementRef.current.focus();
      }
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const formatDuration = (ms: number) => {
    const totalSecs = Math.floor(ms / 1000);
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  const handleItemKeyDown = (e: ReactKeyboardEvent, index: number) => {
    // Keyboard reorder: Alt + ArrowUp / Alt + ArrowDown
    if (e.altKey && e.key === 'ArrowUp') {
      e.preventDefault();
      if (index > 0) onReorderQueue?.(index, index - 1);
    } else if (e.altKey && e.key === 'ArrowDown') {
      e.preventDefault();
      if (index < queue.length - 1) onReorderQueue?.(index, index + 1);
    }
  };

  return (
    <div className="cavia-drawer-backdrop" onClick={onClose}>
      <div
        ref={drawerRef}
        role="dialog"
        aria-modal="true"
        aria-label="Play queue"
        className="cavia-drawer-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="sheet-drag-handle" aria-hidden="true" />

        {/* Header */}
        <div className="drawer-header">
          <div>
            <div className="drawer-title-row">
              <h2 className="drawer-title">Play Queue</h2>
              <Badge variant="neutral" size="sm">
                {queue.length} TRACKS
              </Badge>
            </div>
          </div>
          <IconButton
            label="Close queue"
            icon={<X size={18} />}
            variant="ghost"
            onClick={onClose}
          />
        </div>

        {/* Content */}
        <div className="drawer-body">
          {/* Currently Selected Track */}
          <div className="queue-section">
            <span className="queue-section-label">
              {isPlaying ? 'CURRENTLY PLAYING' : status === 'paused' ? 'CURRENTLY PAUSED' : 'CURRENT SELECTION'}
            </span>
            {currentTrack ? (
              <div className="queue-item active-queue-item">
                <div className="queue-item-art" aria-hidden="true">
                  <Disc3 size={18} className={isPlaying ? 'queue-spinning-art' : undefined} />
                </div>
                <div className="queue-item-meta">
                  <strong className="queue-item-title">{currentTrack.title}</strong>
                  <span className="queue-item-artist">{currentTrack.artist}</span>
                </div>
                <Badge variant={isPlaying ? 'vermilion' : 'neutral'} size="sm">
                  {isPlaying ? 'PLAYING' : status === 'paused' ? 'PAUSED' : 'STOPPED'}
                </Badge>
              </div>
            ) : (
              <div className="queue-empty-box">
                <span className="status-indicator-dot muted" aria-hidden="true" />
                <span>No track currently in playback buffer</span>
              </div>
            )}
          </div>

          {/* Up Next List */}
          <div className="queue-section">
            <div className="queue-section-header-row">
              <span className="queue-section-label">UP NEXT</span>
              {queue.length > 0 && (
                <div className="queue-header-actions">
                  <button
                    type="button"
                    className="queue-action-link"
                    title={!isBackendReady ? 'Audio engine unavailable' : 'Shuffle upcoming queue'}
                    disabled={!isBackendReady}
                    onClick={onShuffleQueue}
                    aria-label="Shuffle upcoming queue"
                  >
                    <Shuffle size={13} aria-hidden="true" />
                    <span>Shuffle</span>
                  </button>
                  <button
                    type="button"
                    className="queue-action-link danger"
                    onClick={onClearQueue}
                    aria-label="Clear all tracks from queue"
                  >
                    <Trash2 size={13} aria-hidden="true" />
                    <span>Clear</span>
                  </button>
                </div>
              )}
            </div>

            {queue.length > 0 ? (
              <ol className="queue-list" aria-label="Upcoming tracks in queue">
                {queue.map((track, index) => {
                  const isCurrent = currentTrack?.id === track.id;
                  const isDragging = draggedIndex === index;
                  const isDragOver = dragOverIndex === index;
                  return (
                    <li
                      key={`${track.id}-${index}`}
                      className={`queue-item ${isCurrent ? 'active-queue-item' : ''} ${
                        isDragging ? 'dragging' : ''
                      } ${isDragOver ? 'drag-over' : ''} ${!isBackendReady ? 'disabled' : ''}`}
                      draggable
                      onDragStart={(e) => {
                        setDraggedIndex(index);
                        e.dataTransfer.effectAllowed = 'move';
                        e.dataTransfer.setData('text/plain', String(index));
                      }}
                      onDragOver={(e) => {
                        e.preventDefault();
                        e.dataTransfer.dropEffect = 'move';
                        if (dragOverIndex !== index) {
                          setDragOverIndex(index);
                        }
                      }}
                      onDragLeave={() => {
                        if (dragOverIndex === index) {
                          setDragOverIndex(null);
                        }
                      }}
                      onDrop={(e) => {
                        e.preventDefault();
                        setDragOverIndex(null);
                        if (draggedIndex !== null && draggedIndex !== index) {
                          onReorderQueue?.(draggedIndex, index);
                        }
                        setDraggedIndex(null);
                      }}
                      onDragEnd={() => {
                        setDraggedIndex(null);
                        setDragOverIndex(null);
                      }}
                      onKeyDown={(e) => handleItemKeyDown(e, index)}
                    >
                      <span className="queue-drag-handle" title="Drag to reorder (or use Alt+Up/Down)" aria-hidden="true">
                        <GripVertical size={14} />
                      </span>
                      <span className="queue-index">{index + 1}</span>

                      {/* Explicit Play Button Trigger */}
                      <button
                        type="button"
                        className="queue-track-play-trigger"
                        onClick={() => {
                          if (isBackendReady) onSelectTrack?.(track.id);
                        }}
                        disabled={!isBackendReady}
                        title={!isBackendReady ? 'Audio engine unavailable' : `Play ${track.title}`}
                      >
                        <span className="queue-item-meta">
                          <strong className="queue-item-title">{track.title}</strong>
                          <span className="queue-item-artist">{track.artist}</span>
                        </span>
                        <span className="queue-time">{formatDuration(track.durationMs)}</span>
                      </button>

                      {/* Accessible Non-Drag Reorder Controls */}
                      <div className="queue-reorder-group" aria-label="Reorder track">
                        <button
                          type="button"
                          className="queue-reorder-btn"
                          disabled={index === 0}
                          onClick={() => onReorderQueue?.(index, index - 1)}
                          aria-label={`Move ${track.title} up`}
                          title="Move up (Alt+Up)"
                        >
                          <ChevronUp size={13} aria-hidden="true" />
                        </button>
                        <button
                          type="button"
                          className="queue-reorder-btn"
                          disabled={index === queue.length - 1}
                          onClick={() => onReorderQueue?.(index, index + 1)}
                          aria-label={`Move ${track.title} down`}
                          title="Move down (Alt+Down)"
                        >
                          <ChevronDown size={13} aria-hidden="true" />
                        </button>
                      </div>

                      {/* Remove Action */}
                      <button
                        type="button"
                        className="queue-remove-btn"
                        onClick={() => onRemoveTrack?.(track.id, index)}
                        aria-label={`Remove ${track.title} from queue`}
                        title="Remove track from queue"
                      >
                        <X size={14} aria-hidden="true" />
                      </button>
                    </li>
                  );
                })}
              </ol>
            ) : (
              <div className="queue-blank-slate">
                <div className="queue-blank-icon" aria-hidden="true">
                  <ListMusic size={24} />
                </div>
                <h3>Your queue is empty</h3>
                <p>
                  As you play music or add tracks and albums to queue, they will appear here in listening order.
                </p>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={onClose}
                  icon={<Play size={14} />}
                >
                  Browse Library
                </Button>
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="drawer-footer">
          <span className="drawer-footer-text">
            Queue persistence is authoritative in SQLite and native audio engine.
          </span>
        </div>
      </div>
    </div>
  );
}
