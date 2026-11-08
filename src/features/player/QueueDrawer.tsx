import { useEffect } from 'react';
import {
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
  onClearQueue?: () => void;
  onRemoveTrack?: (id: string) => void;
}

export function QueueDrawer({
  isOpen,
  onClose,
  currentTrack,
  queue,
  onClearQueue,
  onRemoveTrack,
}: QueueDrawerProps) {
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    }
    if (isOpen) {
      window.addEventListener('keydown', handleKeyDown);
    }
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const formatDuration = (ms: number) => {
    const totalSecs = Math.floor(ms / 1000);
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  return (
    <aside className="cavia-drawer-backdrop" onClick={onClose} role="dialog" aria-modal="true" aria-label="Play queue">
      <div className="cavia-drawer-panel" onClick={(e) => e.stopPropagation()}>
        <div className="sheet-drag-handle" aria-hidden="true" />
        {/* Header */}
        <div className="drawer-header">
          <div>
            <div className="drawer-eyebrow">PLAYLIST ORDER & BUFFER</div>
            <div className="drawer-title-row">
              <h2 className="drawer-title">Play Queue</h2>
              <Badge variant="neutral" size="sm">
                {queue.length + (currentTrack ? 1 : 0)} TRACKS
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
          {/* Currently Playing Track */}
          <div className="queue-section">
            <span className="queue-section-label">CURRENTLY PLAYING</span>
            {currentTrack ? (
              <div className="queue-item active-queue-item">
                <div className="queue-item-art">
                  <Disc3 size={18} className="queue-spinning-art" />
                </div>
                <div className="queue-item-meta">
                  <strong className="queue-item-title">{currentTrack.title}</strong>
                  <span className="queue-item-artist">{currentTrack.artist}</span>
                </div>
                <Badge variant="vermilion" size="sm">
                  PLAYING
                </Badge>
              </div>
            ) : (
              <div className="queue-empty-box">
                <span className="status-indicator-dot muted" />
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
                    title="Shuffle upcoming queue"
                  >
                    <Shuffle size={13} />
                    <span>Shuffle</span>
                  </button>
                  <button
                    type="button"
                    className="queue-action-link danger"
                    onClick={onClearQueue}
                  >
                    <Trash2 size={13} />
                    <span>Clear</span>
                  </button>
                </div>
              )}
            </div>

            {queue.length > 0 ? (
              <div className="queue-list">
                {queue.map((track, index) => (
                  <div key={`${track.id}-${index}`} className="queue-item">
                    <span className="queue-drag-handle" title="Drag to reorder">
                      <GripVertical size={14} />
                    </span>
                    <span className="queue-index">{index + 1}</span>
                    <div className="queue-item-meta">
                      <strong className="queue-item-title">{track.title}</strong>
                      <span className="queue-item-artist">{track.artist}</span>
                    </div>
                    <span className="queue-time">{formatDuration(track.durationMs)}</span>
                    <IconButton
                      label="Remove track from queue"
                      icon={<X size={14} />}
                      size="sm"
                      variant="ghost"
                      onClick={() => onRemoveTrack?.(track.id)}
                    />
                  </div>
                ))}
              </div>
            ) : (
              <div className="queue-blank-slate">
                <div className="queue-blank-icon">
                  <ListMusic size={24} />
                </div>
                <h3>Your queue is empty</h3>
                <p>
                  As you play music or add albums and playlists to queue, they will appear here in listening order.
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
            Queue persistence is maintained in Rust domain state (Milestone 3).
          </span>
        </div>
      </div>
    </aside>
  );
}

