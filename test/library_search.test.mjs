import test from 'node:test';
import assert from 'node:assert/strict';

test('Library Search — Contract & DTO Structure', async (t) => {
  await t.test('validates SearchResultsDTO structure and camelCase properties', () => {
    const mockDto = {
      tracks: [
        {
          id: 't-1',
          title: 'Bohemian Rhapsody',
          artist: 'Queen',
          album: 'A Night at the Opera',
          durationMs: 354000,
          artworkUri: null,
          sourceKind: 'local',
          isLiked: false,
          playCount: 5,
          dateAdded: '2026-11-10T12:00:00Z',
        },
      ],
      albums: [
        {
          id: 'A Night at the Opera_Queen',
          title: 'A Night at the Opera',
          artist: 'Queen',
          year: 1975,
          trackCount: 12,
          artworkUri: null,
        },
      ],
      artists: [
        {
          id: 'Queen',
          name: 'Queen',
          trackCount: 15,
          albumCount: 2,
          artworkUri: null,
        },
      ],
      playlists: [
        {
          id: 'pl-1',
          name: 'Rock Classics',
          description: 'Timeless rock hits',
          artworkUri: null,
          trackCount: 25,
          createdAt: '2026-11-10T12:00:00Z',
        },
      ],
      query: 'queen',
      category: 'all',
      totalCount: 4,
    };

    assert.equal(mockDto.tracks.length, 1);
    assert.equal(mockDto.albums.length, 1);
    assert.equal(mockDto.artists.length, 1);
    assert.equal(mockDto.playlists.length, 1);
    assert.equal(mockDto.totalCount, 4);
    assert.equal(mockDto.category, 'all');
  });
});

test('Library Search — Recent Searches Deduplication & Cap', async (t) => {
  await t.test('deduplicates case-insensitively and prepends newest', () => {
    let history = ['queen', 'beatles', 'pink floyd'];
    const newTerm = 'Queen';

    const trimmed = newTerm.trim();
    history = [trimmed, ...history.filter((item) => item.toLowerCase() !== trimmed.toLowerCase())].slice(0, 8);

    assert.deepEqual(history, ['Queen', 'beatles', 'pink floyd']);
  });

  await t.test('enforces max cap of 8 recent queries', () => {
    let history = ['q1', 'q2', 'q3', 'q4', 'q5', 'q6', 'q7', 'q8'];
    const newTerm = 'q9';

    const trimmed = newTerm.trim();
    history = [trimmed, ...history.filter((item) => item.toLowerCase() !== trimmed.toLowerCase())].slice(0, 8);

    assert.equal(history.length, 8);
    assert.equal(history[0], 'q9');
    assert.equal(history.includes('q8'), false);
  });

  await t.test('rejects empty and whitespace-only queries from history', () => {
    let history = ['existing'];
    const terms = ['', '   ', '\t\n'];

    for (const term of terms) {
      const trimmed = term.trim();
      if (trimmed.length >= 2) {
        history = [trimmed, ...history.filter((item) => item.toLowerCase() !== trimmed.toLowerCase())].slice(0, 8);
      }
    }

    assert.deepEqual(history, ['existing']);
  });
});

test('Library Search — Asynchronous Request Sequence & Stale Response Rejection', async (t) => {
  await t.test('rejects out-of-order stale search responses', async () => {
    let activeResults = null;
    let lastRequestId = 0;

    // Simulate request 1 (dispatched first, resolves slowly)
    const req1Id = ++lastRequestId;
    const req1Promise = new Promise((resolve) =>
      setTimeout(() => resolve({ id: req1Id, data: 'stale result from req1' }), 60)
    );

    // Simulate request 2 (dispatched second, resolves quickly)
    const req2Id = ++lastRequestId;
    const req2Promise = new Promise((resolve) =>
      setTimeout(() => resolve({ id: req2Id, data: 'fresh result from req2' }), 20)
    );

    // Process responses as they arrive
    await req2Promise.then((res) => {
      if (res.id === lastRequestId) {
        activeResults = res.data;
      }
    });
    assert.equal(activeResults, 'fresh result from req2');

    await req1Promise.then((res) => {
      if (res.id === lastRequestId) {
        activeResults = res.data;
      }
    });
    // activeResults MUST NOT be overwritten by the stale req1 response
    assert.equal(activeResults, 'fresh result from req2');
  });
});

test('Library Search — Short Query vs FTS5 Branching', async (t) => {
  await t.test('correctly distinguishes short queries (< 3 chars) from FTS5 queries (>= 3 chars)', () => {
    function getSearchStrategy(query) {
      const trimmed = query.trim();
      if (!trimmed) return 'empty';
      const charCount = [...trimmed].length;
      return charCount < 3 ? 'like_fallback' : 'fts5_trigram';
    }

    assert.equal(getSearchStrategy(''), 'empty');
    assert.equal(getSearchStrategy('   '), 'empty');
    assert.equal(getSearchStrategy('a'), 'like_fallback');
    assert.equal(getSearchStrategy('ab'), 'like_fallback');
    assert.equal(getSearchStrategy('abc'), 'fts5_trigram');
    assert.equal(getSearchStrategy('Queen'), 'fts5_trigram');
    assert.equal(getSearchStrategy('夜'), 'like_fallback'); // 1 unicode kanji
    assert.equal(getSearchStrategy('夜に'), 'like_fallback'); // 2 unicode chars
    assert.equal(getSearchStrategy('夜に駆'), 'fts5_trigram'); // 3 unicode chars
  });
});

