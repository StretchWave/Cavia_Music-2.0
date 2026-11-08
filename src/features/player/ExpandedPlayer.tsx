import { useEffect } from 'react';
import {
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
import type { PlaybackState } from '../../lib/types';

interface ExpandedPlayerProps {
  isOpen: boolean;
  onClose: () => void;
  playback: PlaybackState;
  isBackendReady: boolean;
  onOpenQueue: () => void;
  onOpenLyrics: () => void;
}

export function ExpandedPlayer({
  isOpen,
  onClose,
  playback,
  isBackendReady,
  onOpenQueue,
  onOpenLyrics,
}: ExpandedPlayerProps) {
  const { currentTrack, status, positionMs, durationMs, volume, isMuted, repeatMode, isShuffle, playbackSpeed } = playback;
  const isPlaying = status === 'playing';

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    }
    if (isOpen) {
      document.body.style.overflow = 'hidden';
      window.addEventListener('keydown', handleKeyDown);
    }
    return () => {
      document.body.style.overflow = '';
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const formatTime = (ms: number) => {
    if (!ms || ms <= 0) return '0:00';
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  const engineDisabledTooltip = 'Playback engine is not connected yet (Milestone 3)';

  return (
    <div className="cavia-expanded-player-overlay" role="dialog" aria-modal="true">
      <div className="expanded-background-ambient" aria-hidden="true" />

      <header className="expanded-header">
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
          <Badge variant="outline" size="sm">
            {isBackendReady ? 'BIT-PERFECT OUTPUT' : 'STANDBY ENGINE'}
          </Badge>
          <IconButton
            label="Share track"
            icon={<Share2 size={18} />}
            variant="ghost"
            disabled={!currentTrack}
          />
        </div>
      </header>

      <main className="expanded-body">
        {/* Vinyl & Sleeve Presentation */}
        <div className="expanded-sleeve-stage">
          <div className="sleeve-card">
            {currentTrack?.artworkUri ? (
              <img src={currentTrack.artworkUri} alt="" className="sleeve-art-img" />
            ) : (
              <div className="sleeve-art-placeholder">
                <div className="sleeve-pattern" />
                <span className="sleeve-kanji">音</span>
                <span className="sleeve-label">CAVIA HIGH-FIDELITY</span>
              </div>
            )}
            <div className="sleeve-sheen" />
          </div>

          {/* Grooved Vinyl Record */}
          <div className={`vinyl-disc ${isPlaying ? 'spinning' : ''}`} aria-hidden="true">
            <div className="vinyl-groove groove-a" />
            <div className="vinyl-groove groove-b" />
            <div className="vinyl-groove groove-c" />
            <div className="vinyl-center-label">
              <span className="vinyl-label-text">藏</span>
              <span className="vinyl-spindle" />
            </div>
          </div>
        </div>

        {/* Track Details & Controls */}
        <div className="expanded-info-column">
          <div className="expanded-metadata-header">
            <div className="expanded-title-group">
              <h1 className="expanded-track-title">
                {currentTrack?.title ?? 'No audio loaded'}
              </h1>
              <p className="expanded-track-artist">
                {currentTrack?.artist ? `${currentTrack.artist}` : 'Import real audio to begin'}
              </p>
            </div>

            <button
              type="button"
              className="expanded-heart-btn"
              disabled={!currentTrack || !isBackendReady}
              title={!currentTrack ? 'No track playing' : 'Add to Liked'}
              aria-label="Like track"
            >
              <Heart size={22} />
            </button>
          </div>

          <div className="expanded-format-badges">
            <Badge variant="vermilion" size="sm">
              {currentTrack?.sourceKind === 'local' ? 'LOCAL AUDIO' : 'STREAM SOURCE'}
            </Badge>
            <Badge variant="neutral" size="sm">
              FLAC / 44.1kHz · 16-BIT
            </Badge>
            <Badge variant="outline" size="sm">
              RATE: {playbackSpeed}x
            </Badge>
          </div>

          {/* Scrubber */}
          <div className="expanded-scrubber-box">
            <Slider
              value={positionMs}
              max={durationMs || 100}
              onChange={() => {}}
              disabled={!isBackendReady || durationMs <= 0}
              label="Seek track"
              showFill={durationMs > 0}
            />
            <div className="expanded-time-labels">
              <span>{formatTime(positionMs)}</span>
              <span>{formatTime(durationMs)}</span>
            </div>
          </div>

          {/* Transport Controls */}
          <div className="expanded-transport-row">
            <IconButton
              label={`Shuffle: ${isShuffle ? 'On' : 'Off'}`}
              icon={<Shuffle size={20} />}
              disabled={!isBackendReady}
              active={isShuffle}
              title={engineDisabledTooltip}
              size="lg"
            />

            <IconButton
              label="Previous track"
              icon={<Rewind size={24} />}
              disabled={!isBackendReady}
              title={engineDisabledTooltip}
              size="lg"
            />

            <button
              type="button"
              className="expanded-main-play-btn"
              disabled={!isBackendReady}
              title={engineDisabledTooltip}
              aria-label={isPlaying ? 'Pause' : 'Play'}
            >
              {isPlaying ? <Pause size={28} /> : <Play size={28} className="play-triangle-lg" />}
            </button>

            <IconButton
              label="Next track"
              icon={<FastForward size={24} />}
              disabled={!isBackendReady}
              title={engineDisabledTooltip}
              size="lg"
            />

            <IconButton
              label={`Repeat: ${repeatMode}`}
              icon={repeatMode === 'one' ? <Repeat1 size={20} /> : <Repeat size={20} />}
              disabled={!isBackendReady}
              active={repeatMode !== 'off'}
              title={engineDisabledTooltip}
              size="lg"
            />
          </div>

          {/* Quick Sub-Actions */}
          <div className="expanded-subactions-row">
            <button
              type="button"
              className="expanded-chip-btn"
              onClick={onOpenLyrics}
            >
              <Mic2 size={16} />
              <span>Lyrics</span>
            </button>

            <button
              type="button"
              className="expanded-chip-btn"
              onClick={onOpenQueue}
            >
              <ListMusic size={16} />
              <span>Queue</span>
            </button>

            <button
              type="button"
              className="expanded-chip-btn"
              disabled
              title="DSP & Equalizer planned for Milestone 3"
            >
              <SlidersHorizontal size={16} />
              <span>Equalizer</span>
            </button>

            <div className="expanded-volume-control">
              <IconButton
                label={isMuted ? 'Unmute' : 'Mute'}
                icon={isMuted ? <VolumeX size={18} /> : <Volume2 size={18} />}
                disabled={!isBackendReady}
                size="sm"
              />
              <div className="expanded-vol-slider-wrap">
                <Slider
                  value={isMuted ? 0 : volume}
                  max={100}
                  onChange={() => {}}
                  disabled={!isBackendReady}
                  label="Volume"
                />
              </div>
            </div>
          </div>
        </div>
      </main>

      <footer className="expanded-footer">
        <div className="expanded-pipeline-note">
          <span className="pipeline-dot" />
          <span>AUDIO ENGINE: LIBMPV (DESKTOP) / MEDIA3 (ANDROID) / AVPLAYER (IOS) · PENDING MILESTONE 3</span>
        </div>
      </footer>
    </div>
  );
}
