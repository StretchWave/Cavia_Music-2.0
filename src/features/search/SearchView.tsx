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
import { Modal } from '../../components/Modal';
import { ArtworkImage } from '../../components/ArtworkImage';
import { TrackList } from '../../components/TrackList';
import { AlbumCard, ArtistCard } from '../../components/MediaCards';
import { PlaylistDetailModal } from '../library/PlaylistDetailModal';
import {
  clearRecentPlays,
  deleteRecentPlay,
  formatArtworkUri,
  getRecentPlays,
  searchLibrary,
} from '../../lib/library';
import {
  formatOnlineDuration,
  isOnlineSearchAvailable,
  playYouTubeMusic,
  searchYouTubeMusic,
  type YtMusicTrack,
} from '../../lib/ytmusic';
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
  onPlayOnlineTrack?: (track: YtMusicTrack) => Promise<void>;
  historyRevision?: number;
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
  onPlayOnlineTrack,
  historyRevision = 0,
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
  const [recentPlays, setRecentPlays] = useState<Track[]>([]);
  const [showClearHistoryConfirm, setShowClearHistoryConfirm] = useState(false);
  const [isClearingHistory, setIsClearingHistory] = useState(false);

  // YouTube Music catalogue results. These are online, never persisted into the
  // local library, and only shown for All/Tracks where they make sense.
  const [onlineResults, setOnlineResults] = useState<YtMusicTrack[]>([]);
  const [onlineLoading, setOnlineLoading] = useState(false);
  const [onlineError, setOnlineError] = useState<string | null>(null);
  const [onlineRetryToken, setOnlineRetryToken] = useState(0);
  const [playingOnlineId, setPlayingOnlineId] = useState<string | null>(null);
  const onlineAvailable = isOnlineSearchAvailable();

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

  useEffect(() => {
    const trimmed = debouncedQuery.trim();
    if (!trimmed || !onlineAvailable) {
      setOnlineResults([]);
      setOnlineLoading(false);
      setOnlineError(null);
      return;
    }

    let active = true;
    setOnlineLoading(true);
    setOnlineError(null);

    searchYouTubeMusic(trimmed)
      .then((items) => {
        if (!active) return;
        setOnlineResults(items);
        setOnlineLoading(false);
      })
      .catch((err: unknown) => {
        if (!active) return;
        setOnlineResults([]);
        setOnlineError(err instanceof Error ? err.message : String(err));
        setOnlineLoading(false);
      });

    return () => {
      active = false;
    };
  }, [debouncedQuery, onlineRetryToken, onlineAvailable]);

  useEffect(() => {
    if (!isBackendReady) return;
    let active = true;
    getRecentPlays()
      .then((rows) => {
        if (active) setRecentPlays(rows);
      })
      .catch((err) => console.error('Failed to load playback history:', err));
    return () => {
      active = false;
    };
  }, [isBackendReady, currentTrackId, historyRevision]);

  const handlePlayRecent = (trackId: string) => {
    const entry = recentPlays.find((track) => track.id === trackId);
    if (!entry) return;

    if (entry.sourceKind === 'provider') {
      const videoId = entry.id.replace(/^ytmusic:/, '');
      if (videoId.length !== 11) return;
      void handlePlayOnline({
        videoId,
        title: entry.title,
        artist: entry.artist,
        album: entry.album,
        durationMs: entry.durationMs,
        artworkUrl: entry.artworkUri,
      });
      return;
    }

    const localTrack = allTracks.find((track) => track.id === entry.id);
    if (localTrack) onPlayTracks?.([localTrack], 0, false);
  };

  const handleClearHistory = async () => {
    setIsClearingHistory(true);
    try {
      await clearRecentPlays();
      setRecentPlays([]);
      setShowClearHistoryConfirm(false);
    } catch (err) {
      console.error('Failed to clear playback history:', err);
    } finally {
      setIsClearingHistory(false);
    }
  };

  const handleRemoveRecent = async (trackId: string) => {
    try {
      await deleteRecentPlay(trackId);
      setRecentPlays((prev) => prev.filter((t) => t.id !== trackId));
    } catch (err) {
      console.error('Failed to remove recent play:', err);
    }
  };

  const handlePlayOnline = async (track: YtMusicTrack) => {
    if (playingOnlineId) return;
    setPlayingOnlineId(track.videoId);
    setOnlineError(null);
    try {
      if (onPlayOnlineTrack) {
        await onPlayOnlineTrack(track);
      } else {
        await playYouTubeMusic(track);
      }
    } catch (err) {
      setOnlineError(err instanceof Error ? err.message : String(err));
    } finally {
      setPlayingOnlineId(null);
    }
  };

  // Local entries that no longer exist in the library are hidden rather than
  // shown as broken rows; catalogue entries replay from their provider id.
  const visibleRecent = useMemo(
    () =>
      recentPlays.filter(
        (entry) =>
          entry.sourceKind === 'provider' ||
          allTracks.some((track) => track.id === entry.id)
      ),
    [recentPlays, allTracks]
  );

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

      {!isQueryActive && (
        <section className="search-section" aria-label="Recently played">
          <div className="section-title-row">
            <h2>Recently played</h2>
            {visibleRecent.length > 0 && (
              <div className="section-title-actions">
                <span className="result-count">{visibleRecent.length}</span>
                <button
                  type="button"
                  className="text-link text-danger"
                  onClick={() => setShowClearHistoryConfirm(true)}
                  title="Clear playback history"
                >
                  Clear history
                </button>
              </div>
            )}
          </div>
          {visibleRecent.length > 0 ? (
            <TrackList
              tracks={visibleRecent}
              density={density}
              currentTrackId={currentTrackId}
              isPlaying={isPlaying}
              isBackendReady={isBackendReady}
              onPlayTrack={handlePlayRecent}
              onRemoveFromHistory={handleRemoveRecent}
              ariaLabel="Recently played tracks"
            />
          ) : (
            <div className="search-empty">
              <h2>No playback history yet</h2>
              <p>Play a result and it will appear here, newest first.</p>
            </div>
          )}
        </section>
      )}

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

      {isQueryActive &&
        !isLoading &&
        results &&
        results.totalCount === 0 &&
        !onlineLoading &&
        onlineResults.length === 0 &&
        !onlineError && (
          <div className="search-empty">
            <h2>Nothing found for &ldquo;{debouncedQuery || query}&rdquo;</h2>
            <p>Try a different spelling.</p>
          </div>
        )}

      {isQueryActive && onlineAvailable && (category === 'all' || category === 'tracks') && (
        <section className="search-section" aria-label="YouTube Music results">
          <div className="section-title-row">
            <h2>YouTube Music</h2>
            {onlineResults.length > 0 && <span className="result-count">{onlineResults.length}</span>}
          </div>
          <p className="yt-result-note">
            Streamed from YouTube Music. Nothing is added to your library; played audio is cached locally for seeking.
          </p>

          {onlineLoading && (
            <div className="search-loading-line">
              <Loader2 size={16} className="search-loading-spinner" />
              <span>Searching YouTube Music…</span>
            </div>
          )}

          {!onlineLoading && onlineError && (
            <div className="yt-result-error" role="alert">
              <XCircle size={16} />
              <span>{onlineError}</span>
              <Button variant="outline" size="sm" onClick={() => setOnlineRetryToken((value) => value + 1)}>
                Retry
              </Button>
            </div>
          )}

          {!onlineLoading && !onlineError && onlineResults.length === 0 && (
            <p className="library-muted-note">No YouTube Music match for this search.</p>
          )}

          {!onlineLoading && onlineResults.length > 0 && (
            <div className="playlist-rows">
              {onlineResults.map((track) => {
                const isResolving = playingOnlineId === track.videoId;
                return (
                  <button
                    key={track.videoId}
                    type="button"
                    className="playlist-row"
                    onClick={() => void handlePlayOnline(track)}
                    disabled={playingOnlineId !== null}
                    aria-label={`Play ${track.title} by ${track.artist} from YouTube Music`}
                  >
                    <ArtworkImage
                      src={track.artworkUrl}
                      alt=""
                      className="yt-result-art"
                      fallbackIcon={<Music2 size={16} />}
                    />
                    <span className="playlist-row-copy">
                      <strong>{track.title}</strong>
                      <span>
                        {track.artist || 'Unknown artist'}
                        {track.album ? ` · ${track.album}` : ''}
                      </span>
                    </span>
                    {isResolving ? (
                      <Loader2 size={16} className="search-loading-spinner" aria-hidden="true" />
                    ) : (
                      <Play size={15} aria-hidden="true" />
                    )}
                    <span className="yt-result-duration">{formatOnlineDuration(track.durationMs)}</span>
                  </button>
                );
              })}
            </div>
          )}
        </section>
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
                  // Play only the selected result. Queueing the whole result
                  // list made autoplay continue into a near-identical name
                  // (a remix or another upload) instead of a related song.
                  const selected = results.tracks.find((track) => track.id === trackId);
                  if (selected) onPlayTracks?.([selected], 0, false);
                }}
                ariaLabel="Search track results"
                {...trackListProps}
              />
              {category === 'all' && results.tracks.length >= 20 && (
                <div className="search-more-row">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setCategory('tracks')}
                    className="search-more-btn"
                  >
                    View all matching tracks →
                  </Button>
                </div>
              )}
              {category === 'tracks' && results.tracks.length >= 50 && (
                <p className="library-muted-note" style={{ marginTop: '0.75rem' }}>
                  Showing top 50 matches. Refine your query for more specific results.
                </p>
              )}
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
              {category === 'all' && results.albums.length >= 10 && (
                <div className="search-more-row">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setCategory('albums')}
                    className="search-more-btn"
                  >
                    View all matching albums →
                  </Button>
                </div>
              )}
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
              {category === 'all' && results.artists.length >= 10 && (
                <div className="search-more-row">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setCategory('artists')}
                    className="search-more-btn"
                  >
                    View all matching artists →
                  </Button>
                </div>
              )}
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
              {category === 'all' && results.playlists.length >= 10 && (
                <div className="search-more-row">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setCategory('playlists')}
                    className="search-more-btn"
                  >
                    View all matching playlists →
                  </Button>
                </div>
              )}
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

      {/* Clear History Confirmation Modal */}
      <Modal
        isOpen={showClearHistoryConfirm}
        onClose={() => setShowClearHistoryConfirm(false)}
        title="Clear playback history"
        size="sm"
        role="alertdialog"
        footer={
          <div className="modal-actions-right">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowClearHistoryConfirm(false)}
            >
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              className="btn-danger"
              disabled={isClearingHistory}
              onClick={handleClearHistory}
            >
              {isClearingHistory ? 'Clearing…' : 'Clear history'}
            </Button>
          </div>
        }
      >
        <p className="modal-body-text">
          Are you sure you want to clear your playback history?
          This will not remove any tracks from your library or playlists.
        </p>
      </Modal>
    </section>
  );
}
