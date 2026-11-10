import { useState, useEffect, useRef, type KeyboardEvent } from 'react';
import {
  Clock,
  Disc3,
  Heart,
  ListPlus,
  Loader2,
  RadioTower,
  Search,
  Sparkles,
  Trash2,
  XCircle,
} from 'lucide-react';
import { SearchInput } from '../../components/SearchInput';
import { Tabs } from '../../components/Tabs';
import { Badge } from '../../components/Badge';
import { Button } from '../../components/Button';
import { PlaylistDetailModal } from '../library/PlaylistDetailModal';
import { formatArtworkUri, searchLibrary } from '../../lib/library';
import type { Album, Artist, Playlist, SearchCategory, SearchResultsDTO, Track } from '../../lib/types';

function formatDuration(ms: number): string {
  if (!ms || ms <= 0) return '--:--';
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

export interface SearchViewProps {
  isBackendReady?: boolean;
  onPlayTrack?: (trackId: string) => void;
  onPlayTracks?: (tracks: Track[], startIndex?: number, shuffle?: boolean) => void;
  onToggleLike?: (trackId: string) => void;
  onAddToPlaylist?: (track: Track) => void;
  onAddAlbumToPlaylist?: (album: Album) => void;
  onPlaylistUpdated?: () => void;
  onDeletePlaylist?: (playlistId: string) => void;
  allTracks?: Track[];
  likedTrackIds?: Set<string>;
  currentTrackId?: string;
  isPlaying?: boolean;
}

export function SearchView({
  isBackendReady = true,
  onPlayTrack,
  onPlayTracks,
  onToggleLike,
  onAddToPlaylist,
  onAddAlbumToPlaylist,
  onPlaylistUpdated,
  onDeletePlaylist,
  allTracks = [],
  likedTrackIds = new Set(),
  currentTrackId,
  isPlaying = false,
}: SearchViewProps) {
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [category, setCategory] = useState<SearchCategory>('all');
  const [results, setResults] = useState<SearchResultsDTO | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  // Search history state: stored in session to maintain clean state between sessions
  const [recentSearches, setRecentSearches] = useState<string[]>(() => {
    try {
      const stored = sessionStorage.getItem('cavia_recent_searches');
      return stored ? JSON.parse(stored) : [];
    } catch {
      return [];
    }
  });

  // Selected playlist for detailed inspection in modal
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);

  // Request sequence tracking to safely ignore stale asynchronous responses
  const lastRequestIdRef = useRef(0);
  const lastExecutedQueryRef = useRef<string | null>(null);
  const lastExecutedCategoryRef = useRef<SearchCategory | null>(null);

  // Save recent searches to sessionStorage
  const saveRecentSearches = (searches: string[]) => {
    setRecentSearches(searches);
    try {
      sessionStorage.setItem('cavia_recent_searches', JSON.stringify(searches));
    } catch {
      // sessionStorage unavailable
    }
  };

  const handleClearHistory = () => {
    saveRecentSearches([]);
  };

  const recordSearchTerm = (term: string) => {
    const trimmed = term.trim();
    if (trimmed.length < 2) return;
    setRecentSearches((prev) => {
      const filtered = prev.filter((item) => item.toLowerCase() !== trimmed.toLowerCase());
      const updated = [trimmed, ...filtered].slice(0, 8);
      try {
        sessionStorage.setItem('cavia_recent_searches', JSON.stringify(updated));
      } catch {
        // ignore
      }
      return updated;
    });
  };

  // 180ms debounce on controlled input
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query);
    }, 180);
    return () => clearTimeout(timer);
  }, [query]);

  // Execute search when debounced query or category changes
  useEffect(() => {
    const trimmed = debouncedQuery.trim();

    if (!trimmed) {
      setResults(null);
      setIsLoading(false);
      setSearchError(null);
      lastExecutedQueryRef.current = null;
      lastExecutedCategoryRef.current = null;
      return;
    }

    // Skip duplicate execution for identical query and category
    if (
      trimmed === lastExecutedQueryRef.current &&
      category === lastExecutedCategoryRef.current &&
      results !== null
    ) {
      return;
    }

    const requestId = ++lastRequestIdRef.current;
    setIsLoading(true);
    setSearchError(null);

    searchLibrary(trimmed, category)
      .then((data) => {
        // Reject stale out-of-order response
        if (requestId === lastRequestIdRef.current) {
          setResults(data);
          setIsLoading(false);
          lastExecutedQueryRef.current = trimmed;
          lastExecutedCategoryRef.current = category;

          // Record meaningful queries in recent search history
          recordSearchTerm(trimmed);
        }
      })
      .catch((err) => {
        if (requestId === lastRequestIdRef.current) {
          const msg = err instanceof Error ? err.message : String(err);
          setSearchError(msg);
          setIsLoading(false);
        }
      });
  }, [debouncedQuery, category]);

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      const trimmed = query.trim();
      if (trimmed) {
        setDebouncedQuery(trimmed);
        recordSearchTerm(trimmed);
      }
    }
  };

  // Helper to play album tracks
  const handlePlayAlbum = (album: Album) => {
    const albumTracks = allTracks.filter(
      (t) =>
        t.album.toLowerCase() === album.title.toLowerCase() &&
        (t.artist.toLowerCase() === album.artist.toLowerCase() ||
          t.albumArtist?.toLowerCase() === album.artist.toLowerCase())
    );
    if (albumTracks.length > 0) {
      onPlayTracks?.(albumTracks, 0, false);
    }
  };

  // Helper to add album tracks to playlist
  const handleAddAlbumToPlaylist = (album: Album) => {
    onAddAlbumToPlaylist?.(album);
  };

  // Helper to play artist tracks
  const handlePlayArtist = (artist: Artist) => {
    const artistTracks = allTracks.filter(
      (t) =>
        t.artist.toLowerCase() === artist.name.toLowerCase() ||
        t.albumArtist?.toLowerCase() === artist.name.toLowerCase()
    );
    if (artistTracks.length > 0) {
      onPlayTracks?.(artistTracks, 0, false);
    }
  };

  const isQueryActive = Boolean(query.trim());

  return (
    <section className="cavia-subpage search-page">
      <div className="subpage-header">
        <div>
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>LOCAL LIBRARY & DISCOVERY ENGINE</span>
          </div>
          <h1 className="subpage-title">
            Find your<br />
            <em>frequency.</em>
          </h1>
          <p className="subpage-subtitle">
            Search your indexed local files and curated collections instantly via SQLite FTS5.
          </p>
        </div>
      </div>

      {/* Main Search Input Bar */}
      <div className="search-bar-wrap">
        <SearchInput
          value={query}
          onChange={(val) => setQuery(val)}
          onKeyDown={handleKeyDown}
          onClear={() => {
            setQuery('');
            setDebouncedQuery('');
            setResults(null);
          }}
          isLoading={isLoading}
          placeholder="Search by track title, artist, album name, or playlist..."
          className="search-main-input"
        />
      </div>

      {/* Category Tabs */}
      <div className="search-category-row">
        <Tabs
          items={[
            { id: 'all', label: 'All Results' },
            { id: 'tracks', label: 'Tracks' },
            { id: 'albums', label: 'Albums' },
            { id: 'artists', label: 'Artists' },
            { id: 'playlists', label: 'Playlists' },
          ]}
          activeId={category}
          onChange={(id) => setCategory(id as SearchCategory)}
          variant="segmented"
        />

        <div className="search-scope-tag">
          <Badge variant="outline" size="sm">
            SCOPE: LOCAL SQLITE FTS5
          </Badge>
        </div>
      </div>

      {/* Error Banner */}
      {searchError && (
        <div className="cavia-toast toast-error" role="alert" style={{ marginBottom: 16 }}>
          <XCircle size={16} className="text-danger" />
          <span>Search failed: {searchError}</span>
        </div>
      )}

      {/* Loading state indicator */}
      {isLoading && !results && (
        <div className="search-loading-state">
          <Loader2 size={28} className="search-loading-spinner" />
          <p>Querying local SQLite FTS5 index...</p>
        </div>
      )}

      {/* Empty Input: Show Recent Search History or Discovery Prompt */}
      {!isQueryActive && (
        <>
          {recentSearches.length > 0 ? (
            <div className="search-history-section">
              <div className="history-header">
                <div className="history-title">
                  <Clock size={14} />
                  <span>RECENT SEARCHES</span>
                </div>
                <button
                  type="button"
                  className="history-clear-btn"
                  onClick={handleClearHistory}
                  aria-label="Clear session search history"
                >
                  <Trash2 size={13} />
                  <span>Clear History</span>
                </button>
              </div>

              <div className="history-chips">
                {recentSearches.map((item) => (
                  <button
                    key={item}
                    type="button"
                    className="history-chip"
                    onClick={() => {
                      setQuery(item);
                      setDebouncedQuery(item);
                    }}
                  >
                    <span>{item}</span>
                  </button>
                ))}
              </div>
            </div>
          ) : (
            <div className="search-discovery-prompt">
              <div className="discovery-kanji" aria-hidden="true">
                尋
              </div>
              <div className="discovery-copy">
                <div className="cavia-eyebrow">
                  <Sparkles size={14} className="text-vermilion" />
                  <span>DIRECT CATALOGUE LOOKUP</span>
                </div>
                <h3>Local Search Engine Ready</h3>
                <p>
                  Type any query above to filter tracks, albums, artists, or playlists indexed in your Cavia collection. Your searches remain 100% private and offline.
                </p>
              </div>
            </div>
          )}

          {/* Provider Standing By Banner */}
          <div className="cavia-banner banner-info" style={{ marginTop: 24 }}>
            <RadioTower size={18} className="banner-icon-accent" />
            <div className="banner-content">
              <strong>Local Library Engine Active</strong>
              <p>
                Search queries are evaluated locally against your SQLite FTS5 trigram index. Online streaming catalogues will activate when authorized provider adapters are configured. No fabricated tracks are shown.
              </p>
            </div>
            <Badge variant="neutral" size="sm">
              OFFLINE / FTS5
            </Badge>
          </div>
        </>
      )}

      {/* Query Active & No Results */}
      {isQueryActive && !isLoading && results && results.totalCount === 0 && (
        <div className="search-results-empty">
          <div className="search-results-icon">
            <Search size={28} />
          </div>
          <h3>No matching recordings found</h3>
          <p>
            No indexed tracks, albums, artists, or playlists match &ldquo;
            <strong>{debouncedQuery || query}</strong>&rdquo; in your local library.
          </p>
          <div className="search-empty-actions">
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setQuery('');
                setDebouncedQuery('');
                setResults(null);
              }}
            >
              Clear Search
            </Button>
          </div>
        </div>
      )}

      {/* Query Active & Results Available */}
      {isQueryActive && results && results.totalCount > 0 && (
        <div className="search-results-container">
          {/* TRACKS SECTION */}
          {(category === 'all' || category === 'tracks') && results.tracks.length > 0 && (
            <section className="search-section" aria-label="Matching Tracks">
              <div className="search-section-header">
                <div className="search-section-title">
                  <span>TRACKS</span>
                  <Badge variant="vermilion" size="sm">
                    {results.tracks.length}
                  </Badge>
                </div>
                {category === 'all' && results.tracks.length >= 20 && (
                  <button
                    type="button"
                    className="search-see-all-btn"
                    onClick={() => setCategory('tracks')}
                  >
                    View all track results →
                  </button>
                )}
              </div>

              <div className="tracks-table">
                <div className="tracks-table-head">
                  <span className="col-num">#</span>
                  <span className="col-title">TITLE</span>
                  <span className="col-artist">ARTIST</span>
                  <span className="col-album">ALBUM</span>
                  <span className="col-duration">TIME</span>
                  <span className="col-action">ACTION</span>
                </div>

                <div className="tracks-table-body">
                  {results.tracks.map((track, i) => {
                    const isCurrent = track.id === currentTrackId;
                    const isLiked = track.isLiked || likedTrackIds.has(track.id);
                    const formattedArt = formatArtworkUri(track.artworkUri);

                    return (
                      <div
                        key={track.id}
                        className={`track-row ${isCurrent ? 'current' : ''}`}
                        tabIndex={0}
                        role="row"
                        onClick={() => onPlayTrack?.(track.id)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault();
                            onPlayTrack?.(track.id);
                          }
                        }}
                      >
                        <span className="col-num">
                          {isCurrent && isPlaying ? (
                            <span className="track-playing-indicator" title="Playing">
                              ▶
                            </span>
                          ) : (
                            track.trackNumber || i + 1
                          )}
                        </span>

                        <span className="col-title">
                          {formattedArt ? (
                            <img
                              src={formattedArt}
                              alt=""
                              className="track-row-art"
                              loading="lazy"
                            />
                          ) : (
                            <div className="track-art-placeholder">
                              <Disc3 size={14} />
                            </div>
                          )}
                          <span className="track-title-text">{track.title}</span>
                        </span>

                        <span className="col-artist">{track.artist || 'Unknown Artist'}</span>
                        <span className="col-album">{track.album || 'Unknown Album'}</span>
                        <span className="col-duration">{formatDuration(track.durationMs)}</span>

                        <span className="col-action" onClick={(e) => e.stopPropagation()}>
                          <button
                            type="button"
                            className="track-action-inline-btn"
                            onClick={() => onAddToPlaylist?.(track)}
                            aria-label={`Add ${track.title} to playlist`}
                            title="Add to playlist"
                          >
                            <ListPlus size={15} />
                          </button>

                          <button
                            type="button"
                            className={`track-like-inline-btn ${isLiked ? 'active' : ''}`}
                            onClick={() => onToggleLike?.(track.id)}
                            aria-label={isLiked ? 'Remove from liked songs' : 'Add to liked songs'}
                            title={isLiked ? 'Liked' : 'Like track'}
                          >
                            <Heart
                              size={15}
                              fill={isLiked ? 'var(--accent-vermilion)' : 'none'}
                              color={isLiked ? 'var(--accent-vermilion)' : 'currentColor'}
                            />
                          </button>
                        </span>
                      </div>
                    );
                  })}
                </div>
              </div>
            </section>
          )}

          {/* ALBUMS SECTION */}
          {(category === 'all' || category === 'albums') && results.albums.length > 0 && (
            <section className="search-section" aria-label="Matching Albums">
              <div className="search-section-header">
                <div className="search-section-title">
                  <span>ALBUMS</span>
                  <Badge variant="vermilion" size="sm">
                    {results.albums.length}
                  </Badge>
                </div>
                {category === 'all' && results.albums.length >= 10 && (
                  <button
                    type="button"
                    className="search-see-all-btn"
                    onClick={() => setCategory('albums')}
                  >
                    View all album results →
                  </button>
                )}
              </div>

              <div className="albums-grid">
                {results.albums.map((album) => {
                  const formattedArt = formatArtworkUri(album.artworkUri);

                  return (
                    <article
                      key={album.id}
                      className="album-card"
                      tabIndex={0}
                      role="button"
                      onClick={() => handlePlayAlbum(album)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault();
                          handlePlayAlbum(album);
                        }
                      }}
                    >
                      <div className="album-sleeve-frame">
                        {formattedArt ? (
                          <img
                            src={formattedArt}
                            alt={album.title}
                            className="album-artwork-img"
                            loading="lazy"
                          />
                        ) : (
                          <div className="album-artwork-placeholder">
                            <Disc3 size={32} />
                            <span className="album-badge">ALBUM</span>
                          </div>
                        )}
                        <div className="album-shadow-sheen" />
                      </div>

                      <div className="album-meta">
                        <strong className="album-title">{album.title}</strong>
                        <span className="album-artist">{album.artist}</span>
                        <div className="album-submeta">
                          {album.year && <span>{album.year}</span>}
                          {album.year && <span>·</span>}
                          <span>
                            {album.trackCount} {album.trackCount === 1 ? 'track' : 'tracks'}
                          </span>
                          <button
                            type="button"
                            className="album-add-playlist-inline-btn"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleAddAlbumToPlaylist(album);
                            }}
                            title={`Add all tracks from ${album.title} to playlist`}
                            aria-label={`Add all tracks from ${album.title} to playlist`}
                          >
                            <ListPlus size={13} />
                            <span>Playlist</span>
                          </button>
                        </div>
                      </div>
                    </article>
                  );
                })}
              </div>
            </section>
          )}

          {/* ARTISTS SECTION */}
          {(category === 'all' || category === 'artists') && results.artists.length > 0 && (
            <section className="search-section" aria-label="Matching Artists">
              <div className="search-section-header">
                <div className="search-section-title">
                  <span>ARTISTS</span>
                  <Badge variant="vermilion" size="sm">
                    {results.artists.length}
                  </Badge>
                </div>
                {category === 'all' && results.artists.length >= 10 && (
                  <button
                    type="button"
                    className="search-see-all-btn"
                    onClick={() => setCategory('artists')}
                  >
                    View all artist results →
                  </button>
                )}
              </div>

              <div className="artists-grid">
                {results.artists.map((artist) => {
                  const formattedArt = formatArtworkUri(artist.artworkUri);

                  return (
                    <article
                      key={artist.id}
                      className="artist-card"
                      tabIndex={0}
                      role="button"
                      onClick={() => handlePlayArtist(artist)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault();
                          handlePlayArtist(artist);
                        }
                      }}
                    >
                      <div className="artist-avatar-frame">
                        {formattedArt ? (
                          <img
                            src={formattedArt}
                            alt={artist.name}
                            className="artist-artwork-img"
                            loading="lazy"
                            style={{
                              width: '100%',
                              height: '100%',
                              objectFit: 'cover',
                              borderRadius: '50%',
                            }}
                          />
                        ) : (
                          <span className="artist-monogram">
                            {artist.name.slice(0, 2).toUpperCase()}
                          </span>
                        )}
                        <div className="artist-avatar-halo" />
                      </div>

                      <div className="artist-meta">
                        <strong className="artist-name">{artist.name}</strong>
                        <span className="artist-stats">
                          {artist.albumCount} {artist.albumCount === 1 ? 'album' : 'albums'} ·{' '}
                          {artist.trackCount} {artist.trackCount === 1 ? 'track' : 'tracks'}
                        </span>
                      </div>
                    </article>
                  );
                })}
              </div>
            </section>
          )}

          {/* PLAYLISTS SECTION */}
          {(category === 'all' || category === 'playlists') && results.playlists.length > 0 && (
            <section className="search-section" aria-label="Matching Playlists">
              <div className="search-section-header">
                <div className="search-section-title">
                  <span>PLAYLISTS</span>
                  <Badge variant="vermilion" size="sm">
                    {results.playlists.length}
                  </Badge>
                </div>
                {category === 'all' && results.playlists.length >= 10 && (
                  <button
                    type="button"
                    className="search-see-all-btn"
                    onClick={() => setCategory('playlists')}
                  >
                    View all playlist results →
                  </button>
                )}
              </div>

              <div className="playlists-grid">
                {results.playlists.map((playlist) => (
                  <article
                    key={playlist.id}
                    className="playlist-card"
                    tabIndex={0}
                    role="button"
                    onClick={() => setSelectedPlaylist(playlist)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        setSelectedPlaylist(playlist);
                      }
                    }}
                    aria-label={`Open playlist ${playlist.name}`}
                  >
                    <div className="playlist-card-art">
                      <Disc3 size={36} className="playlist-vinyl-emblem" />
                      <span className="playlist-art-sheen" />
                    </div>

                    <div className="playlist-card-body">
                      <div className="playlist-badge-row">
                        <Badge variant="vermilion" size="sm">
                          {playlist.sourceProvider
                            ? playlist.sourceProvider.toUpperCase()
                            : 'LOCAL PLAYLIST'}
                        </Badge>
                        <span className="playlist-track-badge">
                          {playlist.trackCount}{' '}
                          {playlist.trackCount === 1 ? 'track' : 'tracks'}
                        </span>
                      </div>

                      <h3 className="playlist-title">{playlist.name}</h3>
                      <p className="playlist-description">
                        {playlist.description || 'Custom curated local playlist archive'}
                      </p>
                    </div>
                  </article>
                ))}
              </div>
            </section>
          )}
        </div>
      )}

      {/* Playlist Detail Modal for Selected Playlist in Search Results */}
      {selectedPlaylist && (
        <PlaylistDetailModal
          playlist={selectedPlaylist}
          isOpen={Boolean(selectedPlaylist)}
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
