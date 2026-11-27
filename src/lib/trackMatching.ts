import type { Album, Artist, Track } from './types';
export { detectPlaylistImportSource } from './diagnostics.js';

export function getAlbumTracks(tracks: Track[], album: Album): Track[] {
  const targetTitle = album.title.toLowerCase();
  const targetArtist = album.artist.toLowerCase();
  return tracks.filter(
    (track) =>
      track.album.toLowerCase() === targetTitle &&
      (track.albumArtist?.toLowerCase() === targetArtist ||
        track.artist.toLowerCase() === targetArtist)
  );
}

export function getArtistTracks(tracks: Track[], artist: Artist): Track[] {
  const targetName = artist.name.toLowerCase();
  return tracks.filter(
    (track) =>
      track.artist.toLowerCase() === targetName ||
      track.albumArtist?.toLowerCase() === targetName
  );
}

export function ytMusicTrackToTrack(item: {
  videoId: string;
  title: string;
  artist: string;
  album: string;
  durationMs: number;
  artworkUrl?: string | null;
}): Track {
  const artistName = item.artist.trim() || 'Unknown Artist';
  return {
    id: `ytmusic:${item.videoId}`,
    title: item.title,
    artist: artistName,
    album: item.album,
    albumArtist: artistName,
    durationMs: Math.max(0, item.durationMs),
    artworkUri: item.artworkUrl ?? undefined,
    sourceKind: 'provider',
    providerId: 'youtube_music',
    isLiked: false,
    playCount: 0,
    dateAdded: '',
  };
}

export function shuffleArray<T>(items: T[]): T[] {
  const copy = [...items];
  for (let index = copy.length - 1; index > 0; index -= 1) {
    const swap = Math.floor(Math.random() * (index + 1));
    [copy[index], copy[swap]] = [copy[swap], copy[index]];
  }
  return copy;
}
