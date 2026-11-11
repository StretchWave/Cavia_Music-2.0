-- Forward-only migration: lyrics_cache table for M4.3
-- Supports caching of embedded and LRCLIB lyrics, plain and synchronized (LRC) formats,
-- and negative results (instrumental, not found) tied to a track metadata signature.

CREATE TABLE IF NOT EXISTS lyrics_cache (
    track_id TEXT PRIMARY KEY NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
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

CREATE INDEX IF NOT EXISTS idx_lyrics_cache_track_id ON lyrics_cache(track_id);
CREATE INDEX IF NOT EXISTS idx_lyrics_cache_signature ON lyrics_cache(track_signature);

