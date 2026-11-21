-- Forward-only: catalogues entries (YouTube Music streams) are not rows in
-- `tracks`, so the lyrics cache cannot keep a foreign key to that table for
-- them. Rebuild the cache without the constraint; existing rows are copied
-- verbatim and the lookup still resolves the track before using it.
CREATE TABLE lyrics_cache_rebuilt (
    track_id TEXT PRIMARY KEY NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('embedded', 'lrclib', 'none')),
    status TEXT NOT NULL CHECK (status IN ('found', 'instrumental', 'not_found')),
    plain_lyrics TEXT,
    raw_synced_lyrics TEXT,
    is_instrumental INTEGER NOT NULL DEFAULT 0 CHECK (is_instrumental IN (0, 1)),
    lrclib_id INTEGER,
    track_signature TEXT NOT NULL,
    fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO lyrics_cache_rebuilt (
    track_id, source, status, plain_lyrics, raw_synced_lyrics,
    is_instrumental, lrclib_id, track_signature, fetched_at, updated_at
)
SELECT
    track_id, source, status, plain_lyrics, raw_synced_lyrics,
    is_instrumental, lrclib_id, track_signature, fetched_at, updated_at
FROM lyrics_cache;

DROP TABLE lyrics_cache;
ALTER TABLE lyrics_cache_rebuilt RENAME TO lyrics_cache;

CREATE INDEX IF NOT EXISTS idx_lyrics_cache_track_id ON lyrics_cache(track_id);
CREATE INDEX IF NOT EXISTS idx_lyrics_cache_signature ON lyrics_cache(track_signature);

-- The constraint used to do this implicitly; keep library deletions clearing
-- their cached lyrics now that catalogue rows may reference no track at all.
CREATE TRIGGER IF NOT EXISTS lyrics_cache_track_delete
AFTER DELETE ON tracks
BEGIN
    DELETE FROM lyrics_cache WHERE track_id = OLD.id;
END;