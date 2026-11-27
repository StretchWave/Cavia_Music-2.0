import { useMemo, useRef, useState, type FormEvent, type KeyboardEvent } from 'react';
import {
  ArrowLeft,
  AudioWaveform,
  ChevronRight,
  DownloadCloud,
  Disc3,
  FolderPlus,
  Heart,
  Link2,
  ListMusic,
  Loader2,
  Music2,
  Plus,
  Play,
  Shuffle,
  Trash2,
  WifiOff,
  CheckSquare,
  CheckCheck,
  FlipHorizontal,
  X,
} from 'lucide-react';
import { Button } from '../../components/Button';
import { Modal } from '../../components/Modal';
import { ArtworkImage } from '../../components/ArtworkImage';
import { TrackList } from '../../components/TrackList';
import { AlbumCard, ArtistCard } from '../../components/MediaCards';
import { PlaylistDetailModal } from './PlaylistDetailModal';
import { formatArtworkUri } from '../../lib/library';
import {
  isSpotifyImportAvailable,
  startSpotifyImport,
  type SpotifyImportResult,
} from '../../lib/spotify';
import type { Album, Artist, Playlist, Track } from '../../lib/types';

type LibraryFilter = 'all' | 'tracks' | 'albums' | 'artists' | 'playlists' | 'liked';

interface LibraryViewProps {
  connectionError: string | null;
  libraryError: string | null;
  onRetryLibrary: () => void;
  tracks: Track[];
  albums: Album[];
  artists: Artist[];
  playlists: Playlist[];
  likedTracks: Track[];
  density: 'standard' | 'compact';
  isBackendReady: boolean;
  currentTrackId?: string;
  isPlaying: boolean;
  onImportFolder: () => void;
  isScanning: boolean;
  onCreatePlaylist: (name: string, description: string) => void;
  onDeletePlaylist?: (playlistId: string) => void;
  onDeleteTrack?: (trackId: string) => Promise<void> | void;
  onDeleteTracks?: (trackIds: string[]) => Promise<void> | void;
  onUnlikeTracks?: (trackIds: string[]) => Promise<void> | void;
  onPlaylistUpdated?: () => void;
  onToggleLike: (trackId: string) => void;
  onPlayTrack: (trackId: string) => void;
  onPlayTracks: (tracks: Track[], startIndex?: number, shuffle?: boolean) => void;
  onPlayAll: () => void;
  onShuffleAll: () => void;
  onPlayLikedTrack: (trackId: string) => void;
  onPlayAllLiked: () => void;
  onShuffleAllLiked: () => void;
  onPlayAlbum: (album: Album) => void;
  onPlayArtist: (artist: Artist) => void;
  onAddToPlaylist: (track: Track) => void;
  onAddTracksToPlaylist?: (tracks: Track[]) => void;
  onAddAlbumToPlaylist: (album: Album) => void;
  onPlayNext: (trackId: string) => void;
  onNavigateToImports?: () => void;
  activeImportCount?: number;
  onOpenImportActivity?: () => void;
}

const FILTERS: Array<{ id: LibraryFilter; label: string }> = [
  { id: 'all', label: 'All' },
  { id: 'tracks', label: 'Tracks' },
  { id: 'albums', label: 'Albums' },
  { id: 'artists', label: 'Artists' },
  { id: 'playlists', label: 'Playlists' },
  { id: 'liked', label: 'Liked' },
];

