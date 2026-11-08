import { useState, type KeyboardEvent } from 'react';
import {
  Clock,
  RadioTower,
  Search,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { SearchInput } from '../../components/SearchInput';
import { Tabs } from '../../components/Tabs';
import { Badge } from '../../components/Badge';
import { Button } from '../../components/Button';

type SearchCategory = 'all' | 'tracks' | 'albums' | 'artists' | 'playlists';

export function SearchView() {
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState<SearchCategory>('all');
  // Starts strictly empty to avoid fabricated search data
  const [recentSearches, setRecentSearches] = useState<string[]>([]);

  const handleClearHistory = () => {
    setRecentSearches([]);
  };

  const handleSearchSubmit = (searchTerm: string) => {
    const trimmed = searchTerm.trim();
    if (!trimmed) return;
    setRecentSearches((prev) => [trimmed, ...prev.filter((item) => item !== trimmed)].slice(0, 8));
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      handleSearchSubmit(query);
    }
  };

  return (
    <section className="cavia-subpage search-page">
      <div className="subpage-header">
        <div>
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>UNIVERSAL CATALOGUE & LOCAL DISCOVERY</span>
          </div>
          <h1 className="subpage-title">
            Find your<br />
            <em>frequency.</em>
          </h1>
          <p className="subpage-subtitle">
            Search your indexed local files and authorized provider catalogues from a unified interface.
          </p>
        </div>
      </div>

      {/* Main Search Input Bar */}
      <div className="search-bar-wrap">
        <SearchInput
          value={query}
          onChange={(val) => setQuery(val)}
          onKeyDown={handleKeyDown}
          onClear={() => setQuery('')}
          placeholder="Search by track title, artist, album name, or genre (press Enter)..."
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
          onChange={setCategory}
          variant="segmented"
        />

        <div className="search-scope-tag">
          <Badge variant="outline" size="sm">
            SCOPE: LOCAL SQLITE + ADAPTERS
          </Badge>
        </div>
      </div>

      {/* Provider Status Honest Banner */}
      <div className="cavia-banner banner-info">
        <RadioTower size={18} className="banner-icon-accent" />
        <div className="banner-content">
          <strong>Search Providers Standing By (Milestone 4)</strong>
          <p>
            Local library search will query your SQLite FTS5 index. Streaming catalogue search will activate when authorized provider adapters (e.g. Spotify PKCE metadata) are connected. No fabricated results are simulated.
          </p>
        </div>
        <Badge variant="neutral" size="sm">
          UNCONNECTED
        </Badge>
      </div>

      {/* Recent Search History if active query is empty */}
      {!query && recentSearches.length > 0 && (
        <div className="search-history-section">
          <div className="history-header">
            <div className="history-title">
              <Clock size={14} />
              <span>SESSION RECENT QUERIES</span>
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
                onClick={() => setQuery(item)}
              >
                <span>{item}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Query Search Results or Empty State */}
      {query ? (
        <div className="search-results-empty">
          <div className="search-results-icon">
            <Search size={28} />
          </div>
          <h3>No matching recordings found</h3>
          <p>
            No indexed tracks match &ldquo;<strong>{query}</strong>&rdquo; in your local library. Once local audio files are imported in Milestone 2, real-time search will index title, artist, and album tags.
          </p>
          <div className="search-empty-actions">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setQuery('')}
            >
              Clear Search
            </Button>
          </div>
        </div>
      ) : (
        <div className="search-discovery-prompt">
          <div className="discovery-kanji" aria-hidden="true">尋</div>
          <div className="discovery-copy">
            <div className="cavia-eyebrow">
              <Sparkles size={14} className="text-vermilion" />
              <span>DIRECT CATALOGUE LOOKUP</span>
            </div>
            <h3>Local & Provider Search Ready</h3>
            <p>
              Type any query above to filter tracks, albums, or artists indexed in your Cavia collection. Your searches remain 100% private and offline.
            </p>
          </div>
        </div>
      )}
    </section>
  );
}
