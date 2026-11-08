import {
  Calendar,
  Disc3,
  ListMusic,
  Play,
  Shuffle,
  Trash2,
} from 'lucide-react';
import { Modal } from '../../components/Modal';
import { Button } from '../../components/Button';
import { Badge } from '../../components/Badge';
import type { Playlist, Track } from '../../lib/types';

interface PlaylistDetailModalProps {
  playlist: Playlist | null;
  isOpen: boolean;
  onClose: () => void;
  isBackendReady: boolean;
}

export function PlaylistDetailModal({
  playlist,
  isOpen,
  onClose,
  isBackendReady,
}: PlaylistDetailModalProps) {
  if (!playlist) return null;

  const tracks: Track[] = playlist.tracks ?? [];

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      eyebrow="PLAYLIST ARCHIVE"
      title={playlist.name}
      size="lg"
      footer={
        <div className="playlist-modal-footer">
          <span className="playlist-footer-info">
            Created on {new Date(playlist.createdAt).toLocaleDateString()} · Local SQLite Storage
          </span>
          <div className="playlist-footer-actions">
            <Button variant="ghost" size="sm" onClick={onClose}>
              Close
            </Button>
            <Button
              variant="outline"
              size="sm"
              icon={<Trash2 size={14} />}
              className="text-danger"
              disabled
              title="Playlist deletion will be active in Milestone 2"
            >
              Delete
            </Button>
          </div>
        </div>
      }
    >
      <div className="playlist-detail-content">
        {/* Header Hero */}
        <div className="playlist-hero">
          <div className="playlist-cover-art">
            <Disc3 size={48} className="playlist-vinyl-icon" />
            <div className="cover-inner-groove" />
          </div>

          <div className="playlist-meta-block">
            <div className="playlist-type-badge">
              <Badge variant="vermilion" size="sm">
                {playlist.sourceProvider ? playlist.sourceProvider.toUpperCase() : 'LOCAL PLAYLIST'}
              </Badge>
              <span className="playlist-date">
                <Calendar size={13} />
                {new Date(playlist.createdAt).toLocaleDateString()}
              </span>
            </div>

            <h1 className="playlist-heading">{playlist.name}</h1>
            <p className="playlist-desc">
              {playlist.description || 'A custom curated selection stored locally on your device.'}
            </p>

            <div className="playlist-action-bar">
              <Button
                variant="primary"
                size="md"
                icon={<Play size={16} />}
                disabled={!isBackendReady || tracks.length === 0}
                title={!isBackendReady ? 'Audio playback engine is not connected (Milestone 3)' : undefined}
              >
                Play All
              </Button>

              <Button
                variant="outline"
                size="md"
                icon={<Shuffle size={16} />}
                disabled={!isBackendReady || tracks.length === 0}
                title={!isBackendReady ? 'Audio playback engine is not connected (Milestone 3)' : undefined}
              >
                Shuffle
              </Button>
            </div>
          </div>
        </div>

        {/* Tracks Section */}
        <div className="playlist-tracks-section">
          <div className="playlist-tracks-header">
            <span>TRACK LIST</span>
            <span className="track-count">{tracks.length} tracks</span>
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
            <div className="playlist-empty-tracks">
              <ListMusic size={32} />
              <h4>No tracks in this playlist yet</h4>
              <p>
                Add songs from your personal archive once local files are imported.
              </p>
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
}

