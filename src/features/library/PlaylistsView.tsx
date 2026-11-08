import { useState } from 'react';
import {
  Calendar,
  Disc3,
  ListMusic,
  Plus,
  Sparkles,
} from 'lucide-react';
import { Button } from '../../components/Button';
import { Badge } from '../../components/Badge';
import { Modal } from '../../components/Modal';
import { EmptyState } from '../../components/EmptyState';
import { PlaylistDetailModal } from './PlaylistDetailModal';
import type { Playlist } from '../../lib/types';

interface PlaylistsViewProps {
  playlists: Playlist[];
  onCreatePlaylist: (name: string, description: string) => void;
  isBackendReady: boolean;
}

export function PlaylistsView({
  playlists,
  onCreatePlaylist,
  isBackendReady,
}: PlaylistsViewProps) {
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newDesc, setNewDesc] = useState('');

  const handleCreateSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTitle.trim()) return;
    onCreatePlaylist(newTitle.trim(), newDesc.trim());
    setNewTitle('');
    setNewDesc('');
    setIsCreateModalOpen(false);
  };

  return (
    <section className="cavia-subpage playlists-page">
      <div className="subpage-header">
        <div>
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>COLLECTION ARCHIVE</span>
          </div>
          <h1 className="subpage-title">
            Your Playlists<span>.</span>
          </h1>
          <p className="subpage-subtitle">
            Curated collections saved locally in SQLite without cloud dependencies.
          </p>
        </div>

        <Button
          variant="primary"
          size="md"
          icon={<Plus size={16} />}
          onClick={() => setIsCreateModalOpen(true)}
        >
          New Playlist
        </Button>
      </div>

      {playlists.length > 0 ? (
        <div className="playlists-grid">
          {playlists.map((playlist) => (
            <article
              key={playlist.id}
              className="playlist-card"
              onClick={() => setSelectedPlaylist(playlist)}
              tabIndex={0}
              role="button"
              aria-label={`Open playlist ${playlist.name}`}
            >
              <div className="playlist-card-art">
                <Disc3 size={36} className="playlist-vinyl-emblem" />
                <span className="playlist-art-sheen" />
              </div>

              <div className="playlist-card-body">
                <div className="playlist-card-meta">
                  <Badge variant="vermilion" size="sm">
                    {playlist.sourceProvider ? playlist.sourceProvider.toUpperCase() : 'LOCAL'}
                  </Badge>
                  <span className="playlist-track-badge">
                    {playlist.trackCount} tracks
                  </span>
                </div>

                <h3 className="playlist-card-title">{playlist.name}</h3>
                <p className="playlist-card-desc">
                  {playlist.description || 'No description provided.'}
                </p>

                <div className="playlist-card-footer">
                  <Calendar size={12} />
                  <span>{new Date(playlist.createdAt).toLocaleDateString()}</span>
                </div>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <EmptyState
          icon={<ListMusic size={26} />}
          glyph="選"
          eyebrow="YOUR PLAYLISTS"
          title={<>No playlists created yet<br /><em>ready for your curation.</em></>}
          description="Build playlists to organize your favorite albums, artists, and tracks. Everything is stored locally on your device with complete privacy."
          actions={
            <Button
              variant="primary"
              size="md"
              icon={<Plus size={16} />}
              onClick={() => setIsCreateModalOpen(true)}
            >
              Create First Playlist
            </Button>
          }
          footnote="Local playlists are stored in the SQLite database and never uploaded to remote servers."
        />
      )}

      {/* Create Playlist Modal */}
      <Modal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        title="Create New Playlist"
        eyebrow="LOCAL COLLECTION"
        size="md"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setIsCreateModalOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              disabled={!newTitle.trim()}
              onClick={handleCreateSubmit}
              icon={<Sparkles size={14} />}
            >
              Create Playlist
            </Button>
          </div>
        }
      >
        <form onSubmit={handleCreateSubmit} className="cavia-form">
          <div className="form-field">
            <label htmlFor="playlist-title">Playlist Title</label>
            <input
              id="playlist-title"
              type="text"
              required
              autoFocus
              placeholder="e.g. Night Driving, Japanese Jazz Fusion"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              className="cavia-text-input"
            />
          </div>

          <div className="form-field">
            <label htmlFor="playlist-desc">Description (optional)</label>
            <textarea
              id="playlist-desc"
              rows={3}
              placeholder="Add notes, mood, or context for this selection..."
              value={newDesc}
              onChange={(e) => setNewDesc(e.target.value)}
              className="cavia-textarea"
            />
          </div>
        </form>
      </Modal>

      {/* Playlist Details Modal */}
      <PlaylistDetailModal
        playlist={selectedPlaylist}
        isOpen={!!selectedPlaylist}
        onClose={() => setSelectedPlaylist(null)}
        isBackendReady={isBackendReady}
      />
    </section>
  );
}

