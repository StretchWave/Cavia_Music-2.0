import test from 'node:test';
import assert from 'node:assert/strict';
import { detectPlaylistImportSource } from '../src/lib/diagnostics.js';

test('detects Spotify playlist links and URIs', () => {
  assert.equal(
    detectPlaylistImportSource('https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=123'),
    'spotify'
  );
  assert.equal(
    detectPlaylistImportSource('spotify:playlist:37i9dQZF1DXcBWIGoYBM5M'),
    'spotify'
  );
  assert.equal(detectPlaylistImportSource('37i9dQZF1DXcBWIGoYBM5M'), 'spotify');
});

test('detects YouTube Music and YouTube playlist links cleanly', () => {
  assert.equal(
    detectPlaylistImportSource('https://music.youtube.com/playlist?list=PL1234567890ABCDEF'),
    'youtube_music'
  );
  assert.equal(
    detectPlaylistImportSource('https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=OLAK5uy_k1234567890'),
    'youtube_music'
  );
  assert.equal(
    detectPlaylistImportSource('https://music.youtube.com/browse/VLPL9876543210FEDCBA'),
    'youtube_music'
  );
  assert.equal(detectPlaylistImportSource('PL1234567890ABCDEF'), 'youtube_music');
});

test('rejects unrecognized or empty inputs', () => {
  assert.equal(detectPlaylistImportSource(''), null);
  assert.equal(detectPlaylistImportSource('https://example.com/playlist/123'), null);
  assert.equal(detectPlaylistImportSource('not-a-playlist-url'), null);
});
