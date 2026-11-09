import { useEffect, useState } from 'react';
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
import { formatArtworkUri, getPlaylistTracks } from '../../lib/library';
import type { Playlist, Track } from '../../lib/types';

interface PlaylistDetailModalProps {
  playlist: Playlist | null;
  isOpen: boolean;
  onClose: () => void;
  isBackendReady: boolean;
  onDeletePlaylist?: (playlistId: string) => void;
  onPlayTracks?: (tracks: Track[], startIndex?: number, shuffle?: boolean) => void;
}

export function PlaylistDetailModal({
  playlist,
  isOpen,
  onClose,
  isBackendReady,
  onDeletePlaylist,
  onPlayTracks,
}: PlaylistDetailModalProps) {
  const [tracks, setTracks] = useState<Track[]>([]);

  useEffect(() => {
    if (!playlist || !isOpen) {
      setTracks([]);
      return;
    }
    let active = true;
    getPlaylistTracks(playlist.id)
      .then((t) => {
        if (active) setTracks(t);
      })
      .catch((e) => console.error('Failed to load playlist tracks:', e));

    return () => {
      active = false;
    };
  }, [playlist, isOpen]);

  if (!playlist) return null;

  const handleDelete = () => {
    if (window.confirm(`Delete playlist "${playlist.name}"? This removes the playlist from your local database.`)) {
      onDeletePlaylist?.(playlist.id);
      onClose();
    }
  };

  const formatDuration = (ms: number) => {
    const totalSecs = Math.floor(ms / 1000);
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

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
              onClick={handleDelete}
              title="Delete playlist from local database"
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
                onClick={() => {
                  onPlayTracks?.(tracks, 0, false);
                  onClose();
                }}
                title="Play playlist from beginning"
              >
                Play All
              </Button>

              <Button
                variant="outline"
                size="md"
                icon={<Shuffle size={16} />}
                disabled={!isBackendReady || tracks.length === 0}
                onClick={() => {
                  onPlayTracks?.(tracks, 0, true);
                  onClose();
                }}
                title="Shuffle play playlist"
              >
                Shuffle
              </Button>

              <Badge variant="neutral" size="sm">
                {tracks.length} {tracks.length === 1 ? 'TRACK' : 'TRACKS'}
              </Badge>
            </div>
          </div>
        </div>

        {/* Track listing */}
        <div className="playlist-tracks-section">
          <h3 className="section-label">TRACK LIST</h3>
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
                  <div
                    key={`${track.id}-${i}`}
                    className="track-row"
                    tabIndex={0}
                    role="row"
                    onClick={() => {
                      onPlayTracks?.(tracks, i, false);
                      onClose();
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        onPlayTracks?.(tracks, i, false);
                        onClose();
                      }
                    }}
                  >
                    <span className="col-num">{i + 1}</span>
                    <span className="col-title">
                      {track.artworkUri && (
                        <img
                          src={formatArtworkUri(track.artworkUri)}
                          alt=""
                          className="track-row-art"
                        />
                      )}
                      <span className="track-title-text">{track.title}</span>
                    </span>
                    <span className="col-artist">{track.artist}</span>
                    <span className="col-album">{track.album}</span>
                    <span className="col-duration">{formatDuration(track.durationMs)}</span>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="playlist-empty-tracks">
              <ListMusic size={32} className="text-muted" />
              <p>No tracks currently added to this playlist.</p>
              <span className="text-muted-xs">
                Tracks will be attachable through the playlist management drawer.
              </span>
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
}
