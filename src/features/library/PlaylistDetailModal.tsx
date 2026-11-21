import { useEffect, useState } from 'react';
import {
  Calendar,
  Check,
  CheckCheck,
  CheckSquare,
  ChevronDown,
  ChevronUp,
  Disc3,
  FlipHorizontal,
  ListMusic,
  Pencil,
  Play,
  Shuffle,
  Sparkles,
  Trash2,
  X,
} from 'lucide-react';
import { Modal } from '../../components/Modal';
import { Button } from '../../components/Button';
import { Badge } from '../../components/Badge';
import {
  clearPlaylistTracks,
  formatArtworkUri,
  getPlaylistTracks,
  removeTrackFromPlaylist,
  removeTracksFromPlaylist,
  reorderPlaylistTracks,
  updatePlaylist,
} from '../../lib/library';
import type { Playlist, Track } from '../../lib/types';

interface PlaylistDetailModalProps {
  playlist: Playlist | null;
  isOpen: boolean;
  onClose: () => void;
  isBackendReady: boolean;
  onDeletePlaylist?: (playlistId: string) => void;
  onPlayTracks?: (tracks: Track[], startIndex?: number, shuffle?: boolean) => void;
  onPlaylistUpdated?: () => void;
}

export function PlaylistDetailModal({
  playlist,
  isOpen,
  onClose,
  isBackendReady,
  onDeletePlaylist,
  onPlayTracks,
  onPlaylistUpdated,
}: PlaylistDetailModalProps) {
  const [tracks, setTracks] = useState<Track[]>([]);
  const [currentPlaylist, setCurrentPlaylist] = useState<Playlist | null>(playlist);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [editTitle, setEditTitle] = useState('');
  const [editDesc, setEditDesc] = useState('');
  const [editError, setEditError] = useState<string | null>(null);
  const [isSubmittingEdit, setIsSubmittingEdit] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [showClearTracksConfirm, setShowClearTracksConfirm] = useState(false);
  const [isClearingTracks, setIsClearingTracks] = useState(false);
  const [isManaging, setIsManaging] = useState(false);
  const [selectedPositions, setSelectedPositions] = useState<Set<number>>(new Set());
  const [lastSelectedPosition, setLastSelectedPosition] = useState<number | null>(null);
  const [showBatchRemoveConfirm, setShowBatchRemoveConfirm] = useState(false);
  const [isBatchRemoving, setIsBatchRemoving] = useState(false);

  useEffect(() => {
    setIsManaging(false);
    setSelectedPositions(new Set());
    setLastSelectedPosition(null);
    setShowBatchRemoveConfirm(false);
  }, [currentPlaylist?.id, isOpen]);

  useEffect(() => {
    setCurrentPlaylist(playlist);
    if (playlist) {
      setEditTitle(playlist.name);
      setEditDesc(playlist.description);
    }
  }, [playlist]);

  useEffect(() => {
    if (!currentPlaylist || !isOpen) {
      setTracks([]);
      return;
    }
    let active = true;
    getPlaylistTracks(currentPlaylist.id)
      .then((t) => {
        if (active) setTracks(t);
      })
      .catch((e) => console.error('Failed to load playlist tracks:', e));

    return () => {
      active = false;
    };
  }, [currentPlaylist, isOpen]);

  if (!currentPlaylist) return null;

  const handleDelete = () => {
    setShowDeleteConfirm(true);
  };

  const handleRemoveTrack = async (index: number, e: React.MouseEvent) => {
    e.stopPropagation();
    const targetTrack = tracks[index];
    if (!targetTrack) return;

    // Optimistically update UI
    const updated = tracks.filter((_, i) => i !== index);
    setTracks(updated);

    try {
      await removeTrackFromPlaylist(currentPlaylist.id, index);
      onPlaylistUpdated?.();
    } catch (err) {
      console.error('Failed to remove track from playlist:', err);
      // Revert from backend on error
      const fresh = await getPlaylistTracks(currentPlaylist.id);
      setTracks(fresh);
    }
  };

  const handleMoveTrack = async (
    fromIndex: number,
    toIndex: number,
    e: React.MouseEvent
  ) => {
    e.stopPropagation();
    if (fromIndex === toIndex || toIndex < 0 || toIndex >= tracks.length) return;

    // Optimistically update UI
    const reordered = [...tracks];
    const [moved] = reordered.splice(fromIndex, 1);
    reordered.splice(toIndex, 0, moved);
    setTracks(reordered);

    try {
      await reorderPlaylistTracks(currentPlaylist.id, fromIndex, toIndex);
      onPlaylistUpdated?.();
    } catch (err) {
      console.error('Failed to reorder tracks in playlist:', err);
      // Revert from backend on error
      const fresh = await getPlaylistTracks(currentPlaylist.id);
      setTracks(fresh);
    }
  };

  const handleToggleManage = () => {
    setIsManaging((prev) => {
      if (prev) {
        setSelectedPositions(new Set());
        setLastSelectedPosition(null);
        return false;
      }
      return true;
    });
  };

  const handleToggleSelectPosition = (pos: number) => {
    setIsManaging(true);
    setSelectedPositions((prev) => {
      const next = new Set(prev);
      if (next.has(pos)) {
        next.delete(pos);
      } else {
        next.add(pos);
      }
      return next;
    });
    setLastSelectedPosition(pos);
  };

  const handleRangeSelectPosition = (targetPos: number) => {
    setIsManaging(true);
    const anchorPos = lastSelectedPosition ?? 0;
    const start = Math.min(anchorPos, targetPos);
    const end = Math.max(anchorPos, targetPos);

    setSelectedPositions((prev) => {
      const next = new Set(prev);
      for (let i = start; i <= end; i++) {
        next.add(i);
      }
      return next;
    });
    setLastSelectedPosition(targetPos);
  };

  const handleSelectAll = () => {
    setSelectedPositions(new Set(tracks.map((_, i) => i)));
  };

  const handleSelectInverse = () => {
    const next = new Set<number>();
    for (let i = 0; i < tracks.length; i++) {
      if (!selectedPositions.has(i)) {
        next.add(i);
      }
    }
    setSelectedPositions(next);
  };

  const handleClearSelection = () => {
    setSelectedPositions(new Set());
    setLastSelectedPosition(null);
  };

  const handleExitManage = () => {
    setIsManaging(false);
    setSelectedPositions(new Set());
    setLastSelectedPosition(null);
  };

  const handleConfirmBatchRemove = async () => {
    if (selectedPositions.size === 0 || !currentPlaylist) return;
    setIsBatchRemoving(true);
    try {
      const sortedPositions = Array.from(selectedPositions).sort((a, b) => a - b);
      await removeTracksFromPlaylist(currentPlaylist.id, sortedPositions);

      const posSet = new Set(selectedPositions);
      const updated = tracks.filter((_, idx) => !posSet.has(idx));
      setTracks(updated);
      setCurrentPlaylist((prev) =>
        prev ? { ...prev, trackCount: Math.max(0, prev.trackCount - posSet.size) } : null
      );

      setSelectedPositions(new Set());
      setLastSelectedPosition(null);
      setIsManaging(false);
      setShowBatchRemoveConfirm(false);
      onPlaylistUpdated?.();
    } catch (err) {
      console.error('Failed to batch remove tracks from playlist:', err);
      const fresh = await getPlaylistTracks(currentPlaylist.id);
      setTracks(fresh);
    } finally {
      setIsBatchRemoving(false);
    }
  };

  const handleSaveEdit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editTitle.trim()) {
      setEditError('Playlist name cannot be empty');
      return;
    }
    setIsSubmittingEdit(true);
    setEditError(null);
    try {
      const updated = await updatePlaylist(
        currentPlaylist.id,
        editTitle.trim(),
        editDesc.trim()
      );
      setCurrentPlaylist(updated);
      setIsEditModalOpen(false);
      onPlaylistUpdated?.();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setEditError(`Failed to update playlist: ${msg}`);
    } finally {
      setIsSubmittingEdit(false);
    }
  };

  const formatDuration = (ms: number) => {
    const totalSecs = Math.floor(ms / 1000);
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={onClose}
        title={currentPlaylist.name}
        size="lg"
        footer={
          <div className="playlist-modal-footer">
            <span className="playlist-footer-info">
              Created on {new Date(currentPlaylist.createdAt).toLocaleDateString()} · Local SQLite Storage
            </span>
            <div className="playlist-footer-actions">
              <Button variant="ghost" size="sm" onClick={onClose}>
                Close
              </Button>
              <Button
                variant="outline"
                size="sm"
                icon={<Pencil size={14} />}
                onClick={() => {
                  setEditTitle(currentPlaylist.name);
                  setEditDesc(currentPlaylist.description);
                  setEditError(null);
                  setIsEditModalOpen(true);
                }}
                title="Edit playlist name and description"
              >
                Edit Details
              </Button>
              {tracks.length > 0 && (
                <Button
                  variant="outline"
                  size="sm"
                  icon={<Trash2 size={14} />}
                  onClick={() => setShowClearTracksConfirm(true)}
                  title="Remove all tracks from this playlist"
                >
                  Clear Tracks
                </Button>
              )}
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
                  {currentPlaylist.sourceProvider
                    ? currentPlaylist.sourceProvider.toUpperCase()
                    : 'LOCAL PLAYLIST'}
                </Badge>
                <span className="playlist-date">
                  <Calendar size={13} />
                  {new Date(currentPlaylist.createdAt).toLocaleDateString()}
                </span>
              </div>

              <h1 className="playlist-heading">{currentPlaylist.name}</h1>
              <p className="playlist-desc">
                {currentPlaylist.description ||
                  'A custom curated selection stored locally on your device.'}
              </p>

              <div className="playlist-action-bar">
                {(() => {
                  const playableTracks = tracks.filter(
                    (t) => t.sourceKind === 'provider' || (t.fileUri && t.sourceKind !== 'remote_sync')
                  );
                  const hasPlayable = isBackendReady && playableTracks.length > 0;
                  return (
                    <>
                      <Button
                        variant="primary"
                        size="md"
                        icon={<Play size={16} />}
                        disabled={!hasPlayable}
                        onClick={() => {
                          if (hasPlayable) {
                            onPlayTracks?.(playableTracks, 0, false);
                            onClose();
                          }
                        }}
                        title={
                          hasPlayable
                            ? 'Play playlist from beginning'
                            : 'No playable local audio files available in this playlist'
                        }
                      >
                        Play All
                      </Button>

                      <Button
                        variant="outline"
                        size="md"
                        icon={<Shuffle size={16} />}
                        disabled={!hasPlayable}
                        onClick={() => {
                          if (hasPlayable) {
                            onPlayTracks?.(playableTracks, 0, true);
                            onClose();
                          }
                        }}
                        title={
                          hasPlayable
                            ? 'Shuffle play playlist'
                            : 'No playable local audio files available in this playlist'
                        }
                      >
                        Shuffle
                      </Button>
                    </>
                  );
                })()}

                {tracks.length > 0 && (
                  <button
                    type="button"
                    className={`btn-manage-toggle ${isManaging ? 'active' : ''}`}
                    onClick={handleToggleManage}
                    title="Manage playlist tracks"
                  >
                    <CheckSquare size={14} />
                    {isManaging ? 'Done' : 'Manage'}
                  </button>
                )}

                <Badge variant="neutral" size="sm">
                  {tracks.length} {tracks.length === 1 ? 'TRACK' : 'TRACKS'}
                </Badge>
              </div>
            </div>
          </div>

          {/* Track listing */}
          <div className="playlist-tracks-section">
            <h3 className="section-label">TRACK LIST</h3>

            {(isManaging || selectedPositions.size > 0) && (
              <div className="manage-selection-bar" role="toolbar" aria-label="Manage playlist tracks">
                <div className="manage-selection-status">
                  <span className="manage-count-badge">
                    {selectedPositions.size} of {tracks.length} selected
                  </span>
                </div>

                <div className="manage-selection-buttons">
                  <Button
                    variant="outline"
                    size="sm"
                    icon={<CheckCheck size={14} />}
                    onClick={handleSelectAll}
                    title="Select all tracks in this playlist"
                  >
                    Select All
                  </Button>

                  <Button
                    variant="outline"
                    size="sm"
                    icon={<FlipHorizontal size={14} />}
                    onClick={handleSelectInverse}
                    title="Invert current track selection"
                  >
                    Select Inverse
                  </Button>

                  {selectedPositions.size > 0 && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={handleClearSelection}
                      title="Deselect all tracks"
                    >
                      Clear
                    </Button>
                  )}

                  <Button
                    variant="primary"
                    size="sm"
                    className="btn-danger"
                    disabled={selectedPositions.size === 0}
                    icon={<Trash2 size={14} />}
                    onClick={() => setShowBatchRemoveConfirm(true)}
                    title="Remove selected tracks from playlist"
                  >
                    Remove ({selectedPositions.size})
                  </Button>

                  <Button
                    variant="ghost"
                    size="sm"
                    icon={<X size={14} />}
                    onClick={handleExitManage}
                    title="Exit manage mode"
                  >
                    Done
                  </Button>
                </div>
              </div>
            )}

            {tracks.length > 0 ? (
              <div className="tracks-table" role="table" aria-label={`Tracks in ${currentPlaylist.name}`}>
                <div className="tracks-table-head playlist-table-grid" role="row">
                  <span className="col-num" role="columnheader">#</span>
                  <span className="col-title" role="columnheader">TITLE</span>
                  <span className="col-artist" role="columnheader">ARTIST</span>
                  <span className="col-album" role="columnheader">ALBUM</span>
                  <span className="col-duration" role="columnheader">TIME</span>
                  <span className="col-playlist-actions" role="columnheader">REORDER / REMOVE</span>
                </div>
                <div className="tracks-table-body" role="rowgroup">
                  {tracks.map((track, i) => {
                    const isUnplayable =
                      track.sourceKind !== 'provider' && (!track.fileUri || track.sourceKind === 'remote_sync');
                    const isSelected = selectedPositions.has(i);
                    return (
                      <div
                        key={`${track.id}-${i}`}
                        className={`track-row playlist-table-grid ${isUnplayable ? 'track-row-unplayable' : ''} ${isSelected ? 'row-selected' : ''}`}
                        tabIndex={isUnplayable ? -1 : 0}
                        role="row"
                        aria-label={`Track ${i + 1}: ${track.title} by ${track.artist}${isUnplayable ? ' (Unavailable locally)' : ''}`}
                        onClick={(event) => {
                          if (isManaging) {
                            if (event.shiftKey) {
                              handleRangeSelectPosition(i);
                            } else {
                              handleToggleSelectPosition(i);
                            }
                          } else if (event.shiftKey || event.ctrlKey || event.metaKey) {
                            event.preventDefault();
                            setIsManaging(true);
                            if (event.shiftKey) {
                              handleRangeSelectPosition(i);
                            } else {
                              handleToggleSelectPosition(i);
                            }
                          } else {
                            if (isUnplayable) return;
                            const playable = tracks.filter(
                              (t) => t.sourceKind === 'provider' || (t.fileUri && t.sourceKind !== 'remote_sync')
                            );
                            const playIndex = playable.findIndex((t) => t.id === track.id);
                            onPlayTracks?.(playable, playIndex >= 0 ? playIndex : 0, false);
                            onClose();
                          }
                        }}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault();
                            if (isManaging) {
                              handleToggleSelectPosition(i);
                              return;
                            }
                            if (isUnplayable) return;
                            const playable = tracks.filter(
                              (t) => t.sourceKind === 'provider' || (t.fileUri && t.sourceKind !== 'remote_sync')
                            );
                            const playIndex = playable.findIndex((t) => t.id === track.id);
                            onPlayTracks?.(playable, playIndex >= 0 ? playIndex : 0, false);
                            onClose();
                          }
                        }}
                      >
                        <span className="col-num" role="cell">
                          {isManaging ? (
                            <span
                              className={`track-select-checkbox ${isSelected ? 'checked' : ''}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                if (e.shiftKey) {
                                  handleRangeSelectPosition(i);
                                } else {
                                  handleToggleSelectPosition(i);
                                }
                              }}
                              role="checkbox"
                              aria-checked={isSelected}
                            >
                              {isSelected && <Check size={11} strokeWidth={3} />}
                            </span>
                          ) : (
                            i + 1
                          )}
                        </span>
                        <span className="col-title" role="cell">
                          {track.artworkUri && (
                            <img
                              src={formatArtworkUri(track.artworkUri)}
                              alt=""
                              className="track-row-art"
                            />
                          )}
                          <span className="track-meta-cell">
                            <span className="track-title-text">{track.title}</span>
                            <span className="track-subtitle-mobile">
                              {track.artist || 'Unknown Artist'}
                              {track.album ? ` · ${track.album}` : ''}
                            </span>
                          </span>
                          {isUnplayable && (
                            <span
                              className="track-unavailable-badge"
                              title="Synced from your Cavia cloud account. Audio file is not present locally on this device."
                            >
                              Unavailable locally
                            </span>
                          )}
                        </span>
                        <span className="col-artist" role="cell">{track.artist}</span>
                        <span className="col-album" role="cell">{track.album}</span>
                        <span className="col-duration" role="cell">{formatDuration(track.durationMs)}</span>
                        <span
                          className="col-playlist-actions"
                          role="cell"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <button
                            type="button"
                            className="playlist-track-action-btn"
                            disabled={i === 0}
                            onClick={(e) => handleMoveTrack(i, i - 1, e)}
                            aria-label={`Move ${track.title} up`}
                            title={i === 0 ? 'First track' : 'Move up'}
                          >
                            <ChevronUp size={14} />
                          </button>
                          <button
                            type="button"
                            className="playlist-track-action-btn"
                            disabled={i === tracks.length - 1}
                            onClick={(e) => handleMoveTrack(i, i + 1, e)}
                            aria-label={`Move ${track.title} down`}
                            title={i === tracks.length - 1 ? 'Last track' : 'Move down'}
                          >
                            <ChevronDown size={14} />
                          </button>
                          <button
                            type="button"
                            className="playlist-track-action-btn btn-danger-hover"
                            onClick={(e) => handleRemoveTrack(i, e)}
                            aria-label={`Remove ${track.title} from playlist`}
                            title="Remove from playlist"
                          >
                            <Trash2 size={13} />
                          </button>
                        </span>
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : (
              <div className="playlist-empty-tracks">
                <ListMusic size={32} className="text-muted" />
                <p>No tracks currently added to this playlist.</p>
                <span className="text-muted-xs">
                  Add tracks using the "+" button from All Tracks, Liked Songs, or Albums.
                </span>
              </div>
            )}
          </div>
        </div>
      </Modal>

      {/* Batch Remove from Playlist Confirmation Modal */}
      {showBatchRemoveConfirm && (
        <Modal
          isOpen={showBatchRemoveConfirm}
          onClose={() => setShowBatchRemoveConfirm(false)}
          title={`Remove ${selectedPositions.size} ${selectedPositions.size === 1 ? 'track' : 'tracks'} from playlist?`}
          size="sm"
          role="alertdialog"
          footer={
            <div className="modal-actions-right">
              <Button variant="ghost" size="sm" onClick={() => setShowBatchRemoveConfirm(false)}>
                Cancel
              </Button>
              <Button
                variant="primary"
                size="sm"
                className="btn-danger"
                disabled={isBatchRemoving}
                onClick={handleConfirmBatchRemove}
                icon={<Trash2 size={14} />}
              >
                {isBatchRemoving
                  ? 'Removing…'
                  : `Remove ${selectedPositions.size} ${selectedPositions.size === 1 ? 'track' : 'tracks'}`}
              </Button>
            </div>
          }
        >
          <p className="modal-body-text">
            Remove <strong>{selectedPositions.size}</strong> selected {selectedPositions.size === 1 ? 'track' : 'tracks'} from <strong>"{currentPlaylist.name}"</strong>?
            They will only be removed from this playlist. Your library and original audio files will <strong>not</strong> be deleted.
          </p>
        </Modal>
      )}

      {/* Delete Confirmation Alert Dialog */}
      {showDeleteConfirm && (
        <Modal
          isOpen={showDeleteConfirm}
          onClose={() => setShowDeleteConfirm(false)}
          title={`Delete "${currentPlaylist.name}"?`}
          size="sm"
          role="alertdialog"
          footer={
            <div className="modal-actions-right">
              <Button variant="ghost" size="sm" onClick={() => setShowDeleteConfirm(false)}>
                Cancel
              </Button>
              <Button
                variant="primary"
                size="sm"
                className="text-danger"
                onClick={() => {
                  onDeletePlaylist?.(currentPlaylist.id);
                  setShowDeleteConfirm(false);
                  onClose();
                }}
                icon={<Trash2 size={14} />}
              >
                Delete Playlist
              </Button>
            </div>
          }
        >
          <p className="text-dim-sm" style={{ lineHeight: 1.5 }}>
            Are you sure you want to delete <strong>{currentPlaylist.name}</strong>? This removes the playlist from your local database. Your original audio files remain untouched.
          </p>
        </Modal>
      )}

      {/* Clear Playlist Tracks Confirmation Dialog */}
      {showClearTracksConfirm && (
        <Modal
          isOpen={showClearTracksConfirm}
          onClose={() => setShowClearTracksConfirm(false)}
          title={`Clear all tracks from "${currentPlaylist.name}"?`}
          size="sm"
          role="alertdialog"
          footer={
            <div className="modal-actions-right">
              <Button variant="ghost" size="sm" onClick={() => setShowClearTracksConfirm(false)}>
                Cancel
              </Button>
              <Button
                variant="primary"
                size="sm"
                className="text-danger"
                disabled={isClearingTracks}
                onClick={async () => {
                  setIsClearingTracks(true);
                  try {
                    await clearPlaylistTracks(currentPlaylist.id);
                    setTracks([]);
                    onPlaylistUpdated?.();
                    setShowClearTracksConfirm(false);
                  } catch (err) {
                    console.error('Failed to clear tracks:', err);
                  } finally {
                    setIsClearingTracks(false);
                  }
                }}
                icon={<Trash2 size={14} />}
              >
                {isClearingTracks ? 'Clearing…' : 'Clear All Tracks'}
              </Button>
            </div>
          }
        >
          <p className="text-dim-sm" style={{ lineHeight: 1.5 }}>
            This will remove all {tracks.length} {tracks.length === 1 ? 'track' : 'tracks'} from this playlist. The playlist itself will remain in your library.
          </p>
        </Modal>
      )}

      {/* Edit Details Sub-Modal */}
      {isEditModalOpen && (
        <Modal
          isOpen={isEditModalOpen}
          onClose={() => setIsEditModalOpen(false)}
          title="Edit Playlist Details"
          size="sm"
          footer={
            <div className="modal-actions-right">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setIsEditModalOpen(false)}
              >
                Cancel
              </Button>
              <Button
                variant="primary"
                size="sm"
                disabled={!editTitle.trim() || isSubmittingEdit}
                onClick={handleSaveEdit}
                icon={<Sparkles size={14} />}
              >
                Save Changes
              </Button>
            </div>
          }
        >
          <form onSubmit={handleSaveEdit} className="cavia-form">
            {editError && (
              <div className="cavia-toast toast-error" role="alert" style={{ margin: '0 0 12px 0' }}>
                <span>{editError}</span>
              </div>
            )}
            <div className="form-field">
              <label htmlFor="edit-playlist-title">Playlist Title</label>
              <input
                id="edit-playlist-title"
                type="text"
                required
                autoFocus
                placeholder="Playlist name"
                value={editTitle}
                onChange={(e) => setEditTitle(e.target.value)}
                className="cavia-text-input"
              />
            </div>

            <div className="form-field">
              <label htmlFor="edit-playlist-desc">Description</label>
              <textarea
                id="edit-playlist-desc"
                rows={3}
                placeholder="Playlist description..."
                value={editDesc}
                onChange={(e) => setEditDesc(e.target.value)}
                className="cavia-textarea"
              />
            </div>
          </form>
        </Modal>
      )}
    </>
  );
}
