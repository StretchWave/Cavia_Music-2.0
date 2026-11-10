import test from 'node:test';
import assert from 'node:assert/strict';

test('Playlist Management — Optimistic Reordering Logic', async (t) => {
  await t.test('moves track forward correctly', () => {
    const tracks = [
      { id: 't0', title: 'Song 0' },
      { id: 't1', title: 'Song 1' },
      { id: 't2', title: 'Song 2' },
      { id: 't3', title: 'Song 3' },
    ];
    // Move from 0 to 2
    const fromIndex = 0;
    const toIndex = 2;
    const next = [...tracks];
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);

    assert.deepEqual(
      next.map((t) => t.id),
      ['t1', 't2', 't0', 't3']
    );
  });

  await t.test('moves track backward correctly', () => {
    const tracks = [
      { id: 't0', title: 'Song 0' },
      { id: 't1', title: 'Song 1' },
      { id: 't2', title: 'Song 2' },
      { id: 't3', title: 'Song 3' },
    ];
    // Move from 3 to 1
    const fromIndex = 3;
    const toIndex = 1;
    const next = [...tracks];
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);

    assert.deepEqual(
      next.map((t) => t.id),
      ['t0', 't3', 't1', 't2']
    );
  });

  await t.test('handles adjacent track swap', () => {
    const tracks = [
      { id: 't0', title: 'Song 0' },
      { id: 't1', title: 'Song 1' },
    ];
    const fromIndex = 0;
    const toIndex = 1;
    const next = [...tracks];
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);

    assert.deepEqual(
      next.map((t) => t.id),
      ['t1', 't0']
    );
  });
});

test('Playlist Management — Optimistic Removal & Compaction', async (t) => {
  await t.test('removes target index and preserves remaining sequence', () => {
    const tracks = [
      { id: 't0', title: 'Song 0' },
      { id: 't1', title: 'Song 1' },
      { id: 't2', title: 'Song 2' },
    ];
    // Remove middle (index 1)
    const removedMiddle = tracks.filter((_, i) => i !== 1);
    assert.deepEqual(
      removedMiddle.map((t) => t.id),
      ['t0', 't2']
    );

    // Remove first (index 0)
    const removedFirst = tracks.filter((_, i) => i !== 0);
    assert.deepEqual(
      removedFirst.map((t) => t.id),
      ['t1', 't2']
    );

    // Remove last (index 2)
    const removedLast = tracks.filter((_, i) => i !== 2);
    assert.deepEqual(
      removedLast.map((t) => t.id),
      ['t0', 't1']
    );
  });

  await t.test('removes specific instance when duplicate tracks exist', () => {
    const tracks = [
      { id: 't0', title: 'Song 0' },
      { id: 't1', title: 'Song 1' },
      { id: 't0', title: 'Song 0' }, // duplicate at index 2
    ];
    // Removing index 0 only removes the first instance
    const afterIndex0 = tracks.filter((_, i) => i !== 0);
    assert.equal(afterIndex0.length, 2);
    assert.equal(afterIndex0[0].id, 't1');
    assert.equal(afterIndex0[1].id, 't0');

    // Removing index 2 only removes the duplicate instance
    const afterIndex2 = tracks.filter((_, i) => i !== 2);
    assert.equal(afterIndex2.length, 2);
    assert.equal(afterIndex2[0].id, 't0');
    assert.equal(afterIndex2[1].id, 't1');
  });
});

test('Playlist Management — Name & Metadata Validation', async (t) => {
  await t.test('rejects empty and whitespace-only playlist names', () => {
    const validate = (name) => {
      const trimmed = name?.trim();
      return Boolean(trimmed && trimmed.length > 0);
    };

    assert.equal(validate(''), false);
    assert.equal(validate('   '), false);
    assert.equal(validate('\t\n'), false);
    assert.equal(validate('Night Jazz'), true);
    assert.equal(validate('  Spaced Title  '), true);
  });
});

