-- Cavia Music 2.0: Milestone 4.6 Optional Account & Cloud Sync Migration
-- Forward-only migration: outbox change journal, sync state, tombstones, track mappings,
-- and support for unplayable/remote-only tracks in playlist_tracks.

PRAGMA foreign_keys = OFF;

-- 1. Sync State
CREATE TABLE IF NOT EXISTS sync_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    account_id TEXT,
    account_email TEXT,
    device_id TEXT NOT NULL,
    sync_enabled INTEGER NOT NULL DEFAULT 0 CHECK (sync_enabled IN (0, 1)),
    last_synced_at TEXT,
    remote_cursor TEXT,
    last_error TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO sync_state (id, device_id, sync_enabled)
VALUES (1, 'dev-' || lower(hex(randomblob(8))), 0);

-- 2. Durable Local Outbox / Mutation Journal
CREATE TABLE IF NOT EXISTS sync_outbox (
    operation_id TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('playlist', 'playlist_track', 'liked_track')),
    entity_id TEXT NOT NULL,
    operation_type TEXT NOT NULL CHECK (operation_type IN ('create', 'update', 'delete', 'add', 'remove', 'reorder', 'like', 'unlike')),
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_sync_outbox_pending ON sync_outbox(status, created_at ASC);

-- 3. Tombstones Table to prevent resurrecting deleted content across offline devices
CREATE TABLE IF NOT EXISTS sync_tombstones (
    entity_type TEXT NOT NULL CHECK (entity_type IN ('playlist', 'playlist_track', 'liked_track')),
    entity_id TEXT NOT NULL,
    deleted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (entity_type, entity_id)
);

-- 4. Cloud Track Identity Mappings
CREATE TABLE IF NOT EXISTS cloud_track_mappings (
    cloud_track_id TEXT PRIMARY KEY NOT NULL,
    local_track_id TEXT REFERENCES tracks(id) ON DELETE SET NULL,
    content_hash TEXT,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    album TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_cloud_track_hash ON cloud_track_mappings(content_hash);

-- 5. Upgrade playlist_tracks to support remote-only tracks with metadata snapshots
CREATE TABLE IF NOT EXISTS playlist_tracks_new (
    id TEXT PRIMARY KEY NOT NULL,
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id TEXT REFERENCES tracks(id) ON DELETE SET NULL,
    cloud_track_id TEXT,
    title TEXT,
    artist TEXT,
    album TEXT,
    duration_ms INTEGER DEFAULT 0,
    content_hash TEXT,
    position INTEGER NOT NULL CHECK (position >= 0),
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(playlist_id, position)
);

INSERT INTO playlist_tracks_new (id, playlist_id, track_id, title, artist, album, duration_ms, content_hash, position, added_at)
SELECT
    pt.id,
    pt.playlist_id,
    pt.track_id,
    t.title,
    t.artist,
    t.album,
    t.duration_ms,
    t.content_hash,
    pt.position,
    pt.added_at
FROM playlist_tracks pt
LEFT JOIN tracks t ON pt.track_id = t.id;

DROP TABLE playlist_tracks;

ALTER TABLE playlist_tracks_new RENAME TO playlist_tracks;

CREATE INDEX IF NOT EXISTS idx_playlist_tracks_order ON playlist_tracks(playlist_id, position);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_track ON playlist_tracks(track_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_hash ON playlist_tracks(content_hash);

PRAGMA foreign_keys = ON;

