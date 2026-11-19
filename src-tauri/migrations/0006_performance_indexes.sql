-- Forward-only migration: Performance indexes for library scanning and active queries
-- 1. Index content_hash and file_uri for O(log N) scanner duplicate and rename detection
CREATE INDEX IF NOT EXISTS idx_tracks_content_hash ON tracks(content_hash);
CREATE INDEX IF NOT EXISTS idx_tracks_file_uri ON tracks(file_uri);

-- 2. Partial indexes for active tracks (is_deleted = 0) to accelerate library listings and groupings
CREATE INDEX IF NOT EXISTS idx_tracks_active_title ON tracks(title COLLATE NOCASE) WHERE is_deleted = 0;
CREATE INDEX IF NOT EXISTS idx_tracks_active_album ON tracks(album COLLATE NOCASE, artist COLLATE NOCASE) WHERE is_deleted = 0;
CREATE INDEX IF NOT EXISTS idx_tracks_active_artist ON tracks(artist COLLATE NOCASE) WHERE is_deleted = 0;
