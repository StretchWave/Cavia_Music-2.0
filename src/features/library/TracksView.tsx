import { Music2, FolderPlus, Play, Shuffle } from 'lucide-react';
import { EmptyState } from '../../components/EmptyState';
import { Button } from '../../components/Button';
import type { Track } from '../../lib/types';

interface TracksViewProps {
  tracks: Track[];
  isBackendReady: boolean;
}

export function TracksView({ tracks, isBackendReady }: TracksViewProps) {
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
              disabled
              title="Local audio import pipeline is planned for Milestone 2"
            >
              Import Music Directory <span className="coming-tag">MILESTONE 2</span>
            </Button>
          }
          footnote="Supported formats: FLAC, ALAC, MP3, AAC, OGG, WAV, and OPUS."
        />
      )}
    </section>
  );
}

