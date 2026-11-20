import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import {
  ArrowLeft,
  Disc3,
  Loader2,
  Music2,
  Play,
  Shuffle,
  XCircle,
} from 'lucide-react';
import { SearchInput } from '../../components/SearchInput';
import { Tabs } from '../../components/Tabs';
import { Button } from '../../components/Button';
import { ArtworkImage } from '../../components/ArtworkImage';
import { TrackList } from '../../components/TrackList';
import { AlbumCard, ArtistCard } from '../../components/MediaCards';
import { PlaylistDetailModal } from '../library/PlaylistDetailModal';
import { formatArtworkUri, searchLibrary } from '../../lib/library';
import type { Album, Artist, Playlist, SearchCategory, SearchResultsDTO, Track } from '../../lib/types';

export interface SearchViewProps {
  isBackendReady?: boolean;
  density?: 'standard' | 'compact';
  onPlayTracks?: (tracks: Track[], startIndex?: number, shuffle?: boolean) => void;
  onToggleLike?: (trackId: string) => void;
  onAddToPlaylist?: (track: Track) => void;
  onAddAlbumToPlaylist?: (album: Album) => void;
  onPlaylistUpdated?: () => void;
  onDeletePlaylist?: (playlistId: string) => void;
  onPlayAlbum?: (album: Album) => void;
  onPlayArtist?: (artist: Artist) => void;
  onPlayNext?: (trackId: string) => void;
  allTracks?: Track[];
  likedTrackIds?: Set<string>;
  currentTrackId?: string;
  isPlaying?: boolean;
}

