-- Cavia Music 2.0 — Milestone 4.6 Supabase PostgreSQL Cloud Sync Schema
-- Enables user isolation via Row Level Security (RLS) strictly enforcing auth.uid()
-- Never use service-role key or admin privileges in client code.

-- 1. Devices: Registered client devices per user
CREATE TABLE IF NOT EXISTS public.cloud_devices (
    device_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, device_id)
);

ALTER TABLE public.cloud_devices ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Users can only select their own devices" ON public.cloud_devices;
CREATE POLICY "Users can only select their own devices"
    ON public.cloud_devices FOR SELECT
    USING (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only insert their own devices" ON public.cloud_devices;
CREATE POLICY "Users can only insert their own devices"
    ON public.cloud_devices FOR INSERT
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only update their own devices" ON public.cloud_devices;
CREATE POLICY "Users can only update their own devices"
    ON public.cloud_devices FOR UPDATE
    USING (auth.uid() = user_id)
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only delete their own devices" ON public.cloud_devices;
CREATE POLICY "Users can only delete their own devices"
    ON public.cloud_devices FOR DELETE
    USING (auth.uid() = user_id);

-- 2. Playlists: Synchronized user playlists
CREATE TABLE IF NOT EXISTS public.cloud_playlists (
    id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    artwork_uri TEXT,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    client_updated_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, id)
);

ALTER TABLE public.cloud_playlists ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Users can only select their own playlists" ON public.cloud_playlists;
CREATE POLICY "Users can only select their own playlists"
    ON public.cloud_playlists FOR SELECT
    USING (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only insert their own playlists" ON public.cloud_playlists;
CREATE POLICY "Users can only insert their own playlists"
    ON public.cloud_playlists FOR INSERT
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only update their own playlists" ON public.cloud_playlists;
CREATE POLICY "Users can only update their own playlists"
    ON public.cloud_playlists FOR UPDATE
    USING (auth.uid() = user_id)
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only delete their own playlists" ON public.cloud_playlists;
CREATE POLICY "Users can only delete their own playlists"
    ON public.cloud_playlists FOR DELETE
    USING (auth.uid() = user_id);

-- 3. Playlist Tracks: Playlist entries with track snapshot metadata
CREATE TABLE IF NOT EXISTS public.cloud_playlist_tracks (
    id TEXT NOT NULL,
    playlist_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    cloud_track_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT,
    position INTEGER NOT NULL,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, id)
);

ALTER TABLE public.cloud_playlist_tracks ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Users can only select their own playlist tracks" ON public.cloud_playlist_tracks;
CREATE POLICY "Users can only select their own playlist tracks"
    ON public.cloud_playlist_tracks FOR SELECT
    USING (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only insert their own playlist tracks" ON public.cloud_playlist_tracks;
CREATE POLICY "Users can only insert their own playlist tracks"
    ON public.cloud_playlist_tracks FOR INSERT
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only update their own playlist tracks" ON public.cloud_playlist_tracks;
CREATE POLICY "Users can only update their own playlist tracks"
    ON public.cloud_playlist_tracks FOR UPDATE
    USING (auth.uid() = user_id)
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only delete their own playlist tracks" ON public.cloud_playlist_tracks;
CREATE POLICY "Users can only delete their own playlist tracks"
    ON public.cloud_playlist_tracks FOR DELETE
    USING (auth.uid() = user_id);

CREATE INDEX IF NOT EXISTS idx_cloud_playlist_tracks_pl ON public.cloud_playlist_tracks(user_id, playlist_id, position);

-- 4. Liked Tracks: Synchronized user favorites
CREATE TABLE IF NOT EXISTS public.cloud_liked_tracks (
    cloud_track_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    content_hash TEXT,
    title TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 0,
    is_liked BOOLEAN NOT NULL DEFAULT TRUE,
    liked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, cloud_track_id)
);

ALTER TABLE public.cloud_liked_tracks ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Users can only select their own liked tracks" ON public.cloud_liked_tracks;
CREATE POLICY "Users can only select their own liked tracks"
    ON public.cloud_liked_tracks FOR SELECT
    USING (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only insert their own liked tracks" ON public.cloud_liked_tracks;
CREATE POLICY "Users can only insert their own liked tracks"
    ON public.cloud_liked_tracks FOR INSERT
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only update their own liked tracks" ON public.cloud_liked_tracks;
CREATE POLICY "Users can only update their own liked tracks"
    ON public.cloud_liked_tracks FOR UPDATE
    USING (auth.uid() = user_id)
    WITH CHECK (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only delete their own liked tracks" ON public.cloud_liked_tracks;
CREATE POLICY "Users can only delete their own liked tracks"
    ON public.cloud_liked_tracks FOR DELETE
    USING (auth.uid() = user_id);

-- 5. Append-only Sync Events: For incremental delta sync and deterministic server ordering
CREATE TABLE IF NOT EXISTS public.cloud_sync_events (
    server_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    operation_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, operation_id)
);

ALTER TABLE public.cloud_sync_events ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS "Users can only select their own sync events" ON public.cloud_sync_events;
CREATE POLICY "Users can only select their own sync events"
    ON public.cloud_sync_events FOR SELECT
    USING (auth.uid() = user_id);

DROP POLICY IF EXISTS "Users can only insert their own sync events" ON public.cloud_sync_events;
CREATE POLICY "Users can only insert their own sync events"
    ON public.cloud_sync_events FOR INSERT
    WITH CHECK (auth.uid() = user_id);

CREATE INDEX IF NOT EXISTS idx_cloud_sync_events_user_seq ON public.cloud_sync_events(user_id, server_seq ASC);

-- 6. Explicit table privileges for the authenticated role (RLS still applies)
GRANT SELECT, INSERT, UPDATE, DELETE ON public.cloud_devices TO authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.cloud_playlists TO authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.cloud_playlist_tracks TO authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.cloud_liked_tracks TO authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.cloud_sync_events TO authenticated;

-- 7. Keep updated_at current on server-side writes
CREATE OR REPLACE FUNCTION public.cloud_touch_updated_at()
RETURNS TRIGGER
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_cloud_playlists_touch ON public.cloud_playlists;
CREATE TRIGGER trg_cloud_playlists_touch
    BEFORE UPDATE ON public.cloud_playlists
    FOR EACH ROW EXECUTE FUNCTION public.cloud_touch_updated_at();

DROP TRIGGER IF EXISTS trg_cloud_playlist_tracks_touch ON public.cloud_playlist_tracks;
CREATE TRIGGER trg_cloud_playlist_tracks_touch
    BEFORE UPDATE ON public.cloud_playlist_tracks
    FOR EACH ROW EXECUTE FUNCTION public.cloud_touch_updated_at();

DROP TRIGGER IF EXISTS trg_cloud_liked_tracks_touch ON public.cloud_liked_tracks;
CREATE TRIGGER trg_cloud_liked_tracks_touch
    BEFORE UPDATE ON public.cloud_liked_tracks
    FOR EACH ROW EXECUTE FUNCTION public.cloud_touch_updated_at();

-- 8. Ownership and timestamp defaults
--    PostgREST inserts sent by the desktop client may omit user_id; the default
--    keeps auth.uid() as the single source of truth for row ownership.
ALTER TABLE public.cloud_devices ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.cloud_playlists ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.cloud_playlist_tracks ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.cloud_liked_tracks ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.cloud_sync_events ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.cloud_playlists ALTER COLUMN client_updated_at SET DEFAULT now();

