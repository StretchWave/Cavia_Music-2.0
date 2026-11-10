-- Forward-only migration: SQLite FTS5 Trigram Full-Text Search for Tracks and Playlists
-- Supports case-insensitive substring searching across title, artist, album, and playlist metadata.

-- 1. Tracks FTS5 Virtual Table using trigram tokenizer
CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
    track_id UNINDEXED,
    title,
    artist,
    album,
    album_artist,
    genre,
    tokenize = 'trigram'
);

-- Index existing active tracks
INSERT INTO tracks_fts(rowid, track_id, title, artist, album, album_artist, genre)
SELECT rowid, id, title, artist, album, COALESCE(album_artist, ''), COALESCE(genre, '')
FROM tracks
WHERE is_deleted = 0;

-- Synchronization triggers for tracks
CREATE TRIGGER IF NOT EXISTS tracks_fts_ai AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_fts(rowid, track_id, title, artist, album, album_artist, genre)
    SELECT new.rowid, new.id, new.title, new.artist, new.album, COALESCE(new.album_artist, ''), COALESCE(new.genre, '')
    WHERE new.is_deleted = 0;
END;

CREATE TRIGGER IF NOT EXISTS tracks_fts_ad AFTER DELETE ON tracks BEGIN
    DELETE FROM tracks_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS tracks_fts_au AFTER UPDATE ON tracks BEGIN
    DELETE FROM tracks_fts WHERE rowid = old.rowid;
    INSERT INTO tracks_fts(rowid, track_id, title, artist, album, album_artist, genre)
    SELECT new.rowid, new.id, new.title, new.artist, new.album, COALESCE(new.album_artist, ''), COALESCE(new.genre, '')
    WHERE new.is_deleted = 0;
END;

-- 2. Playlists FTS5 Virtual Table using trigram tokenizer
CREATE VIRTUAL TABLE IF NOT EXISTS playlists_fts USING fts5(
    playlist_id UNINDEXED,
    name,
    description,
    tokenize = 'trigram'
);

-- Index existing active playlists
INSERT INTO playlists_fts(rowid, playlist_id, name, description)
SELECT rowid, id, name, description
FROM playlists
WHERE is_deleted = 0;

-- Synchronization triggers for playlists
CREATE TRIGGER IF NOT EXISTS playlists_fts_ai AFTER INSERT ON playlists BEGIN
    INSERT INTO playlists_fts(rowid, playlist_id, name, description)
    SELECT new.rowid, new.id, new.name, new.description
    WHERE new.is_deleted = 0;
END;

CREATE TRIGGER IF NOT EXISTS playlists_fts_ad AFTER DELETE ON playlists BEGIN
    DELETE FROM playlists_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS playlists_fts_au AFTER UPDATE ON playlists BEGIN
    DELETE FROM playlists_fts WHERE rowid = old.rowid;
    INSERT INTO playlists_fts(rowid, playlist_id, name, description)
    SELECT new.rowid, new.id, new.name, new.description
    WHERE new.is_deleted = 0;
END;

