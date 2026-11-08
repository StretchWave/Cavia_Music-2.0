PRAGMA foreign_keys = ON;

CREATE TABLE tracks (
    id TEXT PRIMARY KEY NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('local', 'provider')),
    provider_id TEXT,
    provider_track_id TEXT,
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    album_artist TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
    file_uri TEXT,
    artwork_uri TEXT,
    mime_type TEXT,
    file_size_bytes INTEGER,
    content_hash TEXT,
    release_year INTEGER,
    track_number INTEGER,
    disc_number INTEGER,
    genre TEXT,
    is_explicit INTEGER NOT NULL DEFAULT 0 CHECK (is_explicit IN (0, 1)),
    is_deleted INTEGER NOT NULL DEFAULT 0 CHECK (is_deleted IN (0, 1)),
    date_added TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_played_at TEXT,
    play_count INTEGER NOT NULL DEFAULT 0 CHECK (play_count >= 0),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    UNIQUE(provider_id, provider_track_id)
);
CREATE INDEX idx_tracks_title ON tracks(title COLLATE NOCASE);
CREATE INDEX idx_tracks_artist ON tracks(artist COLLATE NOCASE);
CREATE INDEX idx_tracks_album ON tracks(album COLLATE NOCASE);
CREATE INDEX idx_tracks_date_added ON tracks(date_added DESC);

CREATE TABLE playlists (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    description TEXT NOT NULL DEFAULT '',
    artwork_uri TEXT,
    source_provider TEXT,
    source_playlist_id TEXT,
    is_deleted INTEGER NOT NULL DEFAULT 0 CHECK (is_deleted IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX idx_playlists_source ON playlists(source_provider, source_playlist_id)
    WHERE source_provider IS NOT NULL AND source_playlist_id IS NOT NULL;

CREATE TABLE playlist_tracks (
    id TEXT PRIMARY KEY NOT NULL,
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(playlist_id, position)
);
CREATE INDEX idx_playlist_tracks_order ON playlist_tracks(playlist_id, position);

CREATE TABLE liked_tracks (
    track_id TEXT PRIMARY KEY NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    liked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE playback_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    queue_json TEXT NOT NULL DEFAULT '[]',
    current_track_id TEXT,
    position_ms INTEGER NOT NULL DEFAULT 0 CHECK (position_ms >= 0),
    repeat_mode TEXT NOT NULL DEFAULT 'off' CHECK (repeat_mode IN ('off', 'all', 'one')),
    shuffle_enabled INTEGER NOT NULL DEFAULT 0 CHECK (shuffle_enabled IN (0, 1)),
    playback_speed REAL NOT NULL DEFAULT 1.0 CHECK (playback_speed > 0 AND playback_speed <= 4.0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE app_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE provider_connections (
    provider_id TEXT PRIMARY KEY NOT NULL,
    account_label TEXT,
    token_reference TEXT,
    connected_at TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE import_history (
    id TEXT PRIMARY KEY NOT NULL,
    source_uri TEXT NOT NULL,
    content_hash TEXT,
    status TEXT NOT NULL,
    error_message TEXT,
    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_import_history_hash ON import_history(content_hash);
