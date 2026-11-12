-- Cavia Music 2.0: Supabase RLS Verification Script
-- Tests cross-user isolation and unauthenticated access prevention.
-- Run in Supabase SQL Editor or psql to verify RLS policy correctness.

BEGIN;

-- Setup test users in auth.users
INSERT INTO auth.users (id, email)
VALUES 
  ('11111111-1111-1111-1111-111111111111', 'user_a@example.com'),
  ('22222222-2222-2222-2222-222222222222', 'user_b@example.com')
ON CONFLICT (id) DO NOTHING;

-- TEST 1: User A acts as authenticated user
SET LOCAL ROLE authenticated;
SET LOCAL "request.jwt.claim.sub" = '11111111-1111-1111-1111-111111111111';

-- User A inserts a playlist
INSERT INTO public.cloud_playlists (id, user_id, name, description, client_updated_at)
VALUES ('pl-user-a-1', '11111111-1111-1111-1111-111111111111', 'User A Playlist', 'Private playlist', now());

-- User A inserts a playlist track
INSERT INTO public.cloud_playlist_tracks (id, playlist_id, user_id, cloud_track_id, title, artist, position)
VALUES ('plt-a-1', 'pl-user-a-1', '11111111-1111-1111-1111-111111111111', 'trk-1', 'Track 1', 'Artist A', 0);

-- User A inserts a liked track
INSERT INTO public.cloud_liked_tracks (cloud_track_id, user_id, title, artist)
VALUES ('trk-1', '11111111-1111-1111-1111-111111111111', 'Track 1', 'Artist A');

-- User A inserts a sync event
INSERT INTO public.cloud_sync_events (operation_id, user_id, device_id, entity_type, entity_id, operation_type, payload)
VALUES ('op-a-1', '11111111-1111-1111-1111-111111111111', 'dev-a', 'playlist', 'pl-user-a-1', 'create', '{"name":"User A Playlist"}');

-- Verify User A can see their own records
DO $$
DECLARE
  cnt INT;
BEGIN
  SELECT count(*) INTO cnt FROM public.cloud_playlists;
  ASSERT cnt = 1, 'User A should see exactly 1 playlist';
  SELECT count(*) INTO cnt FROM public.cloud_playlist_tracks;
  ASSERT cnt = 1, 'User A should see exactly 1 playlist track';
  SELECT count(*) INTO cnt FROM public.cloud_liked_tracks;
  ASSERT cnt = 1, 'User A should see exactly 1 liked track';
  SELECT count(*) INTO cnt FROM public.cloud_sync_events;
  ASSERT cnt = 1, 'User A should see exactly 1 sync event';
  RAISE NOTICE 'SUCCESS: User A self-read verified';
END $$;

-- TEST 2: User B tries to read, modify, or delete User A's data
SET LOCAL "request.jwt.claim.sub" = '22222222-2222-2222-2222-222222222222';

DO $$
DECLARE
  cnt INT;
  aff INT;
BEGIN
  -- Read attempt
  SELECT count(*) INTO cnt FROM public.cloud_playlists WHERE id = 'pl-user-a-1';
  ASSERT cnt = 0, 'User B must not read User A playlist';

  SELECT count(*) INTO cnt FROM public.cloud_playlist_tracks WHERE playlist_id = 'pl-user-a-1';
  ASSERT cnt = 0, 'User B must not read User A playlist tracks';

  SELECT count(*) INTO cnt FROM public.cloud_liked_tracks WHERE cloud_track_id = 'trk-1';
  ASSERT cnt = 0, 'User B must not read User A liked tracks';

  SELECT count(*) INTO cnt FROM public.cloud_sync_events WHERE operation_id = 'op-a-1';
  ASSERT cnt = 0, 'User B must not read User A sync events';

  -- Update attempt
  UPDATE public.cloud_playlists SET name = 'Hacked' WHERE id = 'pl-user-a-1';
  GET DIAGNOSTICS aff = ROW_COUNT;
  ASSERT aff = 0, 'User B update on User A playlist must affect 0 rows';

  -- Delete attempt
  DELETE FROM public.cloud_playlists WHERE id = 'pl-user-a-1';
  GET DIAGNOSTICS aff = ROW_COUNT;
  ASSERT aff = 0, 'User B delete on User A playlist must affect 0 rows';

  DELETE FROM public.cloud_sync_events WHERE operation_id = 'op-a-1';
  GET DIAGNOSTICS aff = ROW_COUNT;
  ASSERT aff = 0, 'User B delete on User A sync events must affect 0 rows';

  RAISE NOTICE 'SUCCESS: User B isolation verified';
END $$;

-- TEST 3: User B tries to forge User A user_id in an INSERT
DO $$
BEGIN
  BEGIN
    INSERT INTO public.cloud_playlists (id, user_id, name, description, client_updated_at)
    VALUES ('pl-user-b-forged', '11111111-1111-1111-1111-111111111111', 'Forged', '', now());
    RAISE EXCEPTION 'RLS CHECK policy failure expected on forged user_id';
  EXCEPTION WHEN check_violation OR insufficient_privilege THEN
    RAISE NOTICE 'SUCCESS: Forged insert correctly rejected by RLS WITH CHECK';
  END;
END $$;

-- TEST 4: Anonymous role has zero read access
SET LOCAL ROLE anon;
DO $$
DECLARE
  cnt INT;
BEGIN
  SELECT count(*) INTO cnt FROM public.cloud_playlists;
  ASSERT cnt = 0, 'Anon role must see 0 playlists';

  SELECT count(*) INTO cnt FROM public.cloud_sync_events;
  ASSERT cnt = 0, 'Anon role must see 0 sync events';

  RAISE NOTICE 'SUCCESS: Anon role lockdown verified';
END $$;

ROLLBACK;

