import { useEffect, useRef, useState } from 'react';
import { searchLibrary } from '../../lib/library';
import { cancelYouTubeMusicSearch, searchYouTubeMusic, type YtMusicTrack } from '../../lib/ytmusic';
import type { SearchCategory, SearchResultsDTO } from '../../lib/types';
import { shouldSearchYouTubeMusic } from './searchPolicy.js';

export function useSearchCatalog(
  query: string,
  category: SearchCategory,
  onlineAvailable: boolean,
) {
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [results, setResults] = useState<SearchResultsDTO | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [onlineResults, setOnlineResults] = useState<YtMusicTrack[]>([]);
  const [onlineLoading, setOnlineLoading] = useState(false);
  const [onlineError, setOnlineError] = useState<string | null>(null);
  const [onlineRetryToken, setOnlineRetryToken] = useState(0);
  const requestIdRef = useRef(0);
  const onlineRequestIdRef = useRef(0);
  const onlineSearchSequenceRef = useRef(0);

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
      .catch((error: unknown) => {
        if (!active || requestId !== requestIdRef.current) return;
        setSearchError(error instanceof Error ? error.message : String(error));
        setIsLoading(false);
      });

    return () => { active = false; };
  }, [debouncedQuery, category]);

  useEffect(() => {
    const trimmed = debouncedQuery.trim();
    const needsOnlineTracks = shouldSearchYouTubeMusic(category);
    if (!trimmed || !onlineAvailable || !needsOnlineTracks) {
      onlineRequestIdRef.current += 1;
      setOnlineResults([]);
      setOnlineLoading(false);
      setOnlineError(null);
      return;
    }

    const requestId = globalThis.crypto?.randomUUID?.()
      ?? `${Date.now()}-${++onlineSearchSequenceRef.current}`;
    const requestSequence = ++onlineRequestIdRef.current;
    let settled = false;
    setOnlineLoading(true);
    setOnlineError(null);
    searchYouTubeMusic(trimmed, requestId)
      .then((items) => {
        settled = true;
        if (requestSequence !== onlineRequestIdRef.current) return;
        setOnlineResults(items);
        setOnlineLoading(false);
      })
      .catch((error: unknown) => {
        settled = true;
        if (requestSequence !== onlineRequestIdRef.current) return;
        setOnlineResults([]);
        setOnlineError(error instanceof Error ? error.message : String(error));
        setOnlineLoading(false);
      });

    return () => {
      if (requestSequence === onlineRequestIdRef.current) onlineRequestIdRef.current += 1;
      if (!settled) {
        void cancelYouTubeMusicSearch(requestId).catch(() => undefined);
      }
    };
  }, [debouncedQuery, category, onlineRetryToken, onlineAvailable]);

  return {
    debouncedQuery,
    setDebouncedQuery,
    results,
    setResults,
    isLoading,
    searchError,
    onlineResults,
    onlineLoading,
    onlineError,
    setOnlineError,
    retryOnlineSearch: () => setOnlineRetryToken((value) => value + 1),
  };
}
