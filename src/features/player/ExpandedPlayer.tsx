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
  SlidersHorizontal,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { IconButton } from '../../components/IconButton';
import { Slider } from '../../components/Slider';
import { Badge } from '../../components/Badge';
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
  const positionMs = livePos.positionMs || playback.positionMs;
  const durationMs = livePos.durationMs || playback.durationMs;
  const isPlaying = status === 'playing';
  const [scrubPosition, setScrubPosition] = useState<number | null>(null);

  const activePosition = scrubPosition !== null ? scrubPosition : positionMs;
  const displayDuration = playback.status === 'error' || !currentTrack ? 0 : durationMs;
  const [copiedShare, setCopiedShare] = useState(false);
  const overlayRef = useRef<HTMLDivElement>(null);
  const previousActiveElementRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    previousActiveElementRef.current = document.activeElement as HTMLElement | null;
    document.body.style.overflow = 'hidden';

    // Focus close button initially
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

      // Trap focus
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
      // Ignore clipboard write failures
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
      <div className="expanded-background-ambient" aria-hidden="true" />

      <div className="expanded-header">
        <IconButton
          label="Collapse full player"
          icon={<ChevronDown size={22} />}
          variant="panel"
          size="lg"
          onClick={onClose}
        />

        <div className="expanded-header-meta">
          <span className="expanded-eyebrow">NOW LISTENING</span>
          <strong className="expanded-header-title">
            {currentTrack ? currentTrack.album : 'Cavia Audio Foundation'}
          </strong>
        </div>

        <div className="expanded-header-actions">
          <Badge
            variant={
              isBackendReady
                ? 'ready'
                : audioBackendStatus === 'error'
                ? 'warn'
                : 'outline'
            }
            size="sm"
          >
            {isBackendReady
              ? 'NATIVE AUDIO ENGINE CONNECTED'
              : audioBackendStatus === 'device_unavailable'
              ? 'AUDIO DEVICE UNAVAILABLE'
              : audioBackendStatus === 'not_integrated'
              ? 'BACKEND NOT INTEGRATED'
              : audioBackendStatus === 'error'
              ? 'ENGINE ERROR'
              : audioBackendStatus === 'unsupported'
              ? 'BACKEND UNSUPPORTED'
              : audioBackendStatus === 'initializing'
              ? 'ENGINE INITIALIZING'
              : 'AUDIO ENGINE STANDBY'}
          </Badge>
          <IconButton
            label={copiedShare ? 'Copied track metadata' : 'Share track'}
            icon={copiedShare ? <Check size={18} className="text-ready" /> : <Share2 size={18} />}
            variant="ghost"
            disabled={!currentTrack}
            onClick={handleShareTrack}
            title={!currentTrack ? 'No track playing' : (copiedShare ? 'Copied to clipboard' : 'Copy track info to clipboard')}
          />
        </div>
      </div>

      <div className="expanded-body">
        {/* Left Column: Tactical Artwork & Vinyl Assembly */}
        <div className="expanded-sleeve-stage">
          <div className={`vinyl-disc ${isPlaying ? 'spinning' : ''}`} aria-hidden="true">
            <span className="groove-a" />
            <span className="groove-b" />
            <span className="groove-c" />
            <div className="vinyl-center-label">
              <span className="vinyl-label-text">藏</span>
              <span className="vinyl-spindle" />
            </div>
          </div>

          <div className="sleeve-card">
            {currentTrack?.artworkUri ? (
              <img
                src={formatArtworkUri(currentTrack.artworkUri)}
                alt={currentTrack.title}
                className="sleeve-art-img"
              />
            ) : (
              <div className="sleeve-art-placeholder">
                <span className="sleeve-kanji" aria-hidden="true">
                  音
                </span>
                <span className="sleeve-label">
                  {currentTrack ? currentTrack.title : 'CAVIA ARCHIVE'}
                </span>
              </div>
            )}
            <div className="sleeve-sheen" aria-hidden="true" />
          </div>
        </div>

        {/* Right Column: Editorial Metadata & Physical Transports */}
        <div className="expanded-info-column">
          <div className="expanded-metadata-header">
            <div className="expanded-title-group">
              <div className="cavia-eyebrow">
                <span className="eyebrow-accent-line" />
                <span>
                  {currentTrack?.trackNumber ? `TRACK ${currentTrack.trackNumber}` : 'NOW PLAYING'}
                </span>
              </div>
              <h1 className="expanded-track-title">
                {currentTrack?.title ?? (playback.errorMessage ? 'Playback Halted' : 'No audio loaded')}
              </h1>
              <p className="expanded-track-artist">
                {currentTrack?.artist ? `${currentTrack.artist} — ${currentTrack.album}` : 'Select a recording to listen'}
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
              title={!currentTrack ? 'No track playing' : (currentTrack.isLiked ? 'Remove from Liked' : 'Add to Liked')}
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

          {/* Honest Technical Badges */}
          <div className="expanded-format-badges">
            {currentTrack ? (
              <>
                <Badge variant="vermilion" size="sm">
                  {currentTrack.sourceKind === 'local' ? 'LOCAL AUDIO' : (currentTrack.providerId?.toUpperCase() ?? 'STREAM')}
                </Badge>
                <Badge variant="neutral" size="sm">
                  {currentTrack.genre || 'BIT-PERFECT PCM'}
                </Badge>
                <button
                  type="button"
                  className="badge-speed-btn"
                  disabled={!isBackendReady}
                  onClick={onCycleSpeed}
                  title={!isBackendReady ? 'Playback engine unavailable' : 'Click to change playback speed'}
                >
                  <Badge variant="outline" size="sm">
                    RATE: {playbackSpeed}x
                  </Badge>
                </button>
              </>
            ) : (
              <>
                <Badge variant="outline" size="sm">
                  AUDIO STANDBY
                </Badge>
                <Badge
                  variant={
                    isBackendReady
                      ? 'ready'
                      : audioBackendStatus === 'error'
                      ? 'warn'
                      : 'neutral'
                  }
                  size="sm"
                >
                  {isBackendReady
                    ? 'ENGINE READY'
                    : audioBackendStatus === 'device_unavailable'
                    ? 'DEVICE UNAVAILABLE'
                    : audioBackendStatus === 'not_integrated'
                    ? 'BACKEND NOT INTEGRATED'
                    : audioBackendStatus === 'error'
                    ? 'ENGINE ERROR'
                    : audioBackendStatus === 'unsupported'
                    ? 'UNSUPPORTED'
                    : audioBackendStatus === 'initializing'
                    ? 'ENGINE INITIALIZING'
                    : 'ENGINE STANDBY'}
                </Badge>
                <button
                  type="button"
                  className="badge-speed-btn"
                  disabled={!isBackendReady}
                  onClick={onCycleSpeed}
                  title={!isBackendReady ? 'Playback engine unavailable' : 'Click to change playback speed'}
                >
                  <Badge variant="outline" size="sm">
                    RATE: {playbackSpeed}x
                  </Badge>
                </button>
              </>
            )}
          </div>

          {/* Scrubber */}
          <div className="expanded-scrubber-box">
            <Slider
              value={activePosition}
              max={displayDuration || 100}
              onChange={(newPos) => setScrubPosition(newPos)}
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

          {/* Transport Controls */}
          <div className="expanded-transport-row">
            <IconButton
              label={`Shuffle: ${isShuffle ? 'On' : 'Off'}`}
              icon={<Shuffle size={20} />}
              disabled={!isBackendReady}
              active={isShuffle}
              onClick={onToggleShuffle}
              title={isShuffle ? 'Disable shuffle' : 'Enable shuffle'}
              size="lg"
            />

            <IconButton
              label="Previous track"
              icon={<Rewind size={24} />}
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
              {isPlaying ? <Pause size={28} /> : <Play size={28} className="play-triangle-lg" />}
            </button>

            <IconButton
              label="Next track"
              icon={<FastForward size={24} />}
              disabled={!isBackendReady}
              onClick={onNext}
              title="Next track"
              size="lg"
            />

            <IconButton
              label={`Repeat: ${repeatMode}`}
              icon={repeatMode === 'one' ? <Repeat1 size={20} /> : <Repeat size={20} />}
              disabled={!isBackendReady}
              active={repeatMode !== 'off'}
              onClick={onCycleRepeat}
              title={`Repeat: ${repeatMode}`}
              size="lg"
            />
          </div>

          {/* Quick Sub-Actions */}
          <div className="expanded-subactions-row">
            <button
              type="button"
              className="expanded-chip-btn"
              onClick={onOpenLyrics}
              aria-label="Open lyrics drawer"
            >
              <Mic2 size={16} />
              <span>Lyrics</span>
            </button>

            <button
              type="button"
              className="expanded-chip-btn"
              onClick={onOpenQueue}
              aria-label="Open queue drawer"
            >
              <ListMusic size={16} />
              <span>Queue</span>
            </button>

            <button
              type="button"
              className="expanded-chip-btn"
              disabled
              title="DSP & Equalizer unconfigured"
              aria-disabled="true"
            >
              <SlidersHorizontal size={16} />
              <span>Equalizer</span>
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
                  value={isMuted ? 0 : volume}
                  max={100}
                  onCommit={(newVol) => onVolumeChange?.(newVol)}
                  disabled={!isBackendReady}
                  label="Volume"
                  ariaValueText={isMuted ? 'Muted' : `${volume}%`}
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="expanded-footer">
        <div className="expanded-pipeline-note">
          <span
            className={`pipeline-dot ${
              isBackendReady ? 'ready' : audioBackendStatus === 'error' ? 'error' : 'muted'
            }`}
          />
          <span>
            {isBackendReady
              ? 'AUDIO ENGINE: NATIVE ZERO-DEPENDENCY ENGINE (RODIO / CPAL WASAPI) · VERIFIED'
              : audioBackendStatus === 'device_unavailable'
              ? 'AUDIO ENGINE: PHYSICAL AUDIO OUTPUT DEVICE UNAVAILABLE'
              : audioBackendStatus === 'not_integrated'
              ? 'AUDIO ENGINE: BACKEND NOT INTEGRATED FOR THIS PLATFORM'
              : audioBackendStatus === 'error'
              ? 'AUDIO ENGINE: INITIALIZATION ERROR'
              : audioBackendStatus === 'unsupported'
              ? 'AUDIO ENGINE: UNSUPPORTED'
              : audioBackendStatus === 'initializing'
              ? 'AUDIO ENGINE: INITIALIZING'
              : 'AUDIO ENGINE: STANDBY'}
          </span>
        </div>
      </div>
    </div>
  );
}
