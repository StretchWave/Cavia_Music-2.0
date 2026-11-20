import { useState } from 'react';
import { usePlaybackPosition } from '../../lib/usePlaybackPosition';
import {
  Disc3,
  FastForward,
  Headphones,
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
  SkipForward,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { IconButton } from '../../components/IconButton';
import { Slider } from '../../components/Slider';
import { formatArtworkUri } from '../../lib/library';
import { ArtworkImage } from '../../components/ArtworkImage';
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
}: PlayerDockProps) {
  const { currentTrack, status, volume, isMuted, repeatMode, isShuffle, playbackSpeed } = playback;
  const livePos = usePlaybackPosition(playback.positionMs, playback.durationMs);
  const positionMs = livePos.positionMs ?? playback.positionMs;
  const durationMs = livePos.durationMs ?? playback.durationMs;
  const isPlaying = status === 'playing';
  const [scrubPosition, setScrubPosition] = useState<number | null>(null);
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);

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
    <footer className="cavia-player-dock" aria-label="Audio player">
      <div className="dock-track-section">
        <button
          type="button"
          className="dock-track-trigger"
          onClick={onOpenExpanded}
          title="Open expanded player"
        >
          <div className="dock-art-frame">
            <ArtworkImage
              src={currentTrack?.artworkUri ? formatArtworkUri(currentTrack.artworkUri) : undefined}
              alt=""
              className="dock-artwork-img"
              fallbackIcon={<Disc3 size={18} aria-hidden="true" />}
            />
          </div>

          <div className="dock-track-info">
            <strong className="dock-track-title">
              {currentTrack?.title ?? (playback.errorMessage ? 'Playback halted' : 'No audio loaded')}
            </strong>
            <span className={`dock-track-artist ${playback.errorMessage ? 'dock-error-text' : ''}`}>
              {playback.errorMessage
                ? playback.errorMessage
                : currentTrack
                  ? currentTrack.artist && currentTrack.album
                    ? `${currentTrack.artist} — ${currentTrack.album}`
                    : currentTrack.artist || currentTrack.album || 'Unknown artist'
                  : 'Select a track to start playback'}
            </span>
          </div>
        </button>
      </div>

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
            title={!isBackendReady ? engineTooltip : isPlaying ? 'Pause' : 'Play'}
            aria-label={isPlaying ? 'Pause' : 'Play'}
          >
            {isPlaying ? <Pause size={18} /> : <Play size={18} className="play-triangle" />}
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
              onChange={setScrubPosition}
              onCommit={(newPos) => {
                setScrubPosition(null);
                onSeek?.(newPos);
              }}
              disabled={!isBackendReady || durationMs <= 0}
              label="Seek track position"
              ariaValueText={`${formatTime(activePosition)} of ${formatTime(displayDuration)}`}
              showFill={durationMs > 0}
            />
          </div>
          <span className="dock-time-code">{formatTime(displayDuration)}</span>
        </div>
      </div>

      <div className="dock-aux-controls">
        {!isBackendReady && audioBackendStatus === 'device_unavailable' && onRetryDevice && (
          <IconButton
            label="Retry audio device"
            icon={<RefreshCw size={16} />}
            onClick={onRetryDevice}
            title="Retry audio device initialization"
            size="sm"
          />
        )}

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

        <button
          type="button"
          className="dock-speed-pill"
          disabled={!isBackendReady}
          title={!isBackendReady ? 'Playback engine unavailable' : 'Change playback rate'}
          aria-label={`Playback speed: ${playbackSpeed}x`}
          onClick={onCycleSpeed}
        >
          {playbackSpeed}x
        </button>

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
              value={isMuted ? 0 : volumeDraft ?? volume}
              max={100}
              onChange={setVolumeDraft}
              onCommit={(newVol) => {
                setVolumeDraft(null);
                onVolumeChange?.(newVol);
              }}
              disabled={!isBackendReady}
              label="Adjust volume"
              ariaValueText={isMuted ? 'Muted' : `${volume}%`}
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

      <div className="dock-mobile-touch-bar">
        <button
          type="button"
          className="mobile-mini-info-btn"
          onClick={onOpenExpanded}
          title="Open expanded player"
        >
          <div className="mobile-mini-art">
            <ArtworkImage
              src={currentTrack?.artworkUri ? formatArtworkUri(currentTrack.artworkUri) : undefined}
              alt=""
              className="mobile-mini-art-img"
              fallbackIcon={<Headphones size={16} />}
            />
          </div>
          <div className="mobile-mini-info">
            <strong>{currentTrack?.title ?? 'No audio loaded'}</strong>
            <span>
              {currentTrack
                ? currentTrack.artist && currentTrack.album
                  ? `${currentTrack.artist} — ${currentTrack.album}`
                  : currentTrack.artist || currentTrack.album || 'Unknown artist'
                : 'Select a track to play'}
            </span>
          </div>
        </button>
        <div className="mobile-mini-actions">
          <button
            type="button"
            className="mobile-mini-play"
            disabled={!isBackendReady}
            onClick={onTogglePlay}
            title={isPlaying ? 'Pause' : 'Play'}
            aria-label={isPlaying ? 'Pause' : 'Play'}
          >
            {isPlaying ? <Pause size={16} /> : <Play size={16} />}
          </button>
          <button
            type="button"
            className="mobile-mini-next"
            disabled={!isBackendReady}
            onClick={onNext}
            aria-label="Next track"
            title="Next track"
          >
            <SkipForward size={18} />
          </button>
        </div>
      </div>
    </footer>
  );
}
