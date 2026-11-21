-- Forward-only: playlist entries that point at a catalogue provider instead
-- of a local file. Existing local rows keep NULL in all three columns, so the
-- playlist reader falls back to the joined track exactly as before.
ALTER TABLE playlist_tracks ADD COLUMN provider_id TEXT;
ALTER TABLE playlist_tracks ADD COLUMN provider_track_id TEXT;
ALTER TABLE playlist_tracks ADD COLUMN artwork_uri TEXT;