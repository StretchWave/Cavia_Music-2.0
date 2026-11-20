import { useMemo, useState, type FormEvent } from 'react';
import {
  ArrowLeft,
  AudioWaveform,
  ChevronRight,
  Disc3,
  FolderPlus,
  Heart,
  ListMusic,
  Music2,
  Plus,
  Play,
  Shuffle,
  WifiOff,
} from 'lucide-react';
import { Button } from '../../components/Button';
import { Modal } from '../../components/Modal';
import { ArtworkImage } from '../../components/ArtworkImage';
import { TrackList } from '../../components/TrackList';
import { AlbumCard, ArtistCard } from '../../components/MediaCards';
import { PlaylistDetailModal } from './PlaylistDetailModal';
import { formatArtworkUri } from '../../lib/library';
import type { Album, Artist, Playlist, Track } from '../../lib/types';

type LibraryFilter = 'all' | 'tracks' | 'albums' | 'artists' | 'playlists' | 'liked';

interface LibraryViewProps {
  connectionError: string | null;
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
  onAddAlbumToPlaylist: (album: Album) => void;
  onPlayNext: (trackId: string) => void;
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
  onAddAlbumToPlaylist,
  onPlayNext,
}: LibraryViewProps) {
  const [filter, setFilter] = useState<LibraryFilter>('all');
  const [selectedAlbum, setSelectedAlbum] = useState<Album | null>(null);
  const [selectedArtist, setSelectedArtist] = useState<Artist | null>(null);
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newDesc, setNewDesc] = useState('');

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
  };

  const handleCreateSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (!newTitle.trim()) return;
    onCreatePlaylist(newTitle.trim(), newDesc.trim());
    setNewTitle('');
    setNewDesc('');
    setIsCreateOpen(false);
  };

  const handleDeletePlaylist = (playlistId: string) => {
    onDeletePlaylist?.(playlistId);
    if (selectedPlaylist?.id === playlistId) {
      setSelectedPlaylist(null);
    }
  };

  const trackListProps = {
    currentTrackId,
    isPlaying,
    isBackendReady,
    onToggleLike,
    onAddToPlaylist,
    onPlayNext,
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
      <Button variant="primary" size="sm" icon={<Plus size={15} />} onClick={() => setIsCreateOpen(true)}>
        New playlist
      </Button>
    </div>
  );

  return (
    <section className="cavia-library-page">
      <header className="library-header">
        <div className="library-heading">
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

      <div className="library-chips" role="tablist" aria-label="Library sections">
        {FILTERS.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={filter === item.id}
            className={`library-chip ${filter === item.id ? 'active' : ''}`}
            onClick={() => selectFilter(item.id)}
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

      {filter === 'all' && (
        <>
          {tracks.length > 0 ? (
            <section className="library-section" aria-label="Recently added">
              <div className="section-title-row">
                <h2>Recently added</h2>
                <button type="button" className="text-link" onClick={() => selectFilter('tracks')}>
                  View all
                </button>
              </div>
              <TrackList
                tracks={recentTracks}
                density={density}
                onPlayTrack={onPlayTrack}
                ariaLabel="Recently added tracks"
                {...trackListProps}
              />
            </section>
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

          <section className="library-section" aria-label="Browse library">
            <div className="section-title-row">
              <h2>Browse</h2>
            </div>
            <div className="browse-list">
              <button type="button" className="browse-row" onClick={() => selectFilter('tracks')}>
                <Music2 size={18} />
                <span className="browse-row-label">Tracks</span>
                <span className="browse-row-count">{tracks.length}</span>
                <ChevronRight size={16} />
              </button>
              <button type="button" className="browse-row" onClick={() => selectFilter('albums')}>
                <Disc3 size={18} />
                <span className="browse-row-label">Albums</span>
                <span className="browse-row-count">{albums.length}</span>
                <ChevronRight size={16} />
              </button>
              <button type="button" className="browse-row" onClick={() => selectFilter('artists')}>
                <AudioWaveform size={18} />
                <span className="browse-row-label">Artists</span>
                <span className="browse-row-count">{artists.length}</span>
                <ChevronRight size={16} />
              </button>
              <button type="button" className="browse-row" onClick={() => selectFilter('playlists')}>
                <ListMusic size={18} />
                <span className="browse-row-label">Playlists</span>
                <span className="browse-row-count">{playlists.length}</span>
                <ChevronRight size={16} />
              </button>
              <button type="button" className="browse-row" onClick={() => selectFilter('liked')}>
                <Heart size={18} />
                <span className="browse-row-label">Liked</span>
                <span className="browse-row-count">{likedTracks.length}</span>
                <ChevronRight size={16} />
              </button>
            </div>
          </section>
        </>
      )}

      {filter === 'tracks' && (
        <section className="library-section" aria-label="All tracks">
          <div className="section-title-row">
            <h2>All tracks</h2>
            {tracks.length > 0 && (
              <div className="section-actions">
                <button type="button" className="text-link" onClick={onPlayAll} disabled={!isBackendReady}>
                  <Play size={14} /> Play all
                </button>
                <button type="button" className="text-link" onClick={onShuffleAll} disabled={!isBackendReady}>
                  <Shuffle size={14} /> Shuffle
                </button>
              </div>
            )}
          </div>
          {tracks.length > 0 ? (
            <TrackList
              tracks={tracks}
              density={density}
              onPlayTrack={onPlayTrack}
              ariaLabel="All tracks"
              {...trackListProps}
            />
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
                onClick={() => setSelectedAlbum(null)}
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

              {albumTracks.length > 0 ? (
                <TrackList
                  tracks={albumTracks}
                  density={density}
                  onPlayTrack={(trackId) => {
                    const index = albumTracks.findIndex((track) => track.id === trackId);
                    onPlayTracks(albumTracks, index >= 0 ? index : 0, false);
                  }}
                  ariaLabel={`Tracks from ${selectedAlbum.title}`}
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
              <button type="button" className="back-link" onClick={() => setSelectedArtist(null)}>
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
                  </div>
                </div>
              </div>

              {artistTracks.length > 0 ? (
                <TrackList
                  tracks={artistTracks}
                  density={density}
                  onPlayTrack={(trackId) => {
                    const index = artistTracks.findIndex((track) => track.id === trackId);
                    onPlayTracks(artistTracks, index >= 0 ? index : 0, false);
                  }}
                  ariaLabel={`Tracks by ${selectedArtist.name}`}
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
                <button
                  key={playlist.id}
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
                <button type="button" className="text-link" onClick={onPlayAllLiked} disabled={!isBackendReady}>
                  <Play size={14} /> Play all
                </button>
                <button type="button" className="text-link" onClick={onShuffleAllLiked} disabled={!isBackendReady}>
                  <Shuffle size={14} /> Shuffle
                </button>
              </div>
            )}
          </div>
          {likedTracks.length > 0 ? (
            <TrackList
              tracks={likedTracks}
              density={density}
              onPlayTrack={onPlayLikedTrack}
              ariaLabel="Liked tracks"
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
    </section>
  );
}
