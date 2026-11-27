import { useCallback, useEffect, useRef, useState } from 'react';
import {
  getAlbums,
  getArtists,
  getLikedTracks,
  getPlaylists,
  getTracks,
} from '../lib/library';
import { isNativeApp } from '../lib/platform';
import type { Album, Artist, Playlist, Track } from '../lib/types';

/**
 * Owns the Rust-backed library snapshot and its refresh lifecycle. The app
 * shell can coordinate playback without threading five collection setters
 * through unrelated screen handlers.
 */
export function useLibraryController(scheduleAutomaticSync: () => void) {
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [albums, setAlbums] = useState<Album[]>([]);
  const [artists, setArtists] = useState<Artist[]>([]);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [likedTracks, setLikedTracks] = useState<Track[]>([]);
  const tracksRef = useRef<Track[]>(tracks);
  const likedTracksRef = useRef<Track[]>(likedTracks);

  useEffect(() => {
    tracksRef.current = tracks;
  }, [tracks]);

  useEffect(() => {
    likedTracksRef.current = likedTracks;
  }, [likedTracks]);

  const refreshLibrary = useCallback(async () => {
    if (!isNativeApp()) return;
    try {
      const [nextTracks, nextAlbums, nextArtists, nextPlaylists, nextLiked] =
        await Promise.all([
          getTracks(),
          getAlbums(),
          getArtists(),
          getPlaylists(),
          getLikedTracks(),
        ]);
      setTracks(nextTracks);
      setAlbums(nextAlbums);
      setArtists(nextArtists);
      setPlaylists(nextPlaylists);
      setLikedTracks(nextLiked);
      setLibraryError(null);
      scheduleAutomaticSync();
    } catch (err) {
      console.error('Error refreshing library from SQLite:', err);
      setLibraryError(err instanceof Error ? err.message : String(err));
    }
  }, [scheduleAutomaticSync]);

  return {
    libraryError,
    setLibraryError,
    tracks,
    setTracks,
    albums,
    setAlbums,
    artists,
    setArtists,
    playlists,
    setPlaylists,
    likedTracks,
    setLikedTracks,
    tracksRef,
    likedTracksRef,
    refreshLibrary,
  };
}