export function LibraryView({
  connectionError,
  libraryError,
  onRetryLibrary,
  tracks,
  albums,
  artists,
  playlists,
  likedTracks,
  density,
  isBackendReady,
  currentTrackId,
  isPlaying,
  onImportFolder,
  isScanning,
  onCreatePlaylist,
  onDeletePlaylist,
  onDeleteTrack,
  onDeleteTracks,
  onUnlikeTracks,
  onPlaylistUpdated,
  onToggleLike,
  onPlayTrack,
  onPlayTracks,
  onPlayAll,
  onShuffleAll,
  onPlayLikedTrack,
  onPlayAllLiked,
  onShuffleAllLiked,
  onPlayAlbum,
  onPlayArtist,
  onAddToPlaylist,
  onAddTracksToPlaylist,
  onAddAlbumToPlaylist,
  onPlayNext,
  onNavigateToImports,
  activeImportCount = 0,
  onOpenImportActivity,
}: LibraryViewProps) {
  const [filter, setFilter] = useState<LibraryFilter>('all');
  const [selectedAlbum, setSelectedAlbum] = useState<Album | null>(null);
  const [selectedArtist, setSelectedArtist] = useState<Artist | null>(null);
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newDesc, setNewDesc] = useState('');
  const [isSpotifyOpen, setIsSpotifyOpen] = useState(false);
  const [spotifyUrl, setSpotifyUrl] = useState('');
  const [spotifyBusy, setSpotifyBusy] = useState(false);
  const [spotifyError, setSpotifyError] = useState<string | null>(null);
  const [spotifyResult, setSpotifyResult] = useState<SpotifyImportResult | null>(null);
  const [playlistToDelete, setPlaylistToDelete] = useState<Playlist | null>(null);
  const [trackToDelete, setTrackToDelete] = useState<Track | null>(null);
  const [isDeletingTrack, setIsDeletingTrack] = useState(false);
  const [isDeletingPlaylist, setIsDeletingPlaylist] = useState(false);

  // Manage mode & multi-selection across library track views
  type ManageSection = 'none' | 'recent' | 'tracks' | 'liked' | 'album' | 'artist';
  const [managingSection, setManagingSection] = useState<ManageSection>('none');
  const [selectedTrackIds, setSelectedTrackIds] = useState<Set<string>>(new Set());
  const [lastSelectedTrackId, setLastSelectedTrackId] = useState<string | null>(null);
  const [showBatchDeleteConfirm, setShowBatchDeleteConfirm] = useState(false);
  const [showBatchUnlikeConfirm, setShowBatchUnlikeConfirm] = useState(false);
  const [isDeletingBatch, setIsDeletingBatch] = useState(false);
  const chipsRef = useRef<HTMLDivElement>(null);
  const emptyTrackIdSet = useMemo(() => new Set<string>(), []);

  const recentTracks = useMemo(
    () =>
      [...tracks]
        .sort((a, b) => {
          const aTime = a.dateAdded ? new Date(a.dateAdded).getTime() : 0;
          const bTime = b.dateAdded ? new Date(b.dateAdded).getTime() : 0;
          return bTime - aTime;
        })
        .slice(0, 20),
    [tracks]
  );

  const albumTracks = useMemo(() => {
    if (!selectedAlbum) return [];
    return tracks.filter(
      (track) =>
        track.album.toLowerCase() === selectedAlbum.title.toLowerCase() &&
        (track.albumArtist?.toLowerCase() === selectedAlbum.artist.toLowerCase() ||
          track.artist.toLowerCase() === selectedAlbum.artist.toLowerCase())
    );
  }, [selectedAlbum, tracks]);

  const artistTracks = useMemo(() => {
    if (!selectedArtist) return [];
    return tracks.filter(
      (track) =>
        track.artist.toLowerCase() === selectedArtist.name.toLowerCase() ||
        track.albumArtist?.toLowerCase() === selectedArtist.name.toLowerCase()
    );
  }, [selectedArtist, tracks]);

  const activePlaylist = selectedPlaylist
    ? playlists.find((playlist) => playlist.id === selectedPlaylist.id) ?? selectedPlaylist
    : null;

  const selectFilter = (next: LibraryFilter) => {
    setFilter(next);
    setSelectedAlbum(null);
    setSelectedArtist(null);
    setSelectedPlaylist(null);
    setManagingSection('none');
    setSelectedTrackIds(new Set());
    setLastSelectedTrackId(null);
  };

  const handleChipKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    let nextIndex = -1;

    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      nextIndex = (index + 1) % FILTERS.length;
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      nextIndex = (index - 1 + FILTERS.length) % FILTERS.length;
    } else if (event.key === 'Home') {
      nextIndex = 0;
    } else if (event.key === 'End') {
      nextIndex = FILTERS.length - 1;
    }

    if (nextIndex === -1) return;
    event.preventDefault();
    selectFilter(FILTERS[nextIndex].id);
    // Focus the newly active tab immediately. Deferring with rAF/setTimeout
    // makes focus follow selection only when frames are being produced.
    chipsRef.current?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[nextIndex]?.focus();
  };

  const handleCreateSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (!newTitle.trim()) return;
    onCreatePlaylist(newTitle.trim(), newDesc.trim());
    setNewTitle('');
    setNewDesc('');
    setIsCreateOpen(false);
  };

  const openSpotifyImport = () => {
    setSpotifyError(null);
    setSpotifyResult(null);
    setIsSpotifyOpen(true);
  };

  const handleSpotifyImport = async (event: FormEvent) => {
    event.preventDefault();
    const link = spotifyUrl.trim();
    if (!link || spotifyBusy) return;

    setSpotifyBusy(true);
    setSpotifyError(null);
    try {
      await startSpotifyImport(link);
      setIsSpotifyOpen(false);
      setSpotifyUrl('');
      onPlaylistUpdated?.();
      onNavigateToImports?.();
    } catch (err) {
      setSpotifyError(err instanceof Error ? err.message : String(err));
    } finally {
      setSpotifyBusy(false);
    }
  };

  const handleDeletePlaylist = (playlistId: string) => {
    onDeletePlaylist?.(playlistId);
    if (selectedPlaylist?.id === playlistId) {
      setSelectedPlaylist(null);
    }
  };

  const handleToggleManage = (section: ManageSection) => {
    if (managingSection === section) {
      setManagingSection('none');
      setSelectedTrackIds(new Set());
      setLastSelectedTrackId(null);
    } else {
      setManagingSection(section);
      setSelectedTrackIds(new Set());
      setLastSelectedTrackId(null);
    }
  };

  const handleEnterManage = (section: ManageSection) => {
    if (managingSection !== section) {
      setManagingSection(section);
    }
  };

  const handleToggleSelectTrack = (trackId: string, section: ManageSection) => {
    if (managingSection !== section) {
      setManagingSection(section);
    }
    setSelectedTrackIds((prev) => {
      const next = new Set(prev);
      if (next.has(trackId)) {
        next.delete(trackId);
      } else {
        next.add(trackId);
      }
      return next;
    });
    setLastSelectedTrackId(trackId);
  };

  const handleRangeSelectTracks = (
    targetTrackId: string,
    sectionTracks: Track[],
    section: ManageSection
  ) => {
    if (managingSection !== section) {
      setManagingSection(section);
    }
    const anchorId = lastSelectedTrackId ?? sectionTracks[0]?.id;
    if (!anchorId) {
      setSelectedTrackIds(new Set([targetTrackId]));
      setLastSelectedTrackId(targetTrackId);
      return;
    }

    const anchorIndex = sectionTracks.findIndex((t) => t.id === anchorId);
    const targetIndex = sectionTracks.findIndex((t) => t.id === targetTrackId);

    if (anchorIndex === -1 || targetIndex === -1) {
      setSelectedTrackIds(new Set([targetTrackId]));
      setLastSelectedTrackId(targetTrackId);
      return;
    }

    const start = Math.min(anchorIndex, targetIndex);
    const end = Math.max(anchorIndex, targetIndex);

    setSelectedTrackIds((prev) => {
      const next = new Set(prev);
      for (let i = start; i <= end; i++) {
        next.add(sectionTracks[i].id);
      }
      return next;
    });
  };

  const handleSelectAll = (sectionTracks: Track[]) => {
    setSelectedTrackIds(new Set(sectionTracks.map((t) => t.id)));
  };

  const handleSelectInverse = (sectionTracks: Track[]) => {
    const next = new Set<string>();
    for (const t of sectionTracks) {
      if (!selectedTrackIds.has(t.id)) {
        next.add(t.id);
      }
    }
    setSelectedTrackIds(next);
  };

  const handleClearSelection = () => {
    setSelectedTrackIds(new Set());
    setLastSelectedTrackId(null);
  };

  const handleExitManage = () => {
    setManagingSection('none');
    setSelectedTrackIds(new Set());
    setLastSelectedTrackId(null);
  };

  const handleConfirmBatchDelete = async () => {
    if (selectedTrackIds.size === 0) return;
    setIsDeletingBatch(true);
    try {
      const ids = Array.from(selectedTrackIds);
      if (onDeleteTracks) {
        await onDeleteTracks(ids);
      } else if (onDeleteTrack) {
        for (const id of ids) {
          await onDeleteTrack(id);
        }
      }
      setSelectedTrackIds(new Set());
      setLastSelectedTrackId(null);
      setManagingSection('none');
      setShowBatchDeleteConfirm(false);
    } catch (err) {
      console.error('Failed to batch delete tracks:', err);
    } finally {
      setIsDeletingBatch(false);
    }
  };

  const handleConfirmBatchUnlike = async () => {
    if (selectedTrackIds.size === 0) return;
    setIsDeletingBatch(true);
    try {
      const ids = Array.from(selectedTrackIds);
      if (onUnlikeTracks) {
        await onUnlikeTracks(ids);
      }
      setSelectedTrackIds(new Set());
      setLastSelectedTrackId(null);
      setManagingSection('none');
      setShowBatchUnlikeConfirm(false);
    } catch (err) {
      console.error('Failed to batch unlike tracks:', err);
    } finally {
      setIsDeletingBatch(false);
    }
  };

  const handleBatchAddToPlaylist = (sectionTracks: Track[]) => {
    if (selectedTrackIds.size === 0) return;
    const selected = sectionTracks.filter((t) => selectedTrackIds.has(t.id));
    if (onAddTracksToPlaylist && selected.length > 0) {
      onAddTracksToPlaylist(selected);
    }
  };

  const renderSelectionBar = (
    section: ManageSection,
    sectionTracks: Track[],
    actionType: 'delete' | 'unlike'
  ) => {
    const isManagingThis = managingSection === section;
    if (!isManagingThis && selectedTrackIds.size === 0) return null;

    return (
      <div className="manage-selection-bar" role="toolbar" aria-label={`Manage ${section} tracks`}>
        <div className="manage-selection-status">
          <span className="manage-count-badge">
            {selectedTrackIds.size} of {sectionTracks.length} selected
          </span>
        </div>

        <div className="manage-selection-buttons">
          <Button
            variant="outline"
            size="sm"
            icon={<CheckCheck size={14} />}
            onClick={() => handleSelectAll(sectionTracks)}
            title="Select all tracks"
          >
            Select All
          </Button>

          <Button
            variant="outline"
            size="sm"
            icon={<FlipHorizontal size={14} />}
            onClick={() => handleSelectInverse(sectionTracks)}
            title="Invert current track selection"
          >
            Select Inverse
          </Button>

          {selectedTrackIds.size > 0 && (
            <Button
              variant="ghost"
              size="sm"
              onClick={handleClearSelection}
              title="Deselect all tracks"
            >
              Clear
            </Button>
          )}

          {onAddTracksToPlaylist && selectedTrackIds.size > 0 && (
            <Button
              variant="outline"
              size="sm"
              icon={<Plus size={14} />}
              onClick={() => handleBatchAddToPlaylist(sectionTracks)}
              title="Add selected tracks to playlist"
            >
              Add to Playlist ({selectedTrackIds.size})
            </Button>
          )}

          <Button
            variant="primary"
            size="sm"
            className="btn-danger"
            disabled={selectedTrackIds.size === 0}
            icon={actionType === 'unlike' ? <Heart size={14} /> : <Trash2 size={14} />}
            onClick={() => {
              if (actionType === 'unlike') {
                setShowBatchUnlikeConfirm(true);
              } else {
                setShowBatchDeleteConfirm(true);
              }
            }}
            title={actionType === 'unlike' ? 'Remove selected tracks from Liked' : 'Remove selected tracks from library'}
          >
            {actionType === 'unlike' ? `Remove from Liked (${selectedTrackIds.size})` : `Remove (${selectedTrackIds.size})`}
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
    );
  };

  const trackListProps = {
    currentTrackId,
    isPlaying,
    isBackendReady,
    onToggleLike,
    onAddToPlaylist,
    onPlayNext,
    onDeleteTrack: onDeleteTrack ? (track: Track) => setTrackToDelete(track) : undefined,
  };

  const headerActions = (
    <div className="library-actions">
      <Button
        variant="outline"
        size="sm"
        icon={<FolderPlus size={15} />}
        onClick={onImportFolder}
        disabled={isScanning}
      >
        {isScanning ? 'Scanning…' : 'Import'}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        icon={<DownloadCloud size={15} />}
        onClick={onOpenImportActivity ?? onNavigateToImports}
        aria-label={activeImportCount > 0 ? `Import activity, ${activeImportCount} active` : 'Import activity'}
      >
        Activity{activeImportCount > 0 ? ` · ${activeImportCount}` : ''}
      </Button>
      {isSpotifyImportAvailable() && (
        <Button
          variant="outline"
          size="sm"
          icon={<Link2 size={15} />}
          onClick={openSpotifyImport}
          title="Rebuild a public Spotify playlist inside Cavia"
        >
          Spotify
        </Button>
      )}
      <Button variant="primary" size="sm" icon={<Plus size={15} />} onClick={() => setIsCreateOpen(true)}>
        New playlist
      </Button>
    </div>
  );

  return (
    <section className="cavia-library-page">
      <header className="library-header">
        <div className="library-heading">
          <p className="page-eyebrow">Your collection</p>
          <h1 className="library-title">
            Library<span>.</span>
          </h1>
          <p className="library-statline">
            {tracks.length.toLocaleString()} tracks · {albums.length.toLocaleString()} albums ·{' '}
            {artists.length.toLocaleString()} artists · {playlists.length.toLocaleString()} playlists
          </p>
        </div>
        {headerActions}
      </header>

      <div
        ref={chipsRef}
        className="library-chips"
        role="tablist"
        aria-label="Library sections"
      >
        {FILTERS.map((item, index) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={filter === item.id}
            aria-controls="library-panel"
            tabIndex={filter === item.id ? 0 : -1}
            className={`library-chip ${filter === item.id ? 'active' : ''}`}
            onClick={() => selectFilter(item.id)}
            onKeyDown={(event) => handleChipKeyDown(event, index)}
          >
            {item.label}
          </button>
        ))}
      </div>

      {connectionError && (
        <div className="cavia-banner banner-error library-error">
          <WifiOff size={16} />
          <span>Local core communication issue: {connectionError}</span>
        </div>
      )}

      {libraryError && (
        <div className="cavia-banner banner-error library-error" role="alert">
          <WifiOff size={16} />
          <span>Could not load your library: {libraryError}</span>
          <Button variant="outline" size="sm" onClick={onRetryLibrary}>
            Retry
          </Button>
        </div>
      )}

      <div
        id="library-panel"
        role="tabpanel"
        aria-label={`${FILTERS.find((item) => item.id === filter)?.label ?? 'Library'} section`}
      >
      {filter === 'all' && (
        <>
          <section className="library-section" aria-label="Liked songs">
            <div className="section-title-row">
              <h2>Liked songs</h2>
              <button type="button" className="text-link" onClick={() => selectFilter('liked')}>
                Open <ChevronRight size={14} />
              </button>
            </div>
            <button type="button" className="playlist-row" onClick={() => selectFilter('liked')}>
              <span className="playlist-row-icon" aria-hidden="true">
                <Heart size={18} />
              </span>
              <span className="playlist-row-copy">
                <strong>Liked songs</strong>
                <span>
                  {likedTracks.length} {likedTracks.length === 1 ? 'track' : 'tracks'} · every song you
                  hearted
                </span>
              </span>
              <ChevronRight size={16} />
            </button>
          </section>

          <section className="library-section" aria-label="Your playlists">
            <div className="section-title-row">
              <h2>Your playlists</h2>
              <button
                type="button"
                className="text-link"
                onClick={() => setIsCreateOpen(true)}
              >
                <Plus size={14} /> New
              </button>
            </div>
            {playlists.length > 0 ? (
              <div className="playlist-rows">
                {playlists.map((playlist) => (
                  <div key={playlist.id} className="playlist-row-wrapper">
                    <button
                      type="button"
                      className="playlist-row"
                      onClick={() => setSelectedPlaylist(playlist)}
                    >
                      <span className="playlist-row-icon" aria-hidden="true">
                        <ListMusic size={18} />
                      </span>
                      <span className="playlist-row-copy">
                        <strong>{playlist.name}</strong>
                        <span>
                          {playlist.trackCount} {playlist.trackCount === 1 ? 'track' : 'tracks'}
                          {playlist.sourceProvider ? ` · ${playlist.sourceProvider}` : ''}
                        </span>
                      </span>
                      <ChevronRight size={16} />
                    </button>
                    {onDeletePlaylist && (
                      <button
                        type="button"
                        className="playlist-row-delete-btn"
                        aria-label={`Delete ${playlist.name}`}
                        title="Delete playlist"
                        onClick={(e) => {
                          e.stopPropagation();
                          setPlaylistToDelete(playlist);
                        }}
                      >
                        <Trash2 size={15} />
                      </button>
                    )}
                  </div>
                ))}
              </div>
            ) : (
              <p className="library-muted-note">
                No playlists yet. Create one to group tracks from your library.
              </p>
            )}
          </section>

          {tracks.length > 0 ? (
            <section className="library-section" aria-label="Recently added">
              <div className="section-title-row">
                <h2>Recently added</h2>
                <div className="section-actions">
                  <button
                    type="button"
                    className={`btn-manage-toggle ${managingSection === 'recent' ? 'active' : ''}`}
                    onClick={() => handleToggleManage('recent')}
                    title="Manage recently added tracks"
                  >
                    <CheckSquare size={14} />
                    {managingSection === 'recent' ? 'Done' : 'Manage'}
                  </button>
                  <button type="button" className="text-link" onClick={() => selectFilter('tracks')}>
                    View all
                  </button>
                </div>
              </div>

              {renderSelectionBar('recent', recentTracks, 'delete')}

              <TrackList
                tracks={recentTracks}
                density={density}
                onPlayTrack={onPlayTrack}
                ariaLabel="Recently added tracks"
                manageMode={managingSection === 'recent'}
                selectedTrackIds={managingSection === 'recent' ? selectedTrackIds : emptyTrackIdSet}
                onToggleSelectTrack={(id) => handleToggleSelectTrack(id, 'recent')}
                onRangeSelectTracks={(id) => handleRangeSelectTracks(id, recentTracks, 'recent')}
                onEnterManageMode={() => handleEnterManage('recent')}
                {...trackListProps}
              />
            </section>
          ) : libraryError ? (
            <div className="library-empty">
              <WifiOff size={24} />
              <h2>Library unavailable</h2>
              <p>Cavia could not read the local library. Your audio files are unchanged.</p>
              <Button variant="primary" size="md" onClick={onRetryLibrary}>
                Retry loading
              </Button>
            </div>
          ) : (
            <div className="library-empty">
              <Music2 size={24} />
              <h2>No tracks indexed yet</h2>
              <p>Cavia plays audio files directly from your local disks. Select a folder to build your library.</p>
              <Button
                variant="primary"
                size="md"
                icon={<FolderPlus size={16} />}
                onClick={onImportFolder}
                disabled={isScanning}
              >
                {isScanning ? 'Scanning…' : 'Add music folders'}
              </Button>
            </div>
          )}

        </>
      )}

      {filter === 'tracks' && (
        <section className="library-section" aria-label="All tracks">
          <div className="section-title-row">
            <h2>All tracks</h2>
            {tracks.length > 0 && (
              <div className="section-actions">
                <button
                  type="button"
                  className={`btn-manage-toggle ${managingSection === 'tracks' ? 'active' : ''}`}
                  onClick={() => handleToggleManage('tracks')}
                  title="Manage all tracks"
                >
                  <CheckSquare size={14} />
                  {managingSection === 'tracks' ? 'Done' : 'Manage'}
                </button>
                <button type="button" className="text-link" onClick={onPlayAll} disabled={!isBackendReady}>
                  <Play size={14} /> Play all
                </button>
                <button type="button" className="text-link" onClick={onShuffleAll} disabled={!isBackendReady}>
                  <Shuffle size={14} /> Shuffle
                </button>
              </div>
            )}
          </div>

          {renderSelectionBar('tracks', tracks, 'delete')}

          {tracks.length > 0 ? (
            <TrackList
              tracks={tracks}
              density={density}
              onPlayTrack={onPlayTrack}
              ariaLabel="All tracks"
              manageMode={managingSection === 'tracks'}
              selectedTrackIds={managingSection === 'tracks' ? selectedTrackIds : emptyTrackIdSet}
              onToggleSelectTrack={(id) => handleToggleSelectTrack(id, 'tracks')}
              onRangeSelectTracks={(id) => handleRangeSelectTracks(id, tracks, 'tracks')}
              onEnterManageMode={() => handleEnterManage('tracks')}
              {...trackListProps}
            />
          ) : libraryError ? (
            <div className="library-empty compact-empty">
              <WifiOff size={22} />
              <h2>Library unavailable</h2>
              <p>Retry loading your library. Audio files are unchanged.</p>
              <Button variant="outline" size="sm" onClick={onRetryLibrary}>
                Retry
              </Button>
            </div>
          ) : (
            <div className="library-empty compact-empty">
              <Music2 size={22} />
              <h2>No tracks indexed</h2>
              <p>Import a folder to add local audio files.</p>
            </div>
          )}
        </section>
      )}

      {filter === 'albums' && (
        <section className="library-section" aria-label="Albums">
          {selectedAlbum ? (
            <>
              <button
                type="button"
                className="back-link"
                onClick={() => {
                  setSelectedAlbum(null);
                  handleExitManage();
                }}
              >
                <ArrowLeft size={15} /> Albums
              </button>

              <div className="collection-detail-header">
                <ArtworkImage
                  src={formatArtworkUri(selectedAlbum.artworkUri)}
                  alt={selectedAlbum.title}
                  className="collection-detail-art"
                  fallbackIcon={<Disc3 size={28} />}
                />
                <div className="collection-detail-copy">
                  <span className="collection-detail-kind">Album</span>
                  <h2>{selectedAlbum.title}</h2>
                  <p>
                    {selectedAlbum.artist}
                    {selectedAlbum.year ? ` · ${selectedAlbum.year}` : ''} · {albumTracks.length}{' '}
                    {albumTracks.length === 1 ? 'track' : 'tracks'}
                  </p>
                  <div className="collection-detail-actions">
                    <Button
                      variant="primary"
                      size="sm"
                      icon={<Play size={14} />}
                      disabled={!isBackendReady || albumTracks.length === 0}
                      onClick={() => onPlayTracks(albumTracks, 0, false)}
                    >
                      Play
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      icon={<Shuffle size={14} />}
                      disabled={!isBackendReady || albumTracks.length === 0}
                      onClick={() => onPlayTracks(albumTracks, 0, true)}
                    >
                      Shuffle
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      className={`btn-manage-toggle ${managingSection === 'album' ? 'active' : ''}`}
                      icon={<CheckSquare size={14} />}
                      onClick={() => handleToggleManage('album')}
                      title="Manage album tracks"
                    >
                      {managingSection === 'album' ? 'Done' : 'Manage'}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      icon={<Plus size={14} />}
                      onClick={() => onAddAlbumToPlaylist(selectedAlbum)}
                    >
                      Add to playlist
                    </Button>
                  </div>
                </div>
              </div>

              {renderSelectionBar('album', albumTracks, 'delete')}

              {albumTracks.length > 0 ? (
                <TrackList
                  tracks={albumTracks}
                  density={density}
                  onPlayTrack={(trackId) => {
                    const index = albumTracks.findIndex((track) => track.id === trackId);
                    onPlayTracks(albumTracks, index >= 0 ? index : 0, false);
                  }}
                  ariaLabel={`Tracks from ${selectedAlbum.title}`}
                  manageMode={managingSection === 'album'}
                  selectedTrackIds={managingSection === 'album' ? selectedTrackIds : emptyTrackIdSet}
                  onToggleSelectTrack={(id) => handleToggleSelectTrack(id, 'album')}
                  onRangeSelectTracks={(id) => handleRangeSelectTracks(id, albumTracks, 'album')}
                  onEnterManageMode={() => handleEnterManage('album')}
                  {...trackListProps}
                />
              ) : (
                <p className="library-muted-note">No indexed tracks for this album.</p>
              )}
            </>
          ) : albums.length > 0 ? (
            <div className="media-grid">
              {albums.map((album) => (
                <AlbumCard
                  key={album.id}
                  album={album}
                  onOpen={setSelectedAlbum}
                  onPlay={onPlayAlbum}
                  playDisabled={!isBackendReady}
                />
              ))}
            </div>
          ) : (
            <div className="library-empty compact-empty">
              <Disc3 size={22} />
              <h2>No albums yet</h2>
              <p>Albums appear automatically from the tags in your imported files.</p>
            </div>
          )}
        </section>
      )}

      {filter === 'artists' && (
        <section className="library-section" aria-label="Artists">
          {selectedArtist ? (
            <>
              <button
                type="button"
                className="back-link"
                onClick={() => {
                  setSelectedArtist(null);
                  handleExitManage();
                }}
              >
                <ArrowLeft size={15} /> Artists
              </button>

              <div className="collection-detail-header">
                {selectedArtist.artworkUri ? (
                  <ArtworkImage
                    src={formatArtworkUri(selectedArtist.artworkUri)}
                    alt={selectedArtist.name}
                    className="collection-detail-art"
                  />
                ) : (
                  <div className="collection-detail-art artist-monogram-large" aria-hidden="true">
                    {selectedArtist.name.slice(0, 2).toUpperCase()}
                  </div>
                )}
                <div className="collection-detail-copy">
                  <span className="collection-detail-kind">Artist</span>
                  <h2>{selectedArtist.name}</h2>
                  <p>
                    {selectedArtist.albumCount} {selectedArtist.albumCount === 1 ? 'album' : 'albums'} ·{' '}
                    {artistTracks.length} {artistTracks.length === 1 ? 'track' : 'tracks'}
                  </p>
                  <div className="collection-detail-actions">
                    <Button
                      variant="primary"
                      size="sm"
                      icon={<Play size={14} />}
                      disabled={!isBackendReady || artistTracks.length === 0}
                      onClick={() => onPlayTracks(artistTracks, 0, false)}
                    >
                      Play
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      icon={<Shuffle size={14} />}
                      disabled={!isBackendReady || artistTracks.length === 0}
                      onClick={() => onPlayTracks(artistTracks, 0, true)}
                    >
                      Shuffle
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      className={`btn-manage-toggle ${managingSection === 'artist' ? 'active' : ''}`}
                      icon={<CheckSquare size={14} />}
                      onClick={() => handleToggleManage('artist')}
                      title="Manage artist tracks"
                    >
                      {managingSection === 'artist' ? 'Done' : 'Manage'}
                    </Button>
                  </div>
                </div>
              </div>

              {renderSelectionBar('artist', artistTracks, 'delete')}

              {artistTracks.length > 0 ? (
                <TrackList
                  tracks={artistTracks}
                  density={density}
                  onPlayTrack={(trackId) => {
                    const index = artistTracks.findIndex((track) => track.id === trackId);
                    onPlayTracks(artistTracks, index >= 0 ? index : 0, false);
                  }}
                  ariaLabel={`Tracks by ${selectedArtist.name}`}
                  manageMode={managingSection === 'artist'}
                  selectedTrackIds={managingSection === 'artist' ? selectedTrackIds : emptyTrackIdSet}
                  onToggleSelectTrack={(id) => handleToggleSelectTrack(id, 'artist')}
                  onRangeSelectTracks={(id) => handleRangeSelectTracks(id, artistTracks, 'artist')}
                  onEnterManageMode={() => handleEnterManage('artist')}
                  {...trackListProps}
                />
              ) : (
                <p className="library-muted-note">No indexed tracks for this artist.</p>
              )}
            </>
          ) : artists.length > 0 ? (
            <div className="media-grid">
              {artists.map((artist) => (
                <ArtistCard
                  key={artist.id}
                  artist={artist}
                  onOpen={setSelectedArtist}
                  onPlay={onPlayArtist}
                  playDisabled={!isBackendReady}
                />
              ))}
            </div>
          ) : (
            <div className="library-empty compact-empty">
              <AudioWaveform size={22} />
              <h2>No artists yet</h2>
              <p>Artists are grouped automatically from your imported tracks.</p>
            </div>
          )}
        </section>
      )}

      {filter === 'playlists' && (
        <section className="library-section" aria-label="Playlists">
          <div className="section-title-row">
            <h2>Playlists</h2>
          </div>
          {playlists.length > 0 ? (
            <div className="playlist-rows">
              {playlists.map((playlist) => (
                <div key={playlist.id} className="playlist-row-wrapper">
                  <button
                    type="button"
                    className="playlist-row"
                    onClick={() => setSelectedPlaylist(playlist)}
                  >
                    <span className="playlist-row-icon" aria-hidden="true">
                      <ListMusic size={18} />
                    </span>
                    <span className="playlist-row-copy">
                      <strong>{playlist.name}</strong>
                      <span>
                        {playlist.trackCount} {playlist.trackCount === 1 ? 'track' : 'tracks'}
                        {playlist.description ? ` · ${playlist.description}` : ''}
                      </span>
                    </span>
                    <ChevronRight size={16} />
                  </button>
                  {onDeletePlaylist && (
                    <button
                      type="button"
                      className="playlist-row-delete-btn"
                      aria-label={`Delete ${playlist.name}`}
                      title="Delete playlist"
                      onClick={(e) => {
                        e.stopPropagation();
                        setPlaylistToDelete(playlist);
                      }}
                    >
                      <Trash2 size={15} />
                    </button>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <div className="library-empty compact-empty">
              <ListMusic size={22} />
              <h2>No playlists yet</h2>
              <p>Create a playlist to organize tracks from your library.</p>
              <Button variant="primary" size="sm" icon={<Plus size={14} />} onClick={() => setIsCreateOpen(true)}>
                New playlist
              </Button>
            </div>
          )}
        </section>
      )}

      {filter === 'liked' && (
        <section className="library-section" aria-label="Liked tracks">
          <div className="section-title-row">
            <h2>Liked</h2>
            {likedTracks.length > 0 && (
              <div className="section-actions">
                <button
                  type="button"
                  className={`btn-manage-toggle ${managingSection === 'liked' ? 'active' : ''}`}
                  onClick={() => handleToggleManage('liked')}
                  title="Manage liked tracks"
                >
                  <CheckSquare size={14} />
                  {managingSection === 'liked' ? 'Done' : 'Manage'}
                </button>
                <button type="button" className="text-link" onClick={onPlayAllLiked} disabled={!isBackendReady}>
                  <Play size={14} /> Play all
                </button>
                <button type="button" className="text-link" onClick={onShuffleAllLiked} disabled={!isBackendReady}>
                  <Shuffle size={14} /> Shuffle
                </button>
              </div>
            )}
          </div>

          {renderSelectionBar('liked', likedTracks, 'unlike')}

          {likedTracks.length > 0 ? (
            <TrackList
              tracks={likedTracks}
              density={density}
              onPlayTrack={onPlayLikedTrack}
              ariaLabel="Liked tracks"
              manageMode={managingSection === 'liked'}
              selectedTrackIds={managingSection === 'liked' ? selectedTrackIds : emptyTrackIdSet}
              onToggleSelectTrack={(id) => handleToggleSelectTrack(id, 'liked')}
              onRangeSelectTracks={(id) => handleRangeSelectTracks(id, likedTracks, 'liked')}
              onEnterManageMode={() => handleEnterManage('liked')}
              {...trackListProps}
            />
          ) : (
            <div className="library-empty compact-empty">
              <Heart size={22} />
              <h2>Nothing liked yet</h2>
              <p>Use the heart action on any track to keep it here.</p>
            </div>
          )}
        </section>
      )}

      </div>

      <Modal
        isOpen={isSpotifyOpen}
        onClose={() => setIsSpotifyOpen(false)}
        title="Import from Spotify"
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setIsSpotifyOpen(false)}>
              {spotifyResult ? 'Done' : 'Cancel'}
            </Button>
            <Button
              variant="primary"
              size="sm"
              icon={spotifyBusy ? <Loader2 size={14} className="spinning" /> : <Link2 size={14} />}
              disabled={!spotifyUrl.trim() || spotifyBusy}
              onClick={handleSpotifyImport}
            >
              {spotifyBusy ? 'Importing…' : 'Import'}
            </Button>
          </div>
        }
      >
        <form onSubmit={handleSpotifyImport} className="cavia-form">
          <div className="form-field">
            <label htmlFor="spotify-url">Public playlist link</label>
            <input
              id="spotify-url"
              type="text"
              required
              autoFocus
              placeholder="https://open.spotify.com/playlist/…"
              value={spotifyUrl}
              onChange={(event) => setSpotifyUrl(event.target.value)}
              className="cavia-text-input"
            />
          </div>

          <p className="library-muted-note">
            No Spotify sign-in needed. Tracks already in your library are linked directly; the
            rest are matched on YouTube Music and stream from there. Nothing leaves your device.
          </p>

          {spotifyError && (
            <div className="cavia-banner banner-error" role="alert">
              {spotifyError}
            </div>
          )}

          {spotifyResult && (
            <div className="spotify-import-summary" role="status">
              <p>
                <strong>{spotifyResult.playlistName}</strong> added to your playlists.
              </p>
              <p className="library-muted-note">
                {spotifyResult.total} found · {spotifyResult.matchedLocal} from your library ·{' '}
                {spotifyResult.matchedOnline} streaming · {spotifyResult.skipped.length} skipped
              </p>
              {spotifyResult.skipped.length > 0 && (
                <p className="library-muted-note">
                  Skipped (no confident match): {spotifyResult.skipped.slice(0, 4).join(', ')}
                  {spotifyResult.skipped.length > 4 ? '…' : ''}
                </p>
              )}
            </div>
          )}
        </form>
      </Modal>

      <Modal
        isOpen={isCreateOpen}
        onClose={() => setIsCreateOpen(false)}
        title="New playlist"
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setIsCreateOpen(false)}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" disabled={!newTitle.trim()} onClick={handleCreateSubmit}>
              Create
            </Button>
          </div>
        }
      >
        <form onSubmit={handleCreateSubmit} className="cavia-form">
          <div className="form-field">
            <label htmlFor="playlist-title">Title</label>
            <input
              id="playlist-title"
              type="text"
              required
              autoFocus
              placeholder="Night driving"
              value={newTitle}
              onChange={(event) => setNewTitle(event.target.value)}
              className="cavia-text-input"
            />
          </div>
          <div className="form-field">
            <label htmlFor="playlist-desc">Description</label>
            <textarea
              id="playlist-desc"
              rows={3}
              placeholder="Optional"
              value={newDesc}
              onChange={(event) => setNewDesc(event.target.value)}
              className="cavia-textarea"
            />
          </div>
        </form>
      </Modal>

      <PlaylistDetailModal
        playlist={activePlaylist}
        isOpen={Boolean(activePlaylist)}
        onClose={() => setSelectedPlaylist(null)}
        isBackendReady={isBackendReady}
        onDeletePlaylist={handleDeletePlaylist}
        onPlayTracks={onPlayTracks}
        onPlaylistUpdated={onPlaylistUpdated}
      />

      {/* Delete Playlist Confirmation Modal */}
      <Modal
        isOpen={Boolean(playlistToDelete)}
        onClose={() => setPlaylistToDelete(null)}
        title="Delete playlist"
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setPlaylistToDelete(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              className="btn-danger"
              disabled={isDeletingPlaylist}
              onClick={async () => {
                if (!playlistToDelete) return;
                setIsDeletingPlaylist(true);
                try {
                  await onDeletePlaylist?.(playlistToDelete.id);
                  setPlaylistToDelete(null);
                } finally {
                  setIsDeletingPlaylist(false);
                }
              }}
            >
              {isDeletingPlaylist ? 'Deleting…' : 'Delete'}
            </Button>
          </div>
        }
      >
        <p className="modal-body-text">
          Are you sure you want to delete <strong>{playlistToDelete?.name}</strong>?
          This will remove the playlist. Your music files on disk will not be affected.
        </p>
      </Modal>

      {/* Remove Track from Library Confirmation Modal */}
      <Modal
        isOpen={Boolean(trackToDelete)}
        onClose={() => setTrackToDelete(null)}
        title="Remove from library"
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setTrackToDelete(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              className="btn-danger"
              disabled={isDeletingTrack}
              onClick={async () => {
                if (!trackToDelete) return;
                setIsDeletingTrack(true);
                try {
                  await onDeleteTrack?.(trackToDelete.id);
                  setTrackToDelete(null);
                } finally {
                  setIsDeletingTrack(false);
                }
              }}
            >
              {isDeletingTrack ? 'Removing…' : 'Remove'}
            </Button>
          </div>
        }
      >
        <p className="modal-body-text">
          Remove <strong>{trackToDelete?.title}</strong> from your library?
          This will remove it from all playlists and library views.
          The audio file on your computer will <strong>not</strong> be deleted.
        </p>
      </Modal>

      {/* Batch Remove Tracks Confirmation Modal */}
      <Modal
        isOpen={showBatchDeleteConfirm}
        onClose={() => setShowBatchDeleteConfirm(false)}
        title={`Remove ${selectedTrackIds.size} ${selectedTrackIds.size === 1 ? 'track' : 'tracks'} from library?`}
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setShowBatchDeleteConfirm(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              className="btn-danger"
              disabled={isDeletingBatch}
              onClick={handleConfirmBatchDelete}
              icon={<Trash2 size={14} />}
            >
              {isDeletingBatch
                ? 'Removing…'
                : `Remove ${selectedTrackIds.size} ${selectedTrackIds.size === 1 ? 'track' : 'tracks'}`}
            </Button>
          </div>
        }
      >
        <p className="modal-body-text">
          Remove <strong>{selectedTrackIds.size}</strong> selected {selectedTrackIds.size === 1 ? 'track' : 'tracks'} from your library?
          This will remove them from all playlists and library views.
          The audio files on your computer will <strong>not</strong> be deleted.
        </p>
      </Modal>

      {/* Batch Unlike Tracks Confirmation Modal */}
      <Modal
        isOpen={showBatchUnlikeConfirm}
        onClose={() => setShowBatchUnlikeConfirm(false)}
        title={`Remove ${selectedTrackIds.size} ${selectedTrackIds.size === 1 ? 'track' : 'tracks'} from Liked?`}
        size="sm"
        footer={
          <div className="modal-actions-right">
            <Button variant="ghost" size="sm" onClick={() => setShowBatchUnlikeConfirm(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              className="btn-danger"
              disabled={isDeletingBatch}
              onClick={handleConfirmBatchUnlike}
              icon={<Heart size={14} />}
            >
              {isDeletingBatch
                ? 'Removing…'
                : `Remove ${selectedTrackIds.size} ${selectedTrackIds.size === 1 ? 'track' : 'tracks'}`}
            </Button>
          </div>
        }
      >
        <p className="modal-body-text">
          Remove <strong>{selectedTrackIds.size}</strong> selected {selectedTrackIds.size === 1 ? 'track' : 'tracks'} from your Liked Songs?
          They will remain indexed in your library and in playlists.
        </p>
      </Modal>
    </section>
  );
}