export function SearchView({
  isBackendReady = false,
  density = 'standard',
  onPlayTracks,
  onToggleLike,
  onAddToPlaylist,
  onAddAlbumToPlaylist,
  onPlaylistUpdated,
  onDeletePlaylist,
  onPlayAlbum,
  onPlayArtist,
  onPlayNext,
  allTracks = [],
  currentTrackId,
  isPlaying = false,
}: SearchViewProps) {
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [category, setCategory] = useState<SearchCategory>('all');
  const [results, setResults] = useState<SearchResultsDTO | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [selectedAlbum, setSelectedAlbum] = useState<Album | null>(null);
  const [selectedArtist, setSelectedArtist] = useState<Artist | null>(null);
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);

  const requestIdRef = useRef(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query), 180);
    return () => clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    const trimmed = debouncedQuery.trim();

    if (!trimmed) {
      requestIdRef.current += 1;
      setResults(null);
      setIsLoading(false);
      setSearchError(null);
      return;
    }

    let active = true;
    const requestId = ++requestIdRef.current;
    setIsLoading(true);
    setSearchError(null);

    searchLibrary(trimmed, category)
      .then((data) => {
        if (!active || requestId !== requestIdRef.current) return;
        setResults(data);
        setIsLoading(false);
      })
      .catch((err) => {
        if (!active || requestId !== requestIdRef.current) return;
        setSearchError(err instanceof Error ? err.message : String(err));
        setIsLoading(false);
      });

    return () => {
      active = false;
    };
  }, [debouncedQuery, category]);

  useEffect(() => {
    setSelectedAlbum(null);
    setSelectedArtist(null);
  }, [debouncedQuery, category]);

  const albumTracks = useMemo(() => {
    if (!selectedAlbum) return [];
    return allTracks.filter(
      (track) =>
        track.album.toLowerCase() === selectedAlbum.title.toLowerCase() &&
        (track.albumArtist?.toLowerCase() === selectedAlbum.artist.toLowerCase() ||
          track.artist.toLowerCase() === selectedAlbum.artist.toLowerCase())
    );
  }, [allTracks, selectedAlbum]);

  const artistTracks = useMemo(() => {
    if (!selectedArtist) return [];
    return allTracks.filter(
      (track) =>
        track.artist.toLowerCase() === selectedArtist.name.toLowerCase() ||
        track.albumArtist?.toLowerCase() === selectedArtist.name.toLowerCase()
    );
  }, [allTracks, selectedArtist]);

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      setDebouncedQuery(query.trim());
    }
  };

  const isQueryActive = Boolean(query.trim());
  const hasResults = Boolean(results && results.totalCount > 0);

  const clearSearch = () => {
    setQuery('');
    setDebouncedQuery('');
    setResults(null);
    setSelectedAlbum(null);
    setSelectedArtist(null);
  };

  const trackListProps = {
    currentTrackId,
    isPlaying,
    isBackendReady,
    onToggleLike,
    onAddToPlaylist,
    onPlayNext,
  };

  if (selectedAlbum) {
    return (
      <section className="cavia-library-page search-page">
        <button type="button" className="back-link" onClick={() => setSelectedAlbum(null)}>
          <ArrowLeft size={15} /> Search results
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
                onClick={() => onPlayTracks?.(albumTracks, 0, false)}
              >
                Play
              </Button>
              <Button
                variant="outline"
                size="sm"
                icon={<Shuffle size={14} />}
                disabled={!isBackendReady || albumTracks.length === 0}
                onClick={() => onPlayTracks?.(albumTracks, 0, true)}
              >
                Shuffle
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={!onAddAlbumToPlaylist}
                onClick={() => onAddAlbumToPlaylist?.(selectedAlbum)}
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
              onPlayTracks?.(albumTracks, index >= 0 ? index : 0, false);
            }}
            ariaLabel={`Tracks from ${selectedAlbum.title}`}
            {...trackListProps}
          />
        ) : (
          <p className="library-muted-note">No indexed tracks for this album.</p>
        )}
      </section>
    );
  }

  if (selectedArtist) {
    return (
      <section className="cavia-library-page search-page">
        <button type="button" className="back-link" onClick={() => setSelectedArtist(null)}>
          <ArrowLeft size={15} /> Search results
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
                onClick={() => onPlayTracks?.(artistTracks, 0, false)}
              >
                Play
              </Button>
              <Button
                variant="outline"
                size="sm"
                icon={<Shuffle size={14} />}
                disabled={!isBackendReady || artistTracks.length === 0}
                onClick={() => onPlayTracks?.(artistTracks, 0, true)}
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
              onPlayTracks?.(artistTracks, index >= 0 ? index : 0, false);
            }}
            ariaLabel={`Tracks by ${selectedArtist.name}`}
            {...trackListProps}
          />
        ) : (
          <p className="library-muted-note">No indexed tracks for this artist.</p>
        )}
      </section>
    );
  }

  return (
    <section className="cavia-library-page search-page">
      <header className="library-header search-header">
        <div className="library-heading">
          <p className="page-eyebrow">Find in library</p>
          <h1 className="library-title">
            Search<span>.</span>
          </h1>
        </div>
      </header>

      <div className="search-bar-wrap">
        <SearchInput
          ref={inputRef}
          value={query}
          onChange={setQuery}
          onKeyDown={handleKeyDown}
          onClear={clearSearch}
          isLoading={isLoading}
          placeholder="Search your library"
          shortcut=""
          className="search-main-input"
        />
      </div>

      <div className="sr-only" role="status" aria-live="polite">
        {isQueryActive && results
          ? `${results.totalCount} ${results.totalCount === 1 ? 'result' : 'results'} for ${debouncedQuery}`
          : ''}
      </div>

      {isQueryActive && (
        <div className="search-category-row">
          <Tabs
            items={[
              { id: 'all', label: 'All' },
              { id: 'tracks', label: 'Tracks' },
              { id: 'albums', label: 'Albums' },
              { id: 'artists', label: 'Artists' },
              { id: 'playlists', label: 'Playlists' },
            ]}
            activeId={category}
            onChange={(id) => setCategory(id as SearchCategory)}
            variant="segmented"
            ariaLabel="Search result categories"
          />
        </div>
      )}

      {searchError && (
        <div className="cavia-banner banner-error search-error" role="alert">
          <XCircle size={16} />
          <span>Search failed: {searchError}</span>
        </div>
      )}

      {isQueryActive && isLoading && !results && (
        <div className="search-loading-line">
          <Loader2 size={16} className="search-loading-spinner" />
          <span>Searching your library…</span>
        </div>
      )}

      {isQueryActive && !isLoading && results && results.totalCount === 0 && (
        <div className="search-empty">
          <h2>Nothing found for &ldquo;{debouncedQuery || query}&rdquo;</h2>
          <p>Try a different spelling.</p>
        </div>
      )}

      {isQueryActive && hasResults && results && (
        <div className="search-results">
          {(category === 'all' || category === 'tracks') && results.tracks.length > 0 && (
            <section className="search-section" aria-label="Track results">
              <div className="section-title-row">
                <h2>Tracks</h2>
                <span className="result-count">{results.tracks.length}</span>
              </div>
              <TrackList
                tracks={results.tracks}
                density={density}
                onPlayTrack={(trackId) => {
                  const index = results.tracks.findIndex((track) => track.id === trackId);
                  onPlayTracks?.(results.tracks, index >= 0 ? index : 0, false);
                }}
                ariaLabel="Search track results"
                {...trackListProps}
              />
            </section>
          )}

          {(category === 'all' || category === 'albums') && results.albums.length > 0 && (
            <section className="search-section" aria-label="Album results">
              <div className="section-title-row">
                <h2>Albums</h2>
                <span className="result-count">{results.albums.length}</span>
              </div>
              <div className="media-grid">
                {results.albums.map((album) => (
                  <AlbumCard
                    key={album.id}
                    album={album}
                    onOpen={setSelectedAlbum}
                    onPlay={onPlayAlbum}
                    playDisabled={!isBackendReady}
                  />
                ))}
              </div>
            </section>
          )}

          {(category === 'all' || category === 'artists') && results.artists.length > 0 && (
            <section className="search-section" aria-label="Artist results">
              <div className="section-title-row">
                <h2>Artists</h2>
                <span className="result-count">{results.artists.length}</span>
              </div>
              <div className="media-grid">
                {results.artists.map((artist) => (
                  <ArtistCard
                    key={artist.id}
                    artist={artist}
                    onOpen={setSelectedArtist}
                    onPlay={onPlayArtist}
                    playDisabled={!isBackendReady}
                  />
                ))}
              </div>
            </section>
          )}

          {(category === 'all' || category === 'playlists') && results.playlists.length > 0 && (
            <section className="search-section" aria-label="Playlist results">
              <div className="section-title-row">
                <h2>Playlists</h2>
                <span className="result-count">{results.playlists.length}</span>
              </div>
              <div className="playlist-rows">
                {results.playlists.map((playlist) => (
                  <button
                    key={playlist.id}
                    type="button"
                    className="playlist-row"
                    onClick={() => setSelectedPlaylist(playlist)}
                  >
                    <span className="playlist-row-icon" aria-hidden="true">
                      <Music2 size={18} />
                    </span>
                    <span className="playlist-row-copy">
                      <strong>{playlist.name}</strong>
                      <span>
                        {playlist.trackCount} {playlist.trackCount === 1 ? 'track' : 'tracks'}
                        {playlist.description ? ` · ${playlist.description}` : ''}
                      </span>
                    </span>
                  </button>
                ))}
              </div>
            </section>
          )}
        </div>
      )}

      {selectedPlaylist && (
        <PlaylistDetailModal
          playlist={selectedPlaylist}
          isOpen
          onClose={() => setSelectedPlaylist(null)}
          isBackendReady={isBackendReady}
          onDeletePlaylist={onDeletePlaylist}
          onPlayTracks={onPlayTracks}
          onPlaylistUpdated={onPlaylistUpdated}
        />
      )}
    </section>
  );
}
