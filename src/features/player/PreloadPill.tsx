import { useEffect, useRef, useState } from 'react';
import {
  AlertCircle,
  CheckCircle2,
  Clock,
  DownloadCloud,
  Loader2,
  Minus,
  Plus,
  RefreshCw,
  WifiOff,
  X,
} from 'lucide-react';
import { Badge } from '../../components/Badge';
import { formatBytes } from '../../lib/artwork';
import { MAX_PRELOAD_TARGET } from '../../lib/connect';
import type { PreloadStatusSnapshot } from '../../lib/types';

interface PreloadPillProps {
  status: PreloadStatusSnapshot | null;
  onChangeTarget: (target: number) => void;
  onRetry?: () => void;
  placement?: 'dock' | 'expanded';
}

export function PreloadPill({
  status,
  onChangeTarget,
  onRetry,
  placement = 'dock',
}: PreloadPillProps) {
  const [isOpen, setIsOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen) return;
    const handlePointerDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        setIsOpen(false);
      }
    };
    window.addEventListener('mousedown', handlePointerDown);
    window.addEventListener('keydown', handleKey);
    return () => {
      window.removeEventListener('mousedown', handlePointerDown);
      window.removeEventListener('keydown', handleKey);
    };
  }, [isOpen]);

  // Only show in Now Playing when preload is enabled (> 0).
  if (!status || !status.enabled || status.targetCount <= 0) {
    return null;
  }

  const {
    targetCount,
    readyCount,
    isDownloading,
    pausedForPlayback,
    offlinePaused,
    activeTitle,
    cacheUsedBytes,
    cacheLimitBytes,
    items,
    lateItems,
  } = status;

  const usagePercent =
    cacheLimitBytes > 0
      ? Math.min(100, Math.round((cacheUsedBytes / cacheLimitBytes) * 100))
      : 0;

  return (
    <div
      ref={rootRef}
      className={`preload-pill-wrap placement-${placement}`}
    >
      <div
        className={`preload-compact-pill ${isDownloading ? 'is-downloading' : ''} ${
          offlinePaused ? 'is-offline' : ''
        }`}
        role="group"
        aria-label={`Queue preload readiness: ${readyCount} of ${targetCount} tracks ready`}
      >
        <button
          type="button"
          className="preload-step-btn"
          onClick={() => onChangeTarget(Math.max(0, targetCount - 1))}
          aria-label="Decrease preload track count"
          title={targetCount <= 1 ? 'Set to 0 to disable preload' : 'Decrease preload target'}
        >
          <Minus size={11} aria-hidden="true" />
        </button>

        <button
          type="button"
          className="preload-main-trigger"
          onClick={() => setIsOpen((v) => !v)}
          aria-expanded={isOpen}
          aria-haspopup="dialog"
          title="Inspect upcoming preload readiness"
        >
          {offlinePaused ? (
            <WifiOff size={12} className="preload-icon-muted" aria-hidden="true" />
          ) : isDownloading ? (
            <Loader2 size={12} className="spin-accent preload-icon-active" aria-hidden="true" />
          ) : (
            <DownloadCloud size={12} className="preload-icon-ready" aria-hidden="true" />
          )}
          <span className="preload-count-text">
            {readyCount}/{targetCount} ready
          </span>
        </button>

        <button
          type="button"
          className="preload-step-btn"
          disabled={targetCount >= MAX_PRELOAD_TARGET}
          onClick={() => onChangeTarget(Math.min(MAX_PRELOAD_TARGET, targetCount + 1))}
          aria-label="Increase preload track count"
          title={
            targetCount >= MAX_PRELOAD_TARGET
              ? `Maximum preload target is ${MAX_PRELOAD_TARGET}`
              : 'Increase preload target'
          }
        >
          <Plus size={11} aria-hidden="true" />
        </button>
      </div>

      {isOpen && (
        <div
          className="preload-popover"
          role="dialog"
          aria-label="Queue preload status"
        >
          <div className="preload-popover-head">
            <div>
              <strong>Queue Preload</strong>
              <span>
                {readyCount}/{targetCount} verified in cache
              </span>
            </div>
            <div className="preload-popover-actions">
              {(offlinePaused || items.some((i) => i.status === 'failed')) && onRetry && (
                <button
                  type="button"
                  className="preload-icon-action"
                  onClick={onRetry}
                  title="Retry preload downloads"
                  aria-label="Retry preload downloads"
                >
                  <RefreshCw size={13} />
                </button>
              )}
              <button
                type="button"
                className="preload-icon-action"
                onClick={() => setIsOpen(false)}
                aria-label="Close preload details"
              >
                <X size={13} />
              </button>
            </div>
          </div>

          <div className="preload-popover-state">
            {offlinePaused ? (
              <span className="preload-state-note warn">
                Connectivity unavailable — preloading paused cleanly.
              </span>
            ) : pausedForPlayback ? (
              <span className="preload-state-note">
                Yielded while resolving active track for playback.
              </span>
            ) : isDownloading && activeTitle ? (
              <span className="preload-state-note active">
                Preparing: {activeTitle}
              </span>
            ) : items.length === 0 ? (
              <span className="preload-state-note">
                No upcoming tracks queued to preload.
              </span>
            ) : (
              <span className="preload-state-note">
                Sequential cache buffer active ({items.length} in window).
              </span>
            )}
          </div>

          <div className="preload-cache-meter">
            <div className="preload-cache-labels">
              <span>Stream cache</span>
              <span>
                {formatBytes(cacheUsedBytes)} / {formatBytes(cacheLimitBytes)}
              </span>
            </div>
            <div className="preload-cache-track">
              <div
                className="preload-cache-fill"
                style={{ width: `${usagePercent}%` }}
              />
            </div>
          </div>

          {items.length > 0 && (
            <ul className="preload-item-list">
              {items.map((item) => (
                <li key={item.queueEntryId} className="preload-item-row">
                  <span className="preload-item-icon" aria-hidden="true">
                    {item.status === 'ready' ? (
                      <CheckCircle2 size={13} className="text-ready" />
                    ) : item.status === 'downloading' ? (
                      <Loader2 size={13} className="spin-accent text-vermilion" />
                    ) : item.status === 'failed' ? (
                      <AlertCircle size={13} className="text-error" />
                    ) : (
                      <Clock size={13} className="text-muted" />
                    )}
                  </span>
                  <div className="preload-item-meta">
                    <strong>{item.title}</strong>
                    <span>
                      {item.artist}
                      {item.contributorName ? ` · ${item.contributorName}` : ''}
                    </span>
                  </div>
                  <Badge
                    variant={
                      item.status === 'ready'
                        ? 'ready'
                        : item.status === 'downloading'
                          ? 'vermilion'
                          : 'neutral'
                    }
                    size="sm"
                  >
                    {item.status === 'ready'
                      ? 'READY'
                      : item.status === 'downloading'
                        ? 'LOADING'
                        : item.status === 'offline_paused'
                          ? 'OFFLINE'
                          : item.status === 'failed'
                            ? 'ERROR'
                            : 'QUEUED'}
                  </Badge>
                </li>
              ))}
            </ul>
          )}

          {lateItems.length > 0 && (
            <div className="preload-late-section">
              <span className="preload-late-heading">
                Pending verification before entering queue ({lateItems.length})
              </span>
              <ul className="preload-item-list">
                {lateItems.map((item) => (
                  <li key={item.queueEntryId} className="preload-item-row">
                    <span className="preload-item-icon" aria-hidden="true">
                      <Loader2 size={12} className="text-muted" />
                    </span>
                    <div className="preload-item-meta">
                      <strong>{item.title}</strong>
                      <span>{item.contributorName ?? item.artist}</span>
                    </div>
                    <Badge variant="neutral" size="sm">PENDING</Badge>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
