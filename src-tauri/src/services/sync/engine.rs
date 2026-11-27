use super::auth::SupabaseAuthClient;
use super::client::SupabaseRestClient;
use super::outbox::OutboxService;
use super::types::{SyncAccountStatus, SyncResult};
use crate::services::credentials::{
    CAVIA_CREDENTIAL_SERVICE, CredentialStore, SUPABASE_REFRESH_TOKEN_ACCOUNT,
};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SyncEngine {
    pool: SqlitePool,
    auth: SupabaseAuthClient,
    client: SupabaseRestClient,
    credentials: Arc<dyn CredentialStore>,
    sync_lock: Mutex<()>,
}

/// Validates a user-supplied Supabase project URL and publishable key before
/// either is stored or used for requests. HTTPS is required; plain HTTP is
/// accepted only for a loopback development project.
fn normalize_sync_config(url: &str, anon_key: &str) -> Result<(String, String), String> {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Enter your Supabase project URL.".to_string());
    }

    let parsed = url::Url::parse(trimmed)
        .map_err(|_| "That is not a valid Supabase project URL.".to_string())?;
    let is_loopback = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    let scheme_ok = parsed.scheme() == "https" || (is_loopback && parsed.scheme() == "http");
    if !scheme_ok {
        return Err("The sync endpoint must use https:// (http:// only for localhost).".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("The sync endpoint must not contain credentials.".to_string());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("Use the project URL only, without a query or fragment.".to_string());
    }
    if parsed.path() != "/" && !parsed.path().is_empty() {
        return Err("Use the project URL only, without an API path.".to_string());
    }

    let key = anon_key.trim();
    if key.is_empty() {
        return Err("Enter the publishable anon key for this project.".to_string());
    }
    if key.chars().any(char::is_whitespace) {
        return Err("The anon key must not contain whitespace.".to_string());
    }

    Ok((parsed.origin().ascii_serialization(), key.to_string()))
}

impl SyncEngine {
    async fn note_sync_error(&self, message: &str) {
        let _ = sqlx::query(
            "UPDATE sync_state SET last_error = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
        )
        .bind(message)
        .execute(&self.pool)
        .await;
    }

    pub fn new(pool: SqlitePool, credentials: Arc<dyn CredentialStore>) -> Self {
        Self {
            pool,
            auth: SupabaseAuthClient::new(),
            client: SupabaseRestClient::new(),
            credentials,
            sync_lock: Mutex::new(()),
        }
    }

    pub async fn get_config(&self) -> Result<(String, String), String> {
        // Read from app_settings, then fall back to development shell variables.
        let url_json: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'supabase_url'")
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| format!("Database error reading the sync endpoint: {e}"))?;

