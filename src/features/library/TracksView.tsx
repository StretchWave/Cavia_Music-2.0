import { FolderPlus, Heart, ListPlus, Music2, Play, Shuffle } from 'lucide-react';
import { EmptyState } from '../../components/EmptyState';
import { Button } from '../../components/Button';
import { formatArtworkUri } from '../../lib/library';
import type { Track } from '../../lib/types';

interface TracksViewProps {
  tracks: Track[];
  isBackendReady: boolean;
  onToggleLike?: (trackId: string) => void;
  onImportFolder?: () => void;
  isScanning?: boolean;
  onPlayTrack?: (trackId: string) => void;
  onPlayAll?: () => void;
  onShuffleAll?: () => void;
  currentTrackId?: string;
  isPlaying?: boolean;
  onAddToPlaylist?: (track: Track) => void;
}

export function TracksView({
  tracks,
  isBackendReady,
  onToggleLike,
  onImportFolder,
  isScanning,
  onPlayTrack,
  onPlayAll,
  onShuffleAll,
  currentTrackId,
  isPlaying,
  onAddToPlaylist,
}: TracksViewProps) {
  const formatTime = (ms: number) => {
    if (!ms || ms <= 0) return '0:00';
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  return (
    <section className="cavia-subpage tracks-page">
      <div className="subpage-header">
        <div>
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>INDIVIDUAL RECORDINGS</span>
          </div>
          <h1 className="subpage-title">
            All Tracks<span>.</span>
          </h1>
          <p className="subpage-subtitle">
            Flat list of every indexed audio file with full technical metadata.
          </p>
        </div>

        <div className="subpage-actions">
          <Button
            variant="outline"
            size="md"
            icon={<FolderPlus size={16} />}
            onClick={onImportFolder}
            disabled={isScanning}
          >
            {isScanning ? 'Scanning...' : 'Import Folder'}
          </Button>

          {tracks.length > 0 && (
            <>
              <Button
                variant="primary"
                size="md"
                icon={<Play size={16} />}
                disabled={!isBackendReady}
                onClick={onPlayAll}
                title="Play all tracks in order"
              >
                Play All
              </Button>
              <Button
                variant="outline"
                size="md"
                icon={<Shuffle size={16} />}
                disabled={!isBackendReady}
                onClick={onShuffleAll}
                title="Shuffle all tracks"
              >
                Shuffle
              </Button>
            </>
          )}
        </div>
      </div>

      {tracks.length > 0 ? (
        <div className="tracks-table">
          <div className="tracks-table-head">
            <span className="col-num">#</span>
            <span className="col-title">TITLE</span>
            <span className="col-artist">ARTIST</span>
            <span className="col-album">ALBUM</span>
            <span className="col-duration">TIME</span>
            <span className="col-action" />
          </div>
          <div className="tracks-table-body">
            {tracks.map((track, i) => {
              const isCurrent = currentTrackId === track.id;
              return (
                <div
                  key={track.id}
                  className={`track-row ${isCurrent ? 'active' : ''}`}
                  tabIndex={0}
                  role="row"
                  onClick={() => onPlayTrack?.(track.id)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      onPlayTrack?.(track.id);
                    }
                  }}
                >
                  <span className="col-num">
                    {isCurrent && isPlaying ? (
                      <span className="track-playing-indicator" title="Playing">▶</span>
                    ) : (
                      track.trackNumber || i + 1
                    )}
                  </span>
                  <span className="col-title">
                    {track.artworkUri && (
                      <img
                        src={formatArtworkUri(track.artworkUri)}
                        alt=""
                        className="track-row-art"
                        loading="lazy"
                      />
                    )}
                    <span className="track-title-text">{track.title}</span>
                  </span>
                  <span className="col-artist">{track.artist || 'Unknown Artist'}</span>
                  <span className="col-album">{track.album || 'Unknown Album'}</span>
                  <span className="col-duration">{formatTime(track.durationMs)}</span>
                  <span className="col-action" onClick={(e) => e.stopPropagation()}>
                    <button
                      type="button"
                      className="track-action-inline-btn"
                      onClick={() => onAddToPlaylist?.(track)}
                      aria-label={`Add ${track.title} to playlist`}
                      title="Add to playlist"
                    >
                      <ListPlus size={15} />
                    </button>
                    <button
                      type="button"
                      className={`track-like-inline-btn ${track.isLiked ? 'active' : ''}`}
                      onClick={() => onToggleLike?.(track.id)}
                      aria-label={track.isLiked ? 'Remove from liked songs' : 'Add to liked songs'}
                      title={track.isLiked ? 'Liked' : 'Like track'}
                    >
                      <Heart
                        size={15}
                        fill={track.isLiked ? 'var(--accent-vermilion)' : 'none'}
                        color={track.isLiked ? 'var(--accent-vermilion)' : 'currentColor'}
                      />
                    </button>
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      ) : (
        <EmptyState
          icon={<Music2 size={28} />}
          glyph="曲"
          eyebrow="LOCAL TRACKS"
          title={<>Your audio catalog<br /><em>is awaiting files.</em></>}
          description="Cavia will scan your local music directories, extract Vorbis / ID3 tags with the high-performance Lofty engine, and catalog them in SQLite."
          actions={
            <Button
              variant="outline"
              size="md"
              icon={<FolderPlus size={16} />}
              onClick={onImportFolder}
              disabled={isScanning}
            >
              {isScanning ? 'Scanning Directory...' : 'Import Music Directory'}
            </Button>
          }
          footnote="Supported formats: FLAC, ALAC, MP3, AAC, OGG, WAV, and OPUS."
        />
      )}
    </section>
  );
}
