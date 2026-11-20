import { useEffect, useRef, useState } from 'react';
import { usePlaybackPosition } from '../../lib/usePlaybackPosition';
import {
  AlertCircle,
  Check,
  ChevronDown,
  FastForward,
  Heart,
  ListMusic,
  Mic2,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Rewind,
  Share2,
  Shuffle,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { IconButton } from '../../components/IconButton';
import { Slider } from '../../components/Slider';
import { ArtworkImage } from '../../components/ArtworkImage';
import { formatArtworkUri } from '../../lib/library';
import type { PlaybackState } from '../../lib/types';

interface ExpandedPlayerProps {
  isOpen: boolean;
  onClose: () => void;
  playback: PlaybackState;
  isBackendReady: boolean;
  audioBackendStatus?: 'ready' | 'device_unavailable' | 'not_integrated' | 'error' | 'unsupported' | 'initializing';
  onOpenQueue: () => void;
  onOpenLyrics: () => void;
  onTogglePlay?: () => void;
  onNext?: () => void;
  onPrevious?: () => void;
  onSeek?: (positionMs: number) => void;
  onVolumeChange?: (volume: number) => void;
  onToggleMute?: () => void;
  onToggleShuffle?: () => void;
  onCycleRepeat?: () => void;
  onCycleSpeed?: () => void;
  onToggleLike?: (trackId: string) => void;
}

export function ExpandedPlayer({
  isOpen,
  onClose,
  playback,
  isBackendReady,
  audioBackendStatus,
  onOpenQueue,
  onOpenLyrics,
  onTogglePlay,
  onNext,
  onPrevious,
  onSeek,
  onVolumeChange,
  onToggleMute,
  onToggleShuffle,
  onCycleRepeat,
  onCycleSpeed,
  onToggleLike,
}: ExpandedPlayerProps) {
  const { currentTrack, status, volume, isMuted, repeatMode, isShuffle, playbackSpeed } = playback;
  const livePos = usePlaybackPosition(playback.positionMs, playback.durationMs);
  const positionMs = livePos.positionMs ?? playback.positionMs;
  const durationMs = livePos.durationMs ?? playback.durationMs;
  const isPlaying = status === 'playing';
  const [scrubPosition, setScrubPosition] = useState<number | null>(null);
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);
  const [copiedShare, setCopiedShare] = useState(false);

  const activePosition = scrubPosition !== null ? scrubPosition : positionMs;
  const displayDuration = playback.status === 'error' || !currentTrack ? 0 : durationMs;

  const overlayRef = useRef<HTMLDivElement>(null);
  const previousActiveElementRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    previousActiveElementRef.current = document.activeElement as HTMLElement | null;
    document.body.style.overflow = 'hidden';

    const timer = setTimeout(() => {
      if (overlayRef.current) {
        const firstFocusable = overlayRef.current.querySelector<HTMLElement>(
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

      if (e.key === 'Tab' && overlayRef.current) {
        const focusables = Array.from(
          overlayRef.current.querySelectorAll<HTMLElement>(
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

  const handleShareTrack = async () => {
    if (!currentTrack) return;
    try {
      const text = `${currentTrack.title} — ${currentTrack.artist}${currentTrack.album ? ` (${currentTrack.album})` : ''}`;
      await navigator.clipboard.writeText(text);
      setCopiedShare(true);
      setTimeout(() => setCopiedShare(false), 2500);
    } catch {
      // Clipboard access is optional; ignore failures.
    }
  };

  if (!isOpen) return null;

  const formatTime = (ms: number) => {
    if (!ms || ms <= 0) return '0:00';
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  return (
    <div
      ref={overlayRef}
      className="cavia-expanded-player-overlay"
      role="dialog"
      aria-modal="true"
      aria-label="Expanded player"
    >
      <div className="expanded-header">
        <IconButton
          label="Collapse full player"
          icon={<ChevronDown size={22} />}
          variant="panel"
          size="lg"
          onClick={onClose}
        />

        <div className="expanded-header-meta">
          <strong className="expanded-header-title">
            {currentTrack?.album || currentTrack?.artist || 'Cavia Music'}
          </strong>
        </div>

        <IconButton
          label={copiedShare ? 'Copied track metadata' : 'Share track'}
          icon={copiedShare ? <Check size={18} className="text-ready" /> : <Share2 size={18} />}
          variant="ghost"
          disabled={!currentTrack}
          onClick={handleShareTrack}
          title={!currentTrack ? 'No track playing' : copiedShare ? 'Copied to clipboard' : 'Copy track info'}
        />
      </div>

      <div className="expanded-body">
        <div className="expanded-sleeve-stage">
          <div className="sleeve-card">
            <ArtworkImage
              src={currentTrack?.artworkUri ? formatArtworkUri(currentTrack.artworkUri) : undefined}
              alt={currentTrack ? currentTrack.title : 'Album artwork'}
              className="sleeve-art-img"
              kanji="音"
              fallbackText={currentTrack?.title || 'CAVIA ARCHIVE'}
            />
          </div>
        </div>

        <div className="expanded-info-column">
          <div className="expanded-metadata-header">
            <div className="expanded-title-group">
              <h1 className="expanded-track-title">
                {currentTrack?.title ?? (playback.errorMessage ? 'Playback halted' : 'No audio loaded')}
              </h1>
              <p className="expanded-track-artist">
                {currentTrack
                  ? currentTrack.artist && currentTrack.album
                    ? `${currentTrack.artist} — ${currentTrack.album}`
                    : currentTrack.artist || currentTrack.album || 'Unknown artist'
                  : playback.errorMessage || 'Select a recording to listen'}
              </p>
              {playback.errorMessage && (
                <div className="expanded-error-banner" role="alert">
                  <AlertCircle size={15} />
                  <span>{playback.errorMessage}</span>
                </div>
              )}
            </div>

            <button
              type="button"
              className={`expanded-heart-btn ${currentTrack?.isLiked ? 'active' : ''}`}
              disabled={!currentTrack || !isBackendReady}
              onClick={() => {
                if (currentTrack) onToggleLike?.(currentTrack.id);
              }}
              title={!currentTrack ? 'No track playing' : currentTrack.isLiked ? 'Remove from Liked' : 'Add to Liked'}
              aria-label={currentTrack?.isLiked ? 'Remove from Liked' : 'Save to Liked'}
              aria-pressed={currentTrack?.isLiked ? 'true' : 'false'}
            >
              <Heart
                size={22}
                fill={currentTrack?.isLiked ? 'var(--accent-vermilion)' : 'none'}
                color={currentTrack?.isLiked ? 'var(--accent-vermilion)' : 'currentColor'}
              />
            </button>
          </div>

          <div className="expanded-scrubber-box">
            <Slider
              value={activePosition}
              max={displayDuration || 100}
              onChange={setScrubPosition}
              onCommit={(newPos) => {
                setScrubPosition(null);
                onSeek?.(newPos);
              }}
              disabled={!isBackendReady || displayDuration <= 0}
              label="Seek track position"
              ariaValueText={`${formatTime(activePosition)} of ${formatTime(displayDuration)}`}
              showFill={displayDuration > 0}
            />
            <div className="expanded-time-labels">
              <span>{formatTime(activePosition)}</span>
              <span>{formatTime(displayDuration)}</span>
            </div>
          </div>

          <div className="expanded-transport-row">
            <IconButton
              label={`Shuffle: ${isShuffle ? 'On' : 'Off'}`}
              icon={<Shuffle size={22} />}
              disabled={!isBackendReady}
              active={isShuffle}
              onClick={onToggleShuffle}
              title={isShuffle ? 'Disable shuffle' : 'Enable shuffle'}
              size="lg"
            />

            <IconButton
              label="Previous track"
              icon={<Rewind size={22} />}
              disabled={!isBackendReady}
              onClick={onPrevious}
              title="Previous track"
              size="lg"
            />

            <button
              type="button"
              className="expanded-main-play-btn"
              disabled={!isBackendReady}
              onClick={onTogglePlay}
              title={isPlaying ? 'Pause' : 'Play'}
              aria-label={isPlaying ? 'Pause' : 'Play'}
            >
              {isPlaying ? <Pause size={24} /> : <Play size={24} className="play-triangle-lg" />}
            </button>

            <IconButton
              label="Next track"
              icon={<FastForward size={22} />}
              disabled={!isBackendReady}
              onClick={onNext}
              title="Next track"
              size="lg"
            />

            <IconButton
              label={`Repeat: ${repeatMode}`}
              icon={repeatMode === 'one' ? <Repeat1 size={22} /> : <Repeat size={22} />}
              disabled={!isBackendReady}
              active={repeatMode !== 'off'}
              onClick={onCycleRepeat}
              title={`Repeat: ${repeatMode}`}
              size="lg"
            />
          </div>

          <div className="expanded-subactions-row">
            <button type="button" className="expanded-chip-btn" onClick={onOpenLyrics} aria-label="Open lyrics drawer">
              <Mic2 size={17} />
              <span>Lyrics</span>
            </button>

            <button type="button" className="expanded-chip-btn" onClick={onOpenQueue} aria-label="Open queue drawer">
              <ListMusic size={17} />
              <span>Queue</span>
            </button>

            <button
              type="button"
              className="expanded-speed-pill"
              disabled={!isBackendReady}
              onClick={onCycleSpeed}
              title={!isBackendReady ? 'Playback engine unavailable' : 'Change playback rate'}
              aria-label={`Playback speed: ${playbackSpeed}x`}
            >
              {playbackSpeed}x
            </button>

            <div className="expanded-volume-control">
              <IconButton
                label={isMuted ? 'Unmute' : 'Mute'}
                icon={isMuted ? <VolumeX size={18} /> : <Volume2 size={18} />}
                disabled={!isBackendReady}
                onClick={onToggleMute}
                title={isMuted ? 'Unmute' : 'Mute'}
                size="sm"
              />
              <div className="expanded-vol-slider-wrap">
                <Slider
                  value={isMuted ? 0 : volumeDraft ?? volume}
                  max={100}
                  onChange={setVolumeDraft}
                  onCommit={(newVol) => {
                    setVolumeDraft(null);
                    onVolumeChange?.(newVol);
                  }}
                  disabled={!isBackendReady}
                  label="Volume"
                  ariaValueText={isMuted ? 'Muted' : `${volume}%`}
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      {audioBackendStatus && audioBackendStatus !== 'ready' && (
        <div className="expanded-footer">
          <div className="expanded-pipeline-note">
            <span className="pipeline-dot error" />
            <span>
              {audioBackendStatus === 'device_unavailable'
                ? 'Audio output device unavailable. Connect speakers or headphones to resume.'
                : audioBackendStatus === 'error'
                  ? 'Audio engine encountered an initialization error.'
                  : 'Audio engine standby.'}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