        let key_json: Option<String> = sqlx::query_scalar(
            "SELECT value_json FROM app_settings WHERE key = 'supabase_anon_key'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error reading the sync credential: {e}"))?;

        let url = url_json
            .and_then(|v| serde_json::from_str::<String>(&v).ok())
            .or_else(|| std::env::var("VITE_SUPABASE_URL").ok())
            .unwrap_or_default();
        let key = key_json
            .and_then(|v| serde_json::from_str::<String>(&v).ok())
            .or_else(|| std::env::var("VITE_SUPABASE_ANON_KEY").ok())
            .unwrap_or_default();

        Ok((url, key))
    }

    pub async fn set_config(&self, url: &str, anon_key: &str) -> Result<(), String> {
        let (url, anon_key) = normalize_sync_config(url, anon_key)?;
        let url_json = serde_json::to_string(&url).map_err(|e| e.to_string())?;
        let key_json = serde_json::to_string(&anon_key).map_err(|e| e.to_string())?;

        sqlx::query("INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('supabase_url', ?, CURRENT_TIMESTAMP)")
            .bind(url_json)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to save the sync endpoint: {e}"))?;

        sqlx::query("INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('supabase_anon_key', ?, CURRENT_TIMESTAMP)")
            .bind(key_json)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to save the sync credential: {e}"))?;

        // A corrected endpoint supersedes whatever the previous configuration
        // reported; do not leave a stale failure on the Account card.
        let _ = sqlx::query(
            "UPDATE sync_state SET last_error = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
        )
        .execute(&self.pool)
        .await;

        Ok(())
    }

    pub async fn get_status(&self) -> Result<SyncAccountStatus, String> {
        let (url, key) = self.get_config().await?;
        let is_configured = !url.is_empty() && !key.is_empty();

        let row = sqlx::query(
            "SELECT account_id, account_email, device_id, sync_enabled, last_synced_at, last_error FROM sync_state WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error reading sync state: {e}"))?;

        let (account_id, account_email, device_id, sync_enabled, last_synced_at, last_error) =
            match row {
                Some(r) => (
                    r.get::<Option<String>, _>("account_id"),
                    r.get::<Option<String>, _>("account_email"),
                    r.get::<String, _>("device_id"),
                    r.get::<i64, _>("sync_enabled") == 1,
                    r.get::<Option<String>, _>("last_synced_at"),
                    r.get::<Option<String>, _>("last_error"),
                ),
                None => (None, None, "dev-local".to_string(), false, None, None),
            };

        let pending_count = OutboxService::get_pending_count(&self.pool)
            .await
            .map_err(|e| format!("Database error counting pending changes: {e}"))?;
        let has_refresh_token = self
            .credentials
            .get(CAVIA_CREDENTIAL_SERVICE, SUPABASE_REFRESH_TOKEN_ACCOUNT)
            .map_err(|e| format!("Secure credential storage is unavailable: {e}"))?
            .is_some();

        Ok(SyncAccountStatus {
            authenticated: account_id.is_some() && has_refresh_token,
            account_id,
            account_email,
            device_id,
            sync_enabled,
            last_synced_at,
            pending_outbox_count: pending_count,
            last_error,
            backend_configured: is_configured,
        })
    }

    pub async fn signup(&self, email: &str, password: &str) -> Result<SyncAccountStatus, String> {
        let (url, key) = self.get_config().await?;
        if url.is_empty() || key.is_empty() {
            return Err("Account sync is not available in this build.".to_string());
        }

        let resp = self.auth.signup(&url, &key, email, password).await?;

        // Save refresh token securely
        self.credentials.set(
            CAVIA_CREDENTIAL_SERVICE,
            SUPABASE_REFRESH_TOKEN_ACCOUNT,
            &resp.refresh_token,
        )?;

        // Update sync_state in SQLite
        sqlx::query(
            r#"
            UPDATE sync_state
            SET account_id = ?,
                account_email = ?,
                sync_enabled = 1,
                last_error = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = 1
            "#,
        )
        .bind(&resp.user.id)
        .bind(resp.user.email.as_deref().unwrap_or(email))
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to save account state: {e}"))?;

        // Perform initial merge so existing local data gets scheduled to sync.
        if let Err(err) = self.initial_merge().await {
            self.note_sync_error(&format!("Signed in, but the local merge failed: {err}"))
                .await;
        }

        self.get_status().await
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<SyncAccountStatus, String> {
        let (url, key) = self.get_config().await?;
        if url.is_empty() || key.is_empty() {
            return Err("Account sync is not available in this build.".to_string());
        }

        let resp = self.auth.login(&url, &key, email, password).await?;

        // Save refresh token securely
        self.credentials.set(
            CAVIA_CREDENTIAL_SERVICE,
            SUPABASE_REFRESH_TOKEN_ACCOUNT,
            &resp.refresh_token,
        )?;

        // Update sync_state in SQLite
        sqlx::query(
            r#"
            UPDATE sync_state
            SET account_id = ?,
                account_email = ?,
                sync_enabled = 1,
                last_error = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = 1
            "#,
        )
        .bind(&resp.user.id)
        .bind(resp.user.email.as_deref().unwrap_or(email))
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to save account state: {e}"))?;

        // Register device (non-fatal: local sync still works without it).
        let status = self.get_status().await?;
        if let Err(err) = self
            .client
            .register_device(
                &url,
                &key,
                &resp.access_token,
                status.account_id.as_deref().unwrap_or_default(),
                &status.device_id,
                "Cavia Desktop",
                std::env::consts::OS,
            )
            .await
        {
            self.note_sync_error(&format!("Device registration failed: {err}"))
                .await;
        }

        // Perform initial merge for local data.
        if let Err(err) = self.initial_merge().await {
            self.note_sync_error(&format!("Signed in, but the local merge failed: {err}"))
                .await;
        }

        self.get_status().await
    }

    pub async fn logout(&self) -> Result<(), String> {
        let (url, key) = self.get_config().await?;
        if let Some(token) = self.auth.get_valid_cached_token().await {
            if !url.is_empty() && !key.is_empty() {
                let _ = self.auth.logout(&url, &key, &token).await;
            }
        }

        // Remove the refresh token from secure storage before reporting a
        // successful sign-out. A keychain failure must not leave a usable
        // token behind while the UI says the account is gone.
        self.credentials
            .delete(CAVIA_CREDENTIAL_SERVICE, SUPABASE_REFRESH_TOKEN_ACCOUNT)
            .map_err(|e| {
                format!("Could not remove the secure session token; you are still signed in: {e}")
            })?;
        self.auth.clear_cached_session().await;

        // Update SQLite sync_state (never deletes local tracks or playlists)
        sqlx::query(
            r#"
            UPDATE sync_state
            SET account_id = NULL,
                account_email = NULL,
                sync_enabled = 0,
                last_error = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = 1
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to clear account state: {e}"))?;

        Ok(())
    }

    pub async fn toggle_sync(&self, enabled: bool) -> Result<bool, String> {
        let val = if enabled { 1 } else { 0 };
        sqlx::query(
            "UPDATE sync_state SET sync_enabled = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
        )
        .bind(val)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to toggle sync: {e}"))?;

        Ok(enabled)
    }

    async fn get_valid_access_token(&self) -> Result<String, String> {
        if let Some(token) = self.auth.get_valid_cached_token().await {
            return Ok(token);
        }

        let (url, key) = self.get_config().await?;
        if url.is_empty() || key.is_empty() {
            return Err("Account sync is not available right now.".to_string());
        }

        let refresh_token = self
            .credentials
            .get(CAVIA_CREDENTIAL_SERVICE, SUPABASE_REFRESH_TOKEN_ACCOUNT)?
            .ok_or_else(|| {
                "Not authenticated: no refresh token available in secure storage".to_string()
            })?;

        let auth_resp = self.auth.refresh(&url, &key, &refresh_token).await?;

        // Update refresh token if rotated. Failing to persist it would leave a
        // stale token in the keychain and break the next session, so surface it.
        if auth_resp.refresh_token != refresh_token {
            self.credentials
                .set(
                    CAVIA_CREDENTIAL_SERVICE,
                    SUPABASE_REFRESH_TOKEN_ACCOUNT,
                    &auth_resp.refresh_token,
                )
                .map_err(|e| format!("Could not store the refreshed secure session: {e}"))?;
        }

        Ok(auth_resp.access_token)
    }

    pub async fn sync(&self) -> Result<SyncResult, String> {
        // Serialize manual and automatic sync requests so the same outbox
        // batch cannot be pushed concurrently by multiple triggers.
        let _sync_guard = self.sync_lock.lock().await;
        let status = self.get_status().await?;
        if !status.sync_enabled {
            return Ok(SyncResult {
                pushed_count: 0,
                pulled_count: 0,
                success: true,
                error_message: Some("Sync is disabled".to_string()),
            });
        }

        let (url, key) = self.get_config().await?;
        if url.is_empty() || key.is_empty() {
            return Ok(SyncResult {
                pushed_count: 0,
                pulled_count: 0,
                success: false,
                error_message: Some("Account sync is not available right now".to_string()),
            });
        }

        let access_token = match self.get_valid_access_token().await {
            Ok(tok) => tok,
            Err(e) => {
                let _ = sqlx::query("UPDATE sync_state SET last_error = ? WHERE id = 1")
                    .bind(&e)
                    .execute(&self.pool)
                    .await;
                return Err(e);
            }
        };

        let device_id = status.device_id.clone();
        let user_id = status.account_id.clone().unwrap_or_default();
        let mut pushed = 0;

        // 1. PUSH: Fetch pending outbox mutations
        let pending = OutboxService::fetch_pending(&self.pool, 50).await?;
        let mut completed_op_ids = Vec::new();

        for item in pending {
            let payload: serde_json::Value = match serde_json::from_str(&item.payload_json) {
                Ok(v) => v,
                Err(e) => {
                    let _ =
                        OutboxService::mark_failed(&self.pool, &item.operation_id, &e.to_string())
                            .await;
                    continue;
                }
            };

            match self
                .client
                .push_sync_event(
                    &url,
                    &key,
                    &access_token,
                    &user_id,
                    &device_id,
                    &item.operation_id,
                    &item.entity_type,
                    &item.entity_id,
                    &item.operation_type,
                    &payload,
                )
                .await
            {
                Ok(()) => {
                    completed_op_ids.push(item.operation_id);
                    pushed += 1;
                }
                Err(err) => {
                    let _ = OutboxService::mark_failed(&self.pool, &item.operation_id, &err).await;
                    // Stop pushing batch on network error
                    break;
                }
            }
        }

        if !completed_op_ids.is_empty() {
            let _ = OutboxService::mark_completed(&self.pool, &completed_op_ids).await;
        }

        // 2. PULL: Pull remote events since cursor
        let cursor_str: Option<String> =
            sqlx::query_scalar("SELECT remote_cursor FROM sync_state WHERE id = 1")
                .fetch_one(&self.pool)
                .await
                .unwrap_or(None);

        let since_seq: i64 = cursor_str
            .as_deref()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        let remote_events = self
            .client
            .pull_events_since(&url, &key, &access_token, since_seq, 100)
            .await?;

        let mut pulled = 0;
        let mut latest_seq = since_seq;

        if !remote_events.is_empty() {
            let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;

            for ev in remote_events {
                let seq = ev.server_seq.unwrap_or(0);
                if seq > latest_seq {
                    latest_seq = seq;
                }

                // If event was emitted by our own device, we already have it locally
                if ev.device_id == device_id {
                    continue;
                }

                // Apply remote event
                match (ev.entity_type.as_str(), ev.operation_type.as_str()) {
                    ("playlist", "create" | "update") => {
                        let is_tombstoned: Option<i64> = sqlx::query_scalar(
                            "SELECT 1 FROM sync_tombstones WHERE entity_type = 'playlist' AND entity_id = ?"
                        )
                        .bind(&ev.entity_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .unwrap_or(None);

                        if is_tombstoned.is_some() {
                            // Don't resurrect deleted playlist
                            continue;
                        }

                        let name = ev
                            .payload
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Untitled Playlist");
                        let desc = ev
                            .payload
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        sqlx::query(
                            r#"
                            INSERT INTO playlists (id, name, description, is_deleted, updated_at)
                            VALUES (?, ?, ?, 0, CURRENT_TIMESTAMP)
                            ON CONFLICT(id) DO UPDATE SET
                                name = excluded.name,
                                description = excluded.description,
                                is_deleted = 0,
                                updated_at = CURRENT_TIMESTAMP
                            "#,
                        )
                        .bind(&ev.entity_id)
                        .bind(name)
                        .bind(desc)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;

                        pulled += 1;
                    }
                    ("playlist", "delete") => {
                        sqlx::query("UPDATE playlists SET is_deleted = 1 WHERE id = ?")
                            .bind(&ev.entity_id)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| e.to_string())?;

                        sqlx::query(
                            "INSERT OR REPLACE INTO sync_tombstones (entity_type, entity_id, deleted_at) VALUES ('playlist', ?, CURRENT_TIMESTAMP)"
                        )
                        .bind(&ev.entity_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;

                        pulled += 1;
                    }
                    ("playlist_track", "add") => {
                        let entry_id = &ev.entity_id;
                        let is_tombstoned: Option<i64> = sqlx::query_scalar(
                            "SELECT 1 FROM sync_tombstones WHERE entity_type = 'playlist_track' AND entity_id = ?"
                        )
                        .bind(entry_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .unwrap_or(None);

                        if is_tombstoned.is_some() {
                            continue;
                        }

                        let pl_id = ev
                            .payload
                            .get("playlistId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let cloud_trk_id = ev
                            .payload
                            .get("cloudTrackId")
                            .and_then(|v| v.as_str())
                            .unwrap_or(entry_id);
                        let title = ev
                            .payload
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown Title");
                        let artist = ev
                            .payload
                            .get("artist")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let album = ev
                            .payload
                            .get("album")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let dur = ev
                            .payload
                            .get("durationMs")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let hash = ev.payload.get("contentHash").and_then(|v| v.as_str());

                        // Check if exact audio file is present in local library
                        let matched_local_id: Option<String> = if let Some(h) = hash {
                            sqlx::query_scalar("SELECT id FROM tracks WHERE content_hash = ? AND is_deleted = 0 LIMIT 1")
                                .bind(h)
                                .fetch_optional(&mut *tx)
                                .await
                                .unwrap_or(None)
                        } else {
                            None
                        };

                        let next_pos: i64 = sqlx::query_scalar(
                            "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?"
                        )
                        .bind(pl_id)
                        .fetch_one(&mut *tx)
                        .await
                        .unwrap_or(0);

                        sqlx::query(
                            r#"
                            INSERT INTO playlist_tracks (id, playlist_id, track_id, cloud_track_id, title, artist, album, duration_ms, content_hash, position)
                            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(id) DO UPDATE SET
                                title = excluded.title,
                                artist = excluded.artist,
                                album = excluded.album,
                                duration_ms = excluded.duration_ms,
                                content_hash = excluded.content_hash,
                                track_id = COALESCE(excluded.track_id, playlist_tracks.track_id)
                            "#,
                        )
                        .bind(entry_id)
                        .bind(pl_id)
                        .bind(matched_local_id)
                        .bind(cloud_trk_id)
                        .bind(title)
                        .bind(artist)
                        .bind(album)
                        .bind(dur)
                        .bind(hash)
                        .bind(next_pos)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;

                        pulled += 1;
                    }
                    ("playlist_track", "remove") => {
                        sqlx::query("DELETE FROM playlist_tracks WHERE id = ?")
                            .bind(&ev.entity_id)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| e.to_string())?;

                        sqlx::query(
                            "INSERT OR REPLACE INTO sync_tombstones (entity_type, entity_id, deleted_at) VALUES ('playlist_track', ?, CURRENT_TIMESTAMP)"
                        )
                        .bind(&ev.entity_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;

                        pulled += 1;
                    }
                    ("liked_track", "like") => {
                        let hash = ev.payload.get("contentHash").and_then(|v| v.as_str());
                        if let Some(h) = hash {
                            if let Some(local_id) = sqlx::query_scalar::<_, String>(
                                "SELECT id FROM tracks WHERE content_hash = ? AND is_deleted = 0 LIMIT 1"
                            )
                            .bind(h)
                            .fetch_optional(&mut *tx)
                            .await
                            .unwrap_or(None) {
                                let _ = sqlx::query(
                                    "INSERT OR IGNORE INTO liked_tracks (track_id, liked_at) VALUES (?, CURRENT_TIMESTAMP)"
                                )
                                .bind(&local_id)
                                .execute(&mut *tx)
                                .await;
                            }
                        }
                        pulled += 1;
                    }
                    ("liked_track", "unlike") => {
                        let hash = ev.payload.get("contentHash").and_then(|v| v.as_str());
                        if let Some(h) = hash {
                            if let Some(local_id) = sqlx::query_scalar::<_, String>(
                                "SELECT id FROM tracks WHERE content_hash = ? LIMIT 1",
                            )
                            .bind(h)
                            .fetch_optional(&mut *tx)
                            .await
                            .unwrap_or(None)
                            {
                                let _ = sqlx::query("DELETE FROM liked_tracks WHERE track_id = ?")
                                    .bind(&local_id)
                                    .execute(&mut *tx)
                                    .await;
                            }
                        }
                        pulled += 1;
                    }
                    _ => {}
                }
            }

            // Save new cursor
            sqlx::query(
                r#"
                UPDATE sync_state
                SET remote_cursor = ?,
                    last_synced_at = CURRENT_TIMESTAMP,
                    last_error = NULL,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = 1
                "#,
            )
            .bind(latest_seq.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

            tx.commit().await.map_err(|e| e.to_string())?;
        } else {
            // Update last_synced_at timestamp even if no remote events
            let _ = sqlx::query("UPDATE sync_state SET last_synced_at = CURRENT_TIMESTAMP, last_error = NULL WHERE id = 1")
                .execute(&self.pool)
                .await;
        }

        // 3. Match unlinked tracks against local library
        let _ = self.match_unlinked_tracks().await;

        Ok(SyncResult {
            pushed_count: pushed,
            pulled_count: pulled,
            success: true,
            error_message: None,
        })
    }

    pub async fn initial_merge(&self) -> Result<(), String> {
        let playlists =
            sqlx::query("SELECT id, name, description FROM playlists WHERE is_deleted = 0")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| e.to_string())?;

        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;

        for row in playlists {
            let id: String = row.get("id");
            let name: String = row.get("name");
            let desc: String = row.get("description");

            // Queue playlist create if not already in outbox
            let exists: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM sync_outbox WHERE entity_type = 'playlist' AND entity_id = ?",
            )
            .bind(&id)
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);

            if exists.is_none() {
                let _ = OutboxService::record_playlist_create(&mut *conn, &id, &name, &desc).await;
            }

            // Queue tracks in that playlist
            let tracks = sqlx::query(
                r#"
                SELECT
                    pt.id AS entry_id,
                    pt.cloud_track_id,
                    COALESCE(t.title, pt.title, 'Unknown') AS title,
                    COALESCE(t.artist, pt.artist, '') AS artist,
                    COALESCE(t.album, pt.album, '') AS album,
                    COALESCE(t.duration_ms, pt.duration_ms, 0) AS duration_ms,
                    COALESCE(t.content_hash, pt.content_hash) AS content_hash,
                    pt.position
                FROM playlist_tracks pt
                LEFT JOIN tracks t ON pt.track_id = t.id
                WHERE pt.playlist_id = ?
                ORDER BY pt.position ASC
                "#,
            )
            .bind(&id)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();

            for trk in tracks {
                let entry_id: String = trk.get("entry_id");
                let trk_exists: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM sync_outbox WHERE entity_type = 'playlist_track' AND entity_id = ?"
                )
                .bind(&entry_id)
                .fetch_optional(&mut *conn)
                .await
                .unwrap_or(None);

                if trk_exists.is_none() {
                    let cloud_id: Option<String> = trk.get("cloud_track_id");
                    let cloud_id =
                        cloud_id.unwrap_or_else(|| format!("trk-{}", uuid::Uuid::new_v4()));
                    let title: String = trk.get("title");
                    let artist: String = trk.get("artist");
                    let album: String = trk.get("album");
                    let dur: i64 = trk.get("duration_ms");
                    let hash: Option<String> = trk.get("content_hash");
                    let pos: i64 = trk.get("position");

                    let _ = OutboxService::record_track_add(
                        &mut *conn,
                        &id,
                        &entry_id,
                        &cloud_id,
                        &title,
                        &artist,
                        &album,
                        dur,
                        hash.as_deref(),
                        pos,
                    )
                    .await;
                }
            }
        }

        // Queue liked tracks
        let liked = sqlx::query(
            r#"
            SELECT t.id, t.title, t.artist, t.album, t.duration_ms, t.content_hash
            FROM liked_tracks l
            JOIN tracks t ON l.track_id = t.id
            WHERE t.is_deleted = 0
            "#,
        )
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        for l in liked {
            let id: String = l.get("id");
            let exists: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM sync_outbox WHERE entity_type = 'liked_track' AND entity_id = ?",
            )
            .bind(&id)
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);

            if exists.is_none() {
                let title: String = l.get("title");
                let artist: String = l.get("artist");
                let album: String = l.get("album");
                let dur: i64 = l.get("duration_ms");
                let hash: Option<String> = l.get("content_hash");

                let _ = OutboxService::record_track_like_status(
                    &mut *conn,
                    &id,
                    hash.as_deref(),
                    &title,
                    &artist,
                    &album,
                    dur,
                    true,
                )
                .await;
            }
        }

        Ok(())
    }

    pub async fn match_unlinked_tracks(&self) -> Result<usize, String> {
        let result = sqlx::query(
            r#"
            UPDATE playlist_tracks
            SET track_id = (
                SELECT t.id
                FROM tracks t
                WHERE t.content_hash = playlist_tracks.content_hash
                  AND t.is_deleted = 0
                LIMIT 1
            )
            WHERE playlist_tracks.track_id IS NULL
              AND playlist_tracks.content_hash IS NOT NULL
              AND EXISTS (
                SELECT 1
                FROM tracks t
                WHERE t.content_hash = playlist_tracks.content_hash
                  AND t.is_deleted = 0
              )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to match unlinked tracks: {e}"))?;

        Ok(result.rows_affected() as usize)
    }

    pub async fn delete_cloud_data(&self) -> Result<(), String> {
        let (url, key) = self.get_config().await?;
        if url.is_empty() || key.is_empty() {
            return Err("Account sync is not available right now".to_string());
        }

        let access_token = self.get_valid_access_token().await?;
        self.client
            .delete_all_cloud_data(&url, &key, &access_token)
            .await?;

        // Clear local sync outbox and reset cursor (does NOT delete local playlists or audio!)
        let _ = OutboxService::clear_all(&self.pool).await;

        sqlx::query(
            r#"
            UPDATE sync_state
            SET remote_cursor = NULL,
                last_synced_at = NULL,
                last_error = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = 1
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to reset local sync cursor: {e}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod config_tests {
    use super::{SyncEngine, normalize_sync_config};
    use crate::services::credentials::InMemoryCredentialStore;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn accepts_https_project_url_and_key() {
        let (url, key) =
            normalize_sync_config(" https://example.supabase.co/ ", " sb_publishable_abc ")
                .expect("valid config");
        assert_eq!(url, "https://example.supabase.co");
        assert_eq!(key, "sb_publishable_abc");
    }

    #[test]
    fn allows_loopback_http_for_development() {
        let (url, _) = normalize_sync_config("http://127.0.0.1:54321", "key")
            .expect("loopback http should be allowed");
        assert_eq!(url, "http://127.0.0.1:54321");
    }

    #[test]
    fn rejects_plain_http_credentials_and_api_paths() {
        assert!(normalize_sync_config("http://example.com", "key").is_err());
        assert!(normalize_sync_config("https://user:pass@example.com", "key").is_err());
        assert!(normalize_sync_config("https://example.com/rest/v1", "key").is_err());
        assert!(normalize_sync_config("https://example.com/?x=1", "key").is_err());
    }

    #[test]
    fn rejects_empty_or_whitespace_keys() {
        assert!(normalize_sync_config("https://example.com", "").is_err());
        assert!(normalize_sync_config("https://example.com", "a b").is_err());
    }

    #[tokio::test]
    async fn sync_lock_serializes_automatic_and_manual_triggers() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        let engine = Arc::new(SyncEngine::new(
            pool,
            Arc::new(InMemoryCredentialStore::new()),
        ));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for _ in 0..4 {
            let engine = Arc::clone(&engine);
            let active = Arc::clone(&active);
            let maximum = Arc::clone(&maximum);
            tasks.push(tokio::spawn(async move {
                let _guard = engine.sync_lock.lock().await;
                let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(running, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for task in tasks {
            task.await.expect("sync trigger task");
        }
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
    }
}
