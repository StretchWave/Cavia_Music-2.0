import { Heart, Play, Shuffle } from 'lucide-react';
import { EmptyState } from '../../components/EmptyState';
import { Button } from '../../components/Button';
import type { Track } from '../../lib/types';

interface LikedTracksViewProps {
  tracks: Track[];
  isBackendReady: boolean;
}

export function LikedTracksView({ tracks, isBackendReady }: LikedTracksViewProps) {
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
              title={!isBackendReady ? 'Audio engine standby' : undefined}
            >
              Play All
            </Button>
            <Button
              variant="outline"
              size="md"
              icon={<Shuffle size={16} />}
              disabled={!isBackendReady}
              title={!isBackendReady ? 'Audio engine standby' : undefined}
            >
              Shuffle
            </Button>
          </div>
        )}
      </div>

      {tracks.length > 0 ? (
        <div className="tracks-table">
          <div className="tracks-table-head">
            <span className="col-num">#</span>
            <span className="col-title">TITLE</span>
            <span className="col-artist">ARTIST</span>
            <span className="col-album">ALBUM</span>
            <span className="col-duration">TIME</span>
          </div>
          <div className="tracks-table-body">
            {tracks.map((track, i) => (
              <div key={track.id} className="track-row">
                <span className="col-num">{i + 1}</span>
                <span className="col-title">{track.title}</span>
                <span className="col-artist">{track.artist}</span>
                <span className="col-album">{track.album}</span>
                <span className="col-duration">
                  {Math.floor(track.durationMs / 60000)}:
                  {Math.floor((track.durationMs % 60000) / 1000)
                    .toString()
                    .padStart(2, '0')}
                </span>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <EmptyState
          icon={<Heart size={28} className="text-vermilion" />}
          glyph="愛"
          eyebrow="YOUR FAVORITES"
          title={<>Songs you love<br /><em>will gather here.</em></>}
          description="Click the heart icon on any track in your library or in the player dock to save it here. Pinned favorites are stored securely in your local SQLite database."
          footnote="Favorites can be exported as M3U8 playlists or synced to external devices."
        />
      )}
    </section>
  );
}

