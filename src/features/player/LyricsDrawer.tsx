import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { usePlaybackPosition } from '../../lib/usePlaybackPosition';
import {
  AlertCircle,
  Disc,
  LocateFixed,
  Mic2,
  RefreshCw,
  X,
} from 'lucide-react';
import { IconButton } from '../../components/IconButton';
import { Badge } from '../../components/Badge';
import { Tabs } from '../../components/Tabs';
import { Button } from '../../components/Button';
import type { Track } from '../../lib/types';
import { getTrackLyrics, refreshTrackLyrics, type LyricsResult } from '../../lib/lyrics';

interface LyricsDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  currentTrack: Track | null;
  playbackPositionMs?: number;
  trackDurationMs?: number;
  onSeek?: (positionMs: number) => Promise<void> | void;
}

export function LyricsDrawer({
  isOpen,
  onClose,
  currentTrack,
  playbackPositionMs,
  trackDurationMs,
  onSeek,
}: LyricsDrawerProps) {
  const [lyrics, setLyrics] = useState<LyricsResult | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<'synced' | 'plain'>('synced');
  const [isFollowing, setIsFollowing] = useState<boolean>(true);
  const [refreshCount, setRefreshCount] = useState<number>(0);

  const containerRef = useRef<HTMLDivElement | null>(null);
  const activeLineRef = useRef<HTMLParagraphElement | null>(null);
  const drawerRef = useRef<HTMLDivElement | null>(null);
  const previousActiveElementRef = useRef<HTMLElement | null>(null);
  const prevActiveIndexRef = useRef<number>(-1);
  const programmaticScrollRef = useRef<boolean>(false);
  const programmaticScrollTimerRef = useRef<number | null>(null);

  // Subscribe directly to live position to decouple updates from parent App re-renders
  const livePos = usePlaybackPosition(playbackPositionMs ?? 0, trackDurationMs ?? 0);
  const currentPositionMs = livePos.positionMs || (playbackPositionMs ?? 0);

  // Focus management and keyboard escape handler
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

  // Fetch lyrics when track changes or drawer opens
  useEffect(() => {
    if (!isOpen || !currentTrack) {
      setLyrics(null);
      setIsLoading(false);
      setError(null);
      return;
    }

    const trackId = currentTrack.id;
    let isSubscribed = true;
    setIsLoading(true);
    setError(null);

    getTrackLyrics(trackId)
      .then((res) => {
        if (!isSubscribed) return;
        setLyrics(res);
        setIsLoading(false);
        if (res.status === 'synced' && res.syncedLines && res.syncedLines.length > 0) {
          setMode('synced');
        } else if (res.status === 'plain' || res.plainLyrics) {
          setMode('plain');
        }
      })
      .catch((err) => {
        if (!isSubscribed) return;
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg);
        setIsLoading(false);
      });

    return () => {
      isSubscribed = false;
    };
  }, [isOpen, currentTrack?.id, refreshCount]);

  // Forced refresh handler
  const handleForceRefresh = async () => {
    if (!currentTrack) return;
    setIsLoading(true);
    setError(null);
    try {
      const res = await refreshTrackLyrics(currentTrack.id);
      setLyrics(res);
      if (res.status === 'synced' && res.syncedLines && res.syncedLines.length > 0) {
        setMode('synced');
      } else if (res.status === 'plain' || res.plainLyrics) {
        setMode('plain');
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
    } finally {
      setIsLoading(false);
    }
  };

  // Determine active synchronized lyric line
  const activeIndex = useMemo(() => {
    if (mode !== 'synced' || !lyrics?.syncedLines || lyrics.syncedLines.length === 0) {
      return -1;
    }
    let idx = -1;
    for (let i = 0; i < lyrics.syncedLines.length; i++) {
      if (lyrics.syncedLines[i].timestampMs <= currentPositionMs) {
        idx = i;
      } else {
        break;
      }
    }
    return idx;
  }, [mode, lyrics?.syncedLines, currentPositionMs]);

  const scrollToActiveLine = useCallback((behavior: ScrollBehavior) => {
    const container = containerRef.current;
    const activeEl = activeLineRef.current;
    if (!container || !activeEl) return;

    // Mark the resulting scroll events as programmatic so the user-scroll
    // detector does not mistake smooth scrolling for intentional scrolling.
    programmaticScrollRef.current = true;
    if (programmaticScrollTimerRef.current !== null) {
      window.clearTimeout(programmaticScrollTimerRef.current);
    }
    programmaticScrollTimerRef.current = window.setTimeout(() => {
      programmaticScrollRef.current = false;
      programmaticScrollTimerRef.current = null;
    }, 800);

    const containerHeight = container.clientHeight;
    const lineTop = activeEl.offsetTop;
    const lineHeight = activeEl.clientHeight;
    const targetScrollTop = Math.max(0, lineTop - containerHeight / 2 + lineHeight / 2);

    container.scrollTo({
      top: targetScrollTop,
      behavior,
    });
  }, []);

  // Reset follow/scroll tracking when the drawer opens or the track changes.
  // Without the index reset, a new track whose first active line has the same
  // index as the previous track's last active line would never receive the
  // initial scroll treatment.
  useEffect(() => {
    prevActiveIndexRef.current = -1;
    setIsFollowing(true);
  }, [isOpen, currentTrack?.id]);

  useEffect(() => {
    return () => {
      if (programmaticScrollTimerRef.current !== null) {
        window.clearTimeout(programmaticScrollTimerRef.current);
      }
    };
  }, []);

  // Restrained auto-scrolling: only triggers when the active line index
  // changes, and only while the reader is following the active line.
  useEffect(() => {
    if (!isOpen || mode !== 'synced' || activeIndex === -1) {
      prevActiveIndexRef.current = activeIndex;
      return;
    }

    if (activeIndex === prevActiveIndexRef.current) {
      return;
    }
    prevActiveIndexRef.current = activeIndex;

    if (!isFollowing) return;

    const prefersReducedMotion =
      typeof window !== 'undefined' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    scrollToActiveLine(prefersReducedMotion ? 'auto' : 'smooth');
  }, [activeIndex, isOpen, mode, isFollowing, scrollToActiveLine]);

  // Pause automatic following when the reader scrolls the active line out of
  // view; resume automatically when it is visible again (or via the explicit
  // follow control below).
  const handleBodyScroll = useCallback(() => {
    if (programmaticScrollRef.current) return;
    const container = containerRef.current;
    const activeEl = activeLineRef.current;
    if (!container || !activeEl) return;

    const containerRect = container.getBoundingClientRect();
    const lineRect = activeEl.getBoundingClientRect();
    const isVisible =
      lineRect.bottom > containerRect.top && lineRect.top < containerRect.bottom;
    setIsFollowing(isVisible);
  }, []);

  const handleResumeFollowing = useCallback(() => {
    setIsFollowing(true);
    const prefersReducedMotion =
      typeof window !== 'undefined' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    scrollToActiveLine(prefersReducedMotion ? 'auto' : 'smooth');
  }, [scrollToActiveLine]);

  // Click-to-seek handler. Clamp to the real track duration when it is known
  // so an embedded timestamp beyond the file's end cannot send an invalid
  // position to the transport.
  const handleLineClick = (timestampMs: number) => {
    if (!onSeek) return;
    let positionMs = Math.max(0, timestampMs);
    if (
      typeof trackDurationMs === 'number' &&
      Number.isFinite(trackDurationMs) &&
      trackDurationMs > 0
    ) {
      positionMs = Math.min(positionMs, trackDurationMs);
    }
    void onSeek(positionMs);
  };

  if (!isOpen) return null;

  const hasSynced = !!(lyrics?.syncedLines && lyrics.syncedLines.length > 0);
  const hasPlain = !!(lyrics?.plainLyrics && lyrics.plainLyrics.trim().length > 0);

  return (
    <div className="cavia-drawer-backdrop" onClick={onClose}>
      <div
        ref={drawerRef}
        role="dialog"
        aria-modal="true"
        aria-label="Lyrics view"
        className="cavia-drawer-panel lyrics-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="sheet-drag-handle" aria-hidden="true" />

        {/* Header */}
        <div className="drawer-header">
          <div>
            <div className="drawer-title-row">
              <h2 className="drawer-title">
                {currentTrack ? currentTrack.title : 'Lyrics View'}
              </h2>
              {lyrics?.source && (
                <Badge variant="outline" size="sm" className="lyrics-source-badge">
                  {lyrics.source === 'embedded' ? 'EMBEDDED' : 'LRCLIB'}
                </Badge>
              )}
            </div>
            <span className="lyrics-track-artist">
              {currentTrack?.artist
                ? `${currentTrack.artist}${currentTrack.album ? ` — ${currentTrack.album}` : ''}`
                : 'Standby'}
            </span>
          </div>

          <div className="drawer-header-right">
            {(hasSynced || hasPlain) && (
              <Tabs
                items={[
                  { id: 'synced', label: 'Synced', disabled: !hasSynced },
                  { id: 'plain', label: 'Plain', disabled: !hasPlain },
                ]}
                activeId={mode}
                onChange={(id) => setMode(id as 'synced' | 'plain')}
                variant="pill"
              />
            )}

            {hasSynced && mode === 'synced' && !isFollowing && (
              <IconButton
                label="Resume lyric following"
                icon={<LocateFixed size={16} />}
                variant="ghost"
                onClick={handleResumeFollowing}
              />
            )}

            {currentTrack && (
              <IconButton
                label="Refresh lyrics"
                icon={<RefreshCw size={16} className={isLoading ? 'cavia-spin' : ''} />}
                variant="ghost"
                onClick={handleForceRefresh}
                disabled={isLoading}
              />
            )}

            <IconButton
              label="Close lyrics"
              icon={<X size={18} />}
              variant="ghost"
              onClick={onClose}
            />
          </div>
        </div>

        {/* Body */}
        <div
          className="drawer-body lyrics-body"
          ref={containerRef}
          onScroll={handleBodyScroll}
        >
          {!currentTrack ? (
            <div className="lyrics-empty-state">
              <div className="lyrics-empty-icon">
                <Mic2 size={32} />
              </div>
              <h3>No track in playback</h3>
              <p>
                When you play a track, synchronized or plain lyrics will automatically be matched using embedded metadata and the open LRCLIB database.
              </p>
            </div>
          ) : isLoading ? (
            <div className="lyrics-state-card">
              <div className="lyrics-empty-icon">
                <RefreshCw size={28} className="cavia-spin" />
              </div>
              <h3>Loading lyrics...</h3>
              <p>Checking local embedded tags and matching metadata in the lyrics repository.</p>
            </div>
          ) : error ? (
            <div className="lyrics-state-card">
              <div className="lyrics-empty-icon">
                <AlertCircle size={28} />
              </div>
              <h3>Unable to retrieve lyrics</h3>
              <p>{error}</p>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setRefreshCount((c) => c + 1)}
              >
                Retry Lookup
              </Button>
            </div>
          ) : lyrics?.isInstrumental ? (
            <div className="lyrics-state-card">
              <div className="lyrics-empty-icon">
                <Disc size={32} />
              </div>
              <h3>Instrumental Recording</h3>
              <p>This composition is recognized as an instrumental track without lyrical vocals.</p>
            </div>
          ) : lyrics?.status === 'not_found' || (!hasSynced && !hasPlain) ? (
            <div className="lyrics-empty-state">
              <div className="lyrics-empty-icon">
                <Mic2 size={32} />
              </div>
              <h3>No lyrics found</h3>
              <p>
                We could not find embedded lyrics or a community match on LRCLIB for{' '}
                <strong>{currentTrack.title}</strong>.
              </p>
              <Button
                variant="outline"
                size="sm"
                onClick={handleForceRefresh}
                className="lyrics-retry-btn"
              >
                Refresh from LRCLIB
              </Button>
            </div>
          ) : mode === 'synced' && hasSynced ? (
            <div className="lyrics-typography-stage">
              {lyrics!.syncedLines!.map((line, idx) => {
                const isActive = idx === activeIndex;
                return (
                  <p
                    key={`${line.timestampMs}-${idx}`}
                    ref={isActive ? activeLineRef : null}
                    className={`lyrics-line clickable ${isActive ? 'active-line' : 'inactive'}`}
                    onClick={() => handleLineClick(line.timestampMs)}
                    title={`Seek to ${formatTime(line.timestampMs)}`}
                    tabIndex={0}
                    role="button"
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        handleLineClick(line.timestampMs);
                      }
                    }}
                  >
                    {line.text}
                  </p>
                );
              })}
            </div>
          ) : (
            <div className="lyrics-plain-text">
              {lyrics?.plainLyrics ||
                lyrics?.syncedLines?.map((l) => l.text).join('\n') ||
                ''}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="drawer-footer">
          <span className="drawer-footer-text">
            Lyrics are matched from embedded audio tags or retrieved securely from LRCLIB using track metadata.
          </span>
        </div>
      </div>
    </div>
  );
}

function formatTime(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}
