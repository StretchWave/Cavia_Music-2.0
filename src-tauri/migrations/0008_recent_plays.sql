-- Playback history for the Search page. Only tracks that actually started
-- playing are stored; search queries themselves are never recorded.
CREATE TABLE IF NOT EXISTS recent_plays (
    track_id TEXT PRIMARY KEY NOT NULL,
    source_kind TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 0,
    artwork_uri TEXT,
    provider_id TEXT,
    play_count INTEGER NOT NULL DEFAULT 1,
    played_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_recent_plays_played_at ON recent_plays(played_at DESC);