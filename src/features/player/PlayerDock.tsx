import {
  ChevronUp,
  Disc3,
  FastForward,
  Headphones,
  Heart,
  ListMusic,
  Maximize2,
  Mic2,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Rewind,
  Shuffle,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { IconButton } from '../../components/IconButton';
import { Slider } from '../../components/Slider';
import type { PlaybackState } from '../../lib/types';

interface PlayerDockProps {
  playback: PlaybackState;
  isBackendReady: boolean;
  onOpenExpanded: () => void;
  onOpenQueue: () => void;
  onOpenLyrics: () => void;
  onTogglePlay?: () => void;
}

export function PlayerDock({
  playback,
  isBackendReady,
  onOpenExpanded,
  onOpenQueue,
  onOpenLyrics,
}: PlayerDockProps) {
  const { currentTrack, status, positionMs, durationMs, volume, isMuted, repeatMode, isShuffle, playbackSpeed } = playback;
  const isPlaying = status === 'playing';

  const formatTime = (ms: number) => {
    if (!ms || ms <= 0) return '0:00';
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  const engineDisabledTooltip = 'Playback engine is not connected yet (Milestone 3)';

  return (
    <footer className="cavia-player-dock" role="region" aria-label="Audio player">
      {/* Track Information */}
      <div className="dock-track-section" onClick={onOpenExpanded} role="button" tabIndex={0} aria-label="Open expanded player">
        <div className="dock-art-container">
          <div className="dock-art-frame">
            {currentTrack?.artworkUri ? (
              <img src={currentTrack.artworkUri} alt="" className="dock-artwork-img" />
            ) : (
              <div className="dock-art-empty" aria-hidden="true">
                <Disc3 size={20} className={isPlaying ? 'spinning' : ''} />
              </div>
            )}
          </div>
          <span className="dock-vinyl-peeking" aria-hidden="true" />
        </div>

        <div className="dock-track-info">
          <strong className="dock-track-title">
            {currentTrack?.title ?? 'No audio loaded'}
          </strong>
          <span className="dock-track-artist">
            {currentTrack?.artist ? `${currentTrack.artist} — ${currentTrack.album}` : 'Standby · Ready for local library'}
          </span>
        </div>

        <button
          type="button"
          className="dock-like-btn"
          disabled={!currentTrack || !isBackendReady}
          title={!currentTrack ? 'No active track to like' : 'Save to Liked Tracks'}
          aria-label="Like current track"
          onClick={(e) => e.stopPropagation()}
        >
          <Heart size={15} />
        </button>
      </div>

      {/* Main Transport & Scrubber */}
      <div className="dock-center-controls">
        <div className="dock-transport-buttons">
          <IconButton
            label={`Shuffle: ${isShuffle ? 'On' : 'Off'}`}
            icon={<Shuffle size={15} />}
            disabled={!isBackendReady}
            active={isShuffle}
            title={engineDisabledTooltip}
            size="sm"
          />

          <IconButton
            label="Previous track"
            icon={<Rewind size={16} />}
            disabled={!isBackendReady}
            title={engineDisabledTooltip}
            size="sm"
          />

          <button
            type="button"
            className="dock-play-btn"
            disabled={!isBackendReady}
            title={engineDisabledTooltip}
            aria-label={isPlaying ? 'Pause' : 'Play'}
          >
            {isPlaying ? <Pause size={17} /> : <Play size={17} className="play-triangle" />}
          </button>

          <IconButton
            label="Next track"
            icon={<FastForward size={16} />}
            disabled={!isBackendReady}
            title={engineDisabledTooltip}
            size="sm"
          />

          <IconButton
            label={`Repeat: ${repeatMode}`}
            icon={repeatMode === 'one' ? <Repeat1 size={15} /> : <Repeat size={15} />}
            disabled={!isBackendReady}
            active={repeatMode !== 'off'}
            title={engineDisabledTooltip}
            size="sm"
          />
        </div>

        <div className="dock-scrubber-row">
          <span className="dock-time-code">{formatTime(positionMs)}</span>
          <div className="dock-timeline-track">
            <Slider
              value={positionMs}
              max={durationMs || 100}
              onChange={() => {}}
              disabled={!isBackendReady || durationMs <= 0}
              label="Seek track position"
              showFill={durationMs > 0}
            />
          </div>
          <span className="dock-time-code">{formatTime(durationMs)}</span>
        </div>
      </div>

      {/* Auxiliary Controls */}
      <div className="dock-aux-controls">
        <div className="dock-engine-status" title="Playback engine candidate: libmpv (desktop) / Media3 (Android) / AVPlayer (iOS)">
          <span className="status-indicator-dot muted" />
          <span className="engine-status-text">
            {isBackendReady ? 'ENGINE READY' : 'ENGINE STANDBY'}
          </span>
        </div>

        <div className="dock-speed-pill" title="Playback rate">
          <span>{playbackSpeed}x</span>
        </div>

        <IconButton
          label="Synchronized lyrics"
          icon={<Mic2 size={16} />}
          onClick={onOpenLyrics}
          title="Open lyrics view"
          size="sm"
        />

        <IconButton
          label="Play queue"
          icon={<ListMusic size={16} />}
          onClick={onOpenQueue}
          title="Open play queue"
          size="sm"
        />

        <div className="dock-volume-group">
          <IconButton
            label={isMuted ? 'Unmute' : 'Mute'}
            icon={isMuted ? <VolumeX size={16} /> : <Volume2 size={16} />}
            disabled={!isBackendReady}
            title={engineDisabledTooltip}
            size="sm"
          />
          <div className="dock-volume-slider">
            <Slider
              value={isMuted ? 0 : volume}
              max={100}
              onChange={() => {}}
              disabled={!isBackendReady}
              label="Adjust volume"
            />
          </div>
        </div>

        <IconButton
          label="Expand player"
          icon={<Maximize2 size={15} />}
          onClick={onOpenExpanded}
          title="Expand full player"
          size="sm"
        />
      </div>

      {/* Mobile Mini-Player View */}
      <div className="dock-mobile-touch-bar" onClick={onOpenExpanded}>
        <div className="mobile-mini-art">
          <Headphones size={16} />
        </div>
        <div className="mobile-mini-info">
          <strong>{currentTrack?.title ?? 'No audio loaded'}</strong>
          <span>{currentTrack?.artist ?? 'Playback standby · Cavia 2.0'}</span>
        </div>
        <div className="mobile-mini-actions" onClick={(e) => e.stopPropagation()}>
          <button
            type="button"
            className="mobile-mini-play"
            disabled={!isBackendReady}
            title={engineDisabledTooltip}
            aria-label="Play/Pause"
          >
            <Play size={15} />
          </button>
          <button
            type="button"
            className="mobile-mini-expand"
            onClick={onOpenExpanded}
            aria-label="Expand player"
          >
            <ChevronUp size={18} />
          </button>
        </div>
      </div>
    </footer>
  );
}

