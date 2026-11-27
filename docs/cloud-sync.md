# Cavia Music 2.0 — Milestone 4.6: Optional Account & Cloud Sync Architecture

## 1. Overview & Principles

Cavia Music 2.0 is designed as a **local-first, cloud-assisted** music system.
- **Local SQLite is authoritative** for playback, indexing, artwork, and local libraries.
- The app operates **100% offline** and without an account.
- **Zero Audio Upload**: Audio files, file bytes, and local filesystem paths are **never** uploaded to the cloud.
- **Zero Fake Availability**: Cloud-synced playlist tracks that do not exist locally on the current device are rendered honestly as unplayable/remote-only ("Metadata only"), with playback controls disabled.
- **Durable Local Change Journal (Outbox)**: Mutations made offline or online are queued in `sync_outbox` transactionally with local SQLite modifications.
- **Idempotency & Tombstones**: Each mutation carries a unique operation UUID. Deletions produce tombstones in `sync_tombstones` to prevent stale resurrection.
- **Supabase Backend with Row Level Security (RLS)**: Client authentication uses standard JWT bearer tokens. Postgres RLS guarantees user data isolation via `auth.uid() = user_id`.
- **Zero Service-Role Key**: Cavia **never** ships or requires Supabase service-role keys or database passwords.

---

## 2. Supabase Setup Instructions

To enable optional cloud synchronization for a Cavia deployment:

1. **Create a Supabase Project**:
   - Go to [supabase.com](https://supabase.com) and create a free project.
2. **Apply Migrations**:
   - In the Supabase Dashboard, open the **SQL Editor**.
   - Copy and execute the contents of `supabase/migrations/20261112000000_cloud_sync_schema.sql`. The script is idempotent (`CREATE TABLE IF NOT EXISTS`, `DROP POLICY IF EXISTS`), so it is safe to re-run after pulling changes.
   - Optionally run `test/verify_supabase_rls.sql` to verify RLS isolation.
   - The `user_id` columns default to `auth.uid()`, and `cloud_playlists.client_updated_at` defaults to `now()`, so a client insert can never create a row owned by another account.

   The live project (`wycltgbzbhvnfavwxgpm`) already has this schema applied. Email confirmation is disabled on it because the desktop client authenticates with email + password and expects a session immediately; enable confirmation only once transactional email is configured.
3. **Configure the Client**:
   - Open **Settings → Account → Sync backend** in the installed app and paste the project URL and the publishable `anon` key. The project URL must use `https://` (plain `http://` is accepted only for a loopback development project), and only the public anon key is permitted.
   - The anon key is stored in the local app database because it is publishable by design. The account refresh token is stored separately in the OS credential manager.
   - A development shell may instead export `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY` before launching the Rust process. Variables in `.env` are not read by the packaged application.

---

## 3. Data Sync Model

### Synchronized Data
- **Playlists**: ID, name, description, artwork URI, timestamps, deleted state.
- **Playlist Tracks**: Stable entry ID, playlist ID, track identity snapshot (title, artist, album, duration_ms, content_hash), position, added timestamp.
- **Liked Tracks**: Track identity snapshot, liked/unliked status, modification timestamp.
- **Device Registrations**: Device ID, device name, platform, last seen.
- **Sync Journal Events**: Operation ID, sequence, payload, created timestamp.

### Explicitly Excluded Data
- Local audio files and byte streams.
- Absolute file paths, Windows drive paths, content URIs, iOS file URLs.
- Artwork cache files and local filesystem paths.
- Local SQLite database files.
- Active playback queue, playback position, volume, and audio device settings.
- Scanned directory roots and import history.
- Plaintext credentials and secrets.

---

## 4. Conflict Resolution & Tombstones

- **Deletions**: Soft-deleted locally with `is_deleted = 1` and recorded in `sync_tombstones`. When remote events arrive for an entity with a tombstone, older edits are discarded to prevent resurrection.
- **Reordering**: Ordered by position index with two-phase collision avoidance. In concurrent sync, server monotonic event sequence assigns canonical order.
- **Track Linking**: When an unavailable remote track's exact audio file (SHA-256 `content_hash`) is later scanned locally, Cavia links the local track ID, immediately enabling local playback.

## 5. Automatic Sync Lifecycle

When sync is enabled and an account is authenticated, the desktop client schedules a debounced sync after the library loads or refreshes. It also schedules one when the app becomes visible again, the network returns, or the user completes sign-in or enables sync in Settings. Repeated triggers replace the pending debounce timer. A Rust-side async mutex serializes automatic and manual sync calls so they cannot push the same outbox batch concurrently.

Failed attempts are retried with exponential delays capped at five minutes. Outbox entries remain in SQLite until the existing acknowledgment path marks them complete. When sync is disabled or the account is signed out, the client does not start a network sync; disabling sync also clears a pending automatic retry.

