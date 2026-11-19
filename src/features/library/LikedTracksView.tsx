import { memo } from 'react';
import { Heart, ListPlus, Play, Shuffle } from 'lucide-react';
import { EmptyState } from '../../components/EmptyState';
import { Button } from '../../components/Button';
import { formatArtworkUri } from '../../lib/library';
import type { Track } from '../../lib/types';

interface LikedTracksViewProps {
  tracks: Track[];
  isBackendReady: boolean;
  onToggleLike?: (trackId: string) => void;
  onPlayTrack?: (trackId: string) => void;
  onPlayAll?: () => void;
  onShuffleAll?: () => void;
  currentTrackId?: string;
  isPlaying?: boolean;
  onAddToPlaylist?: (track: Track) => void;
}

export const LikedTracksView = memo(function LikedTracksView({
  tracks,
  isBackendReady,
  onToggleLike,
  onPlayTrack,
  onPlayAll,
  onShuffleAll,
  currentTrackId,
  isPlaying,
  onAddToPlaylist,
}: LikedTracksViewProps) {
  const formatTime = (ms: number) => {
    if (!ms || ms <= 0) return '0:00';
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  return (
    <section className="cavia-subpage liked-page">
      <div className="subpage-header">
        <div>
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>PINNED FAVORITES</span>
          </div>
          <h1 className="subpage-title">
            Liked Tracks<span>.</span>
          </h1>
          <p className="subpage-subtitle">
            Songs you have marked for fast access across your sessions.
          </p>
        </div>

        {tracks.length > 0 && (
          <div className="subpage-actions">
            <Button
              variant="primary"
              size="md"
              icon={<Play size={16} />}
              disabled={!isBackendReady}
              onClick={onPlayAll}
              title="Play all liked tracks"
            >
              Play All
            </Button>
            <Button
              variant="outline"
              size="md"
              icon={<Shuffle size={16} />}
              disabled={!isBackendReady}
              onClick={onShuffleAll}
              title="Shuffle liked tracks"
            >
              Shuffle
            </Button>
          </div>
        )}
      </div>

      {tracks.length > 0 ? (
        <div className="tracks-table" role="table" aria-label="Liked tracks list">
          <div className="tracks-table-head" role="row">
            <span className="col-num" role="columnheader">#</span>
            <span className="col-title" role="columnheader">TITLE</span>
            <span className="col-artist" role="columnheader">ARTIST</span>
            <span className="col-album" role="columnheader">ALBUM</span>
            <span className="col-duration" role="columnheader">TIME</span>
            <span className="col-action" role="columnheader" aria-label="Actions" />
          </div>
          <div className="tracks-table-body" role="rowgroup">
            {tracks.map((track, i) => {
              const isCurrent = currentTrackId === track.id;
              return (
                <div
                  key={track.id}
                  className={`track-row ${isCurrent ? 'active' : ''}`}
                  tabIndex={0}
                  role="row"
                  aria-label={`Track ${track.trackNumber || i + 1}: ${track.title} by ${track.artist || 'Unknown Artist'}`}
                  onClick={() => onPlayTrack?.(track.id)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      onPlayTrack?.(track.id);
                    }
                  }}
                >
                  <span className="col-num" role="cell">
                    {isCurrent && isPlaying ? (
                      <span className="track-playing-indicator" title="Playing">▶</span>
                    ) : (
                      track.trackNumber || i + 1
                    )}
                  </span>
                  <span className="col-title" role="cell">
                    {track.artworkUri && (
                      <img
                        src={formatArtworkUri(track.artworkUri)}
                        alt=""
                        className="track-row-art"
                        loading="lazy"
                      />
                    )}
                    <span className="track-meta-cell">
                      <span className="track-title-text">{track.title}</span>
                      <span className="track-subtitle-mobile">
                        {track.artist || 'Unknown Artist'}
                        {track.album ? ` · ${track.album}` : ''}
                      </span>
                    </span>
                  </span>
                  <span className="col-artist" role="cell">{track.artist || 'Unknown Artist'}</span>
                  <span className="col-album" role="cell">{track.album || 'Unknown Album'}</span>
                  <span className="col-duration" role="cell">{formatTime(track.durationMs)}</span>
                  <span className="col-action" role="cell" onClick={(e) => e.stopPropagation()}>
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
                      className="track-like-inline-btn active"
                      onClick={() => onToggleLike?.(track.id)}
                      aria-label={`Remove ${track.title} from liked tracks`}
                      aria-pressed="true"
                      title="Remove from Liked"
                    >
                      <Heart size={15} fill="var(--accent-vermilion)" color="var(--accent-vermilion)" />
                    </button>
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      ) : (
        <EmptyState
          icon={<Heart size={28} />}
          glyph="好"
          eyebrow="FAVORITES ARCHIVE"
          title={<>Your liked list<br /><em>is currently empty.</em></>}
          description="Click the heart icon on any track row, player dock, or expanded player to collect your essential listening catalog."
          footnote="Liked songs are stored persistently in SQLite and work across all sessions."
        />
      )}
    </section>
  );
});
