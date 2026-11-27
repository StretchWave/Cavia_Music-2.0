import test from 'node:test';
import assert from 'node:assert/strict';
import { shouldSearchYouTubeMusic } from '../src/features/search/searchPolicy.js';

test('YouTube Music search follows track-capable library categories', () => {
  assert.equal(shouldSearchYouTubeMusic('all'), true);
  assert.equal(shouldSearchYouTubeMusic('tracks'), true);
  assert.equal(shouldSearchYouTubeMusic('albums'), false);
  assert.equal(shouldSearchYouTubeMusic('artists'), false);
  assert.equal(shouldSearchYouTubeMusic('playlists'), false);
  assert.equal(shouldSearchYouTubeMusic('liked'), false);
});
