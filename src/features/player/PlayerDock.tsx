import { useState } from 'react';
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
  RefreshCw,
  Repeat,
  Repeat1,
  Rewind,
  Shuffle,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { IconButton } from '../../components/IconButton';
import { Slider } from '../../components/Slider';
import { formatArtworkUri } from '../../lib/library';
import type { PlaybackState } from '../../lib/types';

interface PlayerDockProps {
  playback: PlaybackState;
  isBackendReady: boolean;
  audioBackendStatus?: 'ready' | 'device_unavailable' | 'not_integrated' | 'error' | 'unsupported' | 'initializing';
  onRetryDevice?: () => void;
  onOpenExpanded: () => void;
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

export function PlayerDock({
  playback,
  isBackendReady,
  audioBackendStatus,
  onRetryDevice,
  onOpenExpanded,
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
}: PlayerDockProps) {
  const { currentTrack, status, positionMs, durationMs, volume, isMuted, repeatMode, isShuffle, playbackSpeed } = playback;
  const isPlaying = status === 'playing';
  const [scrubPosition, setScrubPosition] = useState<number | null>(null);

  const activePosition = scrubPosition !== null ? scrubPosition : positionMs;
  const displayDuration = playback.status === 'error' || !currentTrack ? 0 : durationMs;

  const formatTime = (ms: number) => {
    if (!ms || ms <= 0) return '0:00';
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  const engineTooltip = isBackendReady
    ? undefined
    : audioBackendStatus === 'device_unavailable'
      ? 'Audio device unavailable'
      : audioBackendStatus === 'not_integrated'
        ? 'Audio backend not integrated'
        : audioBackendStatus === 'error'
          ? 'Audio engine error'
          : audioBackendStatus === 'unsupported'
            ? 'Backend capability unsupported'
            : audioBackendStatus === 'initializing'
              ? 'Audio engine initializing'
              : 'Audio engine standby';

  return (
    <footer className="cavia-player-dock" role="region" aria-label="Audio player">
      {/* Track Information */}
      <div
        className="dock-track-section"
        onClick={onOpenExpanded}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onOpenExpanded();
          }
        }}
        role="button"
        tabIndex={0}
        aria-label="Open expanded player"
      >
        <div className="dock-art-container">
          <div className="dock-art-frame">
            {currentTrack?.artworkUri ? (
              <img src={formatArtworkUri(currentTrack.artworkUri)} alt="" className="dock-artwork-img" />
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
            {currentTrack?.title ?? (playback.errorMessage ? 'Playback Halted' : 'No audio loaded')}
          </strong>
          <span className={`dock-track-artist ${playback.errorMessage ? 'dock-error-text' : ''}`}>
            {playback.errorMessage
              ? playback.errorMessage
              : currentTrack?.artist
              ? `${currentTrack.artist} — ${currentTrack.album}`
              : 'Select a track to start playback'}
          </span>
        </div>

        <button
          type="button"
          className={`dock-like-btn ${currentTrack?.isLiked ? 'active' : ''}`}
          disabled={!currentTrack || !isBackendReady}
          title={!currentTrack ? 'No active track to like' : (currentTrack.isLiked ? 'Remove from Liked' : 'Save to Liked')}
          aria-label="Like current track"
          onClick={(e) => {
            e.stopPropagation();
            if (currentTrack) onToggleLike?.(currentTrack.id);
          }}
        >
          <Heart size={15} fill={currentTrack?.isLiked ? 'var(--accent-vermilion)' : 'none'} />
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
            onClick={onToggleShuffle}
            title={isShuffle ? 'Disable shuffle' : 'Enable shuffle'}
            size="sm"
          />

          <IconButton
            label="Previous track"
            icon={<Rewind size={16} />}
            disabled={!isBackendReady}
            onClick={onPrevious}
            title="Previous track"
            size="sm"
          />

          <button
            type="button"
            className="dock-play-btn"
            disabled={!isBackendReady}
            onClick={onTogglePlay}
            title={isPlaying ? 'Pause' : 'Play'}
            aria-label={isPlaying ? 'Pause' : 'Play'}
          >
            {isPlaying ? <Pause size={17} /> : <Play size={17} className="play-triangle" />}
          </button>

          <IconButton
            label="Next track"
            icon={<FastForward size={16} />}
            disabled={!isBackendReady}
            onClick={onNext}
            title="Next track"
            size="sm"
          />

          <IconButton
            label={`Repeat: ${repeatMode}`}
            icon={repeatMode === 'one' ? <Repeat1 size={15} /> : <Repeat size={15} />}
            disabled={!isBackendReady}
            active={repeatMode !== 'off'}
            onClick={onCycleRepeat}
            title={`Repeat mode: ${repeatMode}`}
            size="sm"
          />
        </div>

        <div className="dock-scrubber-row">
          <span className="dock-time-code">{formatTime(activePosition)}</span>
          <div className="dock-timeline-track">
            <Slider
              value={activePosition}
              max={durationMs || 100}
              onChange={(newPos) => setScrubPosition(newPos)}
              onCommit={(newPos) => {
                setScrubPosition(null);
                onSeek?.(newPos);
              }}
              disabled={!isBackendReady || durationMs <= 0}
              label="Seek track position"
              showFill={durationMs > 0}
            />
          </div>
          <span className="dock-time-code">{formatTime(displayDuration)}</span>
        </div>
      </div>

      {/* Auxiliary Controls */}
      <div className="dock-aux-controls">
        <div
          className="dock-engine-status"
          title={
            playback.errorMessage
              ? `Playback error: ${playback.errorMessage}`
              : isBackendReady
              ? 'Active audio backend: Native Rodio (WASAPI / CoreAudio via Symphonia)'
              : audioBackendStatus === 'device_unavailable'
              ? 'No audio output device found on system'
              : audioBackendStatus === 'not_integrated'
              ? 'Audio backend not integrated for this platform'
              : audioBackendStatus === 'error'
              ? 'Audio engine initialization error'
              : audioBackendStatus === 'unsupported'
              ? 'Audio backend unsupported'
              : audioBackendStatus === 'initializing'
              ? 'Audio engine is initializing'
              : 'Audio engine standby'
          }
        >
          <span
            className={`status-indicator-dot ${
              playback.errorMessage || audioBackendStatus === 'error'
                ? 'error'
                : isBackendReady
                ? 'ready'
                : 'muted'
            }`}
          />
          <span className="engine-status-text">
            {playback.errorMessage
              ? 'PLAYBACK ERROR'
              : isBackendReady
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
          </span>
        </div>

        {!isBackendReady && audioBackendStatus === 'device_unavailable' && onRetryDevice && (
          <IconButton
            label="Retry audio device"
            icon={<RefreshCw size={16} />}
            onClick={onRetryDevice}
            title="Retry audio device initialization"
            size="sm"
          />
        )}

        <button
          type="button"
          className="dock-speed-pill"
          disabled={!isBackendReady}
          title={!isBackendReady ? 'Playback engine unavailable' : 'Click to cycle playback rate'}
          onClick={onCycleSpeed}
        >
          <span>{playbackSpeed}x</span>
        </button>

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
            onClick={onToggleMute}
            title={isMuted ? 'Unmute' : 'Mute'}
            size="sm"
          />
          <div className="dock-volume-slider">
            <Slider
              value={isMuted ? 0 : volume}
              max={100}
              onCommit={(newVol) => onVolumeChange?.(newVol)}
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
      <div
        className="dock-mobile-touch-bar"
        onClick={onOpenExpanded}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onOpenExpanded();
          }
        }}
        role="button"
        tabIndex={0}
        aria-label="Open expanded player"
      >
        <div className="mobile-mini-art">
          {currentTrack?.artworkUri ? (
            <img src={formatArtworkUri(currentTrack.artworkUri)} alt="" className="mobile-mini-art-img" />
          ) : (
            <Headphones size={16} />
          )}
        </div>
        <div className="mobile-mini-info">
          <strong>{currentTrack?.title ?? 'No audio loaded'}</strong>
          <span>{currentTrack?.artist ?? 'Select a track to play'}</span>
        </div>
        <div className="mobile-mini-actions" onClick={(e) => e.stopPropagation()}>
          <button
            type="button"
            className="mobile-mini-play"
            disabled={!isBackendReady}
            onClick={onTogglePlay}
            title={engineTooltip}
            aria-label="Play/Pause"
          >
            {isPlaying ? <Pause size={15} /> : <Play size={15} />}
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
