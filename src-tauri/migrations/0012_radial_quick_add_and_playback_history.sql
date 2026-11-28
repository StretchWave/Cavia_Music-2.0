-- Forward-only migration: Radial Quick Add playlist metadata & intentional playback history log.
-- Preserves all existing playlists and recent_plays rows intact.

ALTER TABLE playlists ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1));
ALTER TABLE playlists ADD COLUMN last_used_at TEXT;

CREATE INDEX IF NOT EXISTS idx_playlists_quick_add
    ON playlists(is_deleted, is_pinned DESC, last_used_at DESC, updated_at DESC);

ALTER TABLE recent_plays ADD COLUMN last_origin TEXT;

CREATE TABLE IF NOT EXISTS recent_play_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 0,
    playback_origin TEXT NOT NULL,
    played_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_recent_play_events_played_at
    ON recent_play_events(played_at DESC, id DESC);
