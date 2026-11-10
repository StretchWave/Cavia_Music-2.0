import { useState } from 'react';
import { Check, FolderPlus, ListMusic, Plus, Sparkles } from 'lucide-react';
import { Modal } from '../../components/Modal';
import { Button } from '../../components/Button';
import { addTrackToPlaylist, createPlaylist, formatArtworkUri } from '../../lib/library';
import type { Playlist, Track } from '../../lib/types';

interface AddToPlaylistModalProps {
  isOpen: boolean;
  onClose: () => void;
  tracksToAdd: Track[];
  contextLabel?: string;
  playlists: Playlist[];
  onPlaylistUpdated?: () => void;
}

export function AddToPlaylistModal({
  isOpen,
  onClose,
  tracksToAdd,
  contextLabel,
  playlists,
  onPlaylistUpdated,
}: AddToPlaylistModalProps) {
  const [isCreatingInline, setIsCreatingInline] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newDesc, setNewDesc] = useState('');
  const [addedPlaylists, setAddedPlaylists] = useState<Record<string, boolean>>({});
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  if (!isOpen || tracksToAdd.length === 0) return null;

  const firstTrack = tracksToAdd[0];
  const isMulti = tracksToAdd.length > 1;

  const handleAddTrack = async (playlistId: string) => {
    setIsSubmitting(true);
    setErrorMessage(null);
    try {
      for (const track of tracksToAdd) {
        await addTrackToPlaylist(playlistId, track.id);
      }
      setAddedPlaylists((prev) => ({ ...prev, [playlistId]: true }));
      onPlaylistUpdated?.();
      setTimeout(() => {
        onClose();
        setAddedPlaylists({});
      }, 700);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMessage(`Failed to add tracks: ${msg}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCreateAndAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTitle.trim()) return;
    setIsSubmitting(true);
    setErrorMessage(null);
    try {
      const created = await createPlaylist(newTitle.trim(), newDesc.trim());
      for (const track of tracksToAdd) {
        await addTrackToPlaylist(created.id, track.id);
      }
      setAddedPlaylists((prev) => ({ ...prev, [created.id]: true }));
      onPlaylistUpdated?.();
      setNewTitle('');
      setNewDesc('');
      setIsCreatingInline(false);
      setTimeout(() => {
        onClose();
        setAddedPlaylists({});
      }, 700);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMessage(`Failed to create playlist: ${msg}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        onClose();
        setAddedPlaylists({});
        setIsCreatingInline(false);
        setErrorMessage(null);
      }}
      eyebrow="PLAYLIST ARCHIVE"
      title={isMulti ? `Add ${tracksToAdd.length} Tracks to Playlist` : 'Add Track to Playlist'}
      size="md"
      footer={
        <div className="add-to-pl-footer">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              onClose();
              setAddedPlaylists({});
              setIsCreatingInline(false);
            }}
          >
            Done
          </Button>
          {!isCreatingInline && (
            <Button
              variant="outline"
              size="sm"
              icon={<Plus size={14} />}
              onClick={() => setIsCreatingInline(true)}
            >
              New Playlist
            </Button>
          )}
        </div>
      }
    >
      <div className="add-to-pl-container">
        {/* Track Preview Header */}
        <div className="add-to-pl-target-card">
          {firstTrack.artworkUri ? (
            <img
              src={formatArtworkUri(firstTrack.artworkUri)}
              alt=""
              className="add-to-pl-thumb"
            />
          ) : (
            <div className="add-to-pl-thumb-placeholder">
              <ListMusic size={20} />
            </div>
          )}
          <div className="add-to-pl-target-meta">
            <strong className="add-to-pl-title">
              {isMulti
                ? contextLabel || `${tracksToAdd.length} Selected Recordings`
                : firstTrack.title}
            </strong>
            <span className="add-to-pl-subtitle">
              {isMulti
                ? `Including "${firstTrack.title}" and ${tracksToAdd.length - 1} more`
                : `${firstTrack.artist} · ${firstTrack.album}`}
            </span>
          </div>
        </div>

        {errorMessage && (
          <div className="cavia-toast toast-error" role="alert" style={{ margin: '12px 0' }}>
            <span>{errorMessage}</span>
          </div>
        )}

        {/* Inline Create Form */}
        {isCreatingInline && (
          <form onSubmit={handleCreateAndAdd} className="add-to-pl-inline-form">
            <div className="form-field">
              <label htmlFor="new-pl-inline-title">New Playlist Name</label>
              <input
                id="new-pl-inline-title"
                type="text"
                autoFocus
                required
                placeholder="e.g. Midnight Grooves"
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
                className="cavia-text-input"
              />
            </div>
            <div className="form-field">
              <label htmlFor="new-pl-inline-desc">Description (optional)</label>
              <input
                id="new-pl-inline-desc"
                type="text"
                placeholder="Notes or mood..."
                value={newDesc}
                onChange={(e) => setNewDesc(e.target.value)}
                className="cavia-text-input"
              />
            </div>
            <div className="add-to-pl-inline-actions">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => setIsCreatingInline(false)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                variant="primary"
                size="sm"
                disabled={!newTitle.trim() || isSubmitting}
                icon={<Sparkles size={14} />}
              >
                Create & Add
              </Button>
            </div>
          </form>
        )}

        {/* Existing Playlists List */}
        <div className="add-to-pl-list-section">
          <span className="section-label">CHOOSE PLAYLIST</span>
          {playlists.length > 0 ? (
            <div className="add-to-pl-items">
              {playlists.map((pl) => {
                const isAdded = addedPlaylists[pl.id];
                return (
                  <div
                    key={pl.id}
                    className={`add-to-pl-row ${isAdded ? 'added' : ''}`}
                    onClick={() => !isSubmitting && !isAdded && handleAddTrack(pl.id)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        if (!isSubmitting && !isAdded) handleAddTrack(pl.id);
                      }
                    }}
                  >
                    <div className="add-to-pl-row-left">
                      <div className="add-to-pl-row-icon">
                        <ListMusic size={16} />
                      </div>
                      <div className="add-to-pl-row-text">
                        <strong className="add-to-pl-name">{pl.name}</strong>
                        <span className="add-to-pl-count">
                          {pl.trackCount} {pl.trackCount === 1 ? 'track' : 'tracks'}
                        </span>
                      </div>
                    </div>

                    <Button
                      variant={isAdded ? 'primary' : 'outline'}
                      size="sm"
                      disabled={isSubmitting || isAdded}
                      icon={isAdded ? <Check size={14} /> : <Plus size={14} />}
                      onClick={(e) => {
                        e.stopPropagation();
                        handleAddTrack(pl.id);
                      }}
                    >
                      {isAdded ? 'Added' : 'Add'}
                    </Button>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="add-to-pl-empty">
              <FolderPlus size={24} className="text-muted" />
              <p>No playlists found in your local collection.</p>
              {!isCreatingInline && (
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Plus size={14} />}
                  onClick={() => setIsCreatingInline(true)}
                >
                  Create Your First Playlist
                </Button>
              )}
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
}
