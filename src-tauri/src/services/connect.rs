use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::domain::Track;
use crate::services::fair_queue::{
    DEFAULT_FAIRNESS_ALPHA, FairQueueEngine, FairnessConfig, ParticipantRole, ParticipantState,
    ParticipantWeightSummary, QueueEntryStatus, SharedQueueEntry, attach_entry_metadata,
    canonical_provider_id,
};
use crate::services::preload::{
    PreloadCoordinator, PreloadFetcher, extract_ytmusic_video_id, is_connectivity_error,
};
use crate::services::sync::engine::SyncEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectSessionSnapshot {
    pub active: bool,
    pub session_id: String,
    pub join_code: String,
    pub name: String,
    pub local_participant_id: String,
    pub local_role: ParticipantRole,
    pub host_participant_id: String,
    pub host_display_name: String,
    pub fairness_alpha: f64,
    pub preload_target: usize,
    pub queue_revision: u64,
    pub current_entry: Option<SharedQueueEntry>,
    pub shared_pool: Vec<SharedQueueEntry>,
    pub upcoming_sequence: Vec<SharedQueueEntry>,
    pub preload_window: Vec<SharedQueueEntry>,
    pub pending_preload: Vec<SharedQueueEntry>,
    pub weights: Vec<ParticipantWeightSummary>,
    pub unsynced_event_count: usize,
    pub last_sync_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectPlaylistContributionResult {
    pub accepted_count: usize,
    pub rejected_duplicates: usize,
    pub rejected_unsupported: usize,
    pub pending_preload_count: usize,
    pub snapshot: ConnectSessionSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectContributionResult {
    pub entry: SharedQueueEntry,
    pub snapshot: ConnectSessionSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectSyncResult {
    pub pushed_events: usize,
    pub pulled_events: usize,
    pub skipped_duplicates: usize,
    pub revision: u64,
    pub offline: bool,
    pub error_message: Option<String>,
}

struct ActiveConnectSession {
    session_id: String,
    join_code: String,
    name: String,
    host_participant_id: String,
    host_display_name: String,
    local_participant_id: String,
    local_role: ParticipantRole,
    engine: FairQueueEngine,
    last_sync_error: Option<String>,
}

pub struct ConnectService {
    db: SqlitePool,
    app_handle: Option<AppHandle>,
    fetcher: Option<Arc<dyn PreloadFetcher>>,
    sync_engine: Option<Arc<SyncEngine>>,
    preload: Mutex<Option<Arc<PreloadCoordinator>>>,
    session: Mutex<Option<ActiveConnectSession>>,
    applied_event_ids: Mutex<HashSet<String>>,
}

impl ConnectService {
    pub fn new(
        db: SqlitePool,
        app_handle: Option<AppHandle>,
        fetcher: Option<Arc<dyn PreloadFetcher>>,
        sync_engine: Option<Arc<SyncEngine>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            app_handle,
            fetcher,
            sync_engine,
            preload: Mutex::new(None),
            session: Mutex::new(None),
            applied_event_ids: Mutex::new(HashSet::new()),
        })
    }

    pub fn attach_preload(&self, preload: Arc<PreloadCoordinator>) {
        if let Ok(mut guard) = self.preload.lock() {
            *guard = Some(preload);
        }
    }

    fn current_preload_target(&self) -> usize {
        self.preload
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|p| p.snapshot().target_count))
            .unwrap_or(10)
    }

    fn is_track_cached_locally(&self, track: &Track) -> bool {
        if track.source_kind == "local" {
            return track
                .file_uri
                .as_deref()
                .is_some_and(|p| std::path::Path::new(p).is_file());
        }
        if let (Some(fetcher), Some(vid)) = (&self.fetcher, extract_ytmusic_video_id(track)) {
            return fetcher.cached_playable_path(&vid).is_some();
        }
        false
    }

    pub async fn local_participant_id(&self) -> Result<String, String> {
        if let Some(row) =
            sqlx::query("SELECT value_json FROM app_settings WHERE key = 'connect_participant_id'")
                .fetch_optional(&self.db)
                .await
                .map_err(|e| format!("Failed to read connect participant id: {e}"))?
        {
            let val: String = row.get("value_json");
            let trimmed = val.trim_matches('"').trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
        let generated = format!("part-{}", Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value_json, updated_at)
            VALUES ('connect_participant_id', ?, CURRENT_TIMESTAMP)
            ON CONFLICT(key) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(serde_json::to_string(&generated).unwrap_or_else(|_| format!("\"{generated}\"")))
        .execute(&self.db)
        .await
        .map_err(|e| format!("Failed to persist connect participant id: {e}"))?;
        Ok(generated)
    }

    pub async fn restore_active_session(&self) -> Result<Option<ConnectSessionSnapshot>, String> {
        let row = sqlx::query(
            r#"
            SELECT session_id, join_code, name, host_participant_id, host_display_name,
                   local_participant_id, local_role, fairness_alpha, queue_revision, session_seed
            FROM connect_sessions
            WHERE is_active = 1
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.db)
        .await
        .map_err(|e| format!("Failed to load active connect session: {e}"))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let session_id: String = row.get("session_id");
        let join_code: String = row.get("join_code");
        let name: String = row.get("name");
        let host_participant_id: String = row.get("host_participant_id");
        let host_display_name: String = row.get("host_display_name");
        let local_participant_id: String = row.get("local_participant_id");
        let local_role_raw: String = row.get("local_role");
        let local_role = if local_role_raw == "guest" {
            ParticipantRole::Guest
        } else {
            ParticipantRole::Host
        };
        let alpha: f64 = row.get("fairness_alpha");
        let queue_revision: i64 = row.get("queue_revision");
        let session_seed: i64 = row.get("session_seed");

        let participant_rows = sqlx::query(
            r#"
            SELECT participant_id, display_name, role, is_connected, penalty_credits
            FROM connect_participants
            WHERE session_id = ?
            "#,
        )
        .bind(&session_id)
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("Failed to restore connect participants: {e}"))?;

        let participants: Vec<ParticipantState> = participant_rows
            .into_iter()
            .map(|r| {
                let role_str: String = r.get("role");
                ParticipantState {
                    participant_id: r.get("participant_id"),
                    display_name: r.get("display_name"),
                    role: if role_str == "guest" {
                        ParticipantRole::Guest
                    } else {
                        ParticipantRole::Host
                    },
                    is_connected: r.get::<i64, _>("is_connected") == 1,
                    penalty_credits: r.get("penalty_credits"),
                    total_added: 0,
                    total_played: 0,
                }
            })
            .collect();

        let entry_rows = sqlx::query(
            r#"
            SELECT queue_entry_id, track_json, canonical_provider_id, contributor_id,
                   contributor_name, status, priority_override, manual_position,
                   committed_rank, created_at_seq
            FROM connect_queue_entries
            WHERE session_id = ? AND status NOT IN ('played', 'removed')
            ORDER BY created_at_seq ASC
            "#,
        )
        .bind(&session_id)
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("Failed to restore connect queue entries: {e}"))?;

        let mut entries = Vec::with_capacity(entry_rows.len());
        for r in entry_rows {
            let track_json: String = r.get("track_json");
            let Ok(track) = serde_json::from_str::<Track>(&track_json) else {
                continue;
            };
            let status_raw: String = r.get("status");
            let status = match status_raw.as_str() {
                "pending_preload" => QueueEntryStatus::PendingPreload,
                "playing" => QueueEntryStatus::Playing,
                _ => QueueEntryStatus::Ready,
            };
            let committed_rank: Option<i64> = r.get("committed_rank");
            let manual_position: Option<i64> = r.get("manual_position");
            let contributor_id: String = r.get("contributor_id");
            let role = if contributor_id == host_participant_id {
                ParticipantRole::Host
            } else {
                ParticipantRole::Guest
            };
            entries.push(SharedQueueEntry {
                queue_entry_id: r.get("queue_entry_id"),
                track,
                canonical_provider_id: r
                    .get::<Option<String>, _>("canonical_provider_id")
                    .unwrap_or_default(),
                contributor_id,
                contributor_name: r.get("contributor_name"),
                role,
                status,
                priority_override: r.get::<i64, _>("priority_override") == 1,
                manual_position: manual_position.map(|v| v.max(0) as usize),
                committed: committed_rank.is_some(),
                committed_rank: committed_rank.map(|v| v.max(0) as u64),
                created_at_seq: r.get::<i64, _>("created_at_seq").max(0) as u64,
            });
        }

        let mut engine = FairQueueEngine::new(
            FairnessConfig {
                alpha: alpha.clamp(0.0, 5.0),
                preferred_spacing: 2,
            },
            session_seed.max(1) as u64,
        );
        engine.restore_state(participants, entries, queue_revision.max(0) as u64);

        let event_rows =
            sqlx::query("SELECT event_id FROM connect_contribution_events WHERE session_id = ?")
                .bind(&session_id)
                .fetch_all(&self.db)
                .await
                .unwrap_or_default();
        if let Ok(mut applied) = self.applied_event_ids.lock() {
            applied.clear();
            for r in event_rows {
                applied.insert(r.get("event_id"));
            }
        }

        {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            *guard = Some(ActiveConnectSession {
                session_id,
                join_code,
                name,
                host_participant_id,
                host_display_name,
                local_participant_id,
                local_role,
                engine,
                last_sync_error: None,
            });
        }

        self.sync_preload_coordinator();
        self.get_snapshot().await
    }

    pub async fn create_session(
        &self,
        name: &str,
        host_display_name: &str,
        alpha: Option<f64>,
    ) -> Result<ConnectSessionSnapshot, String> {
        let trimmed_name = name.trim();
        let session_name = if trimmed_name.is_empty() {
            "Cavia Connect Session".to_string()
        } else {
            trimmed_name.to_string()
        };
        let host_name = if host_display_name.trim().is_empty() {
            "Host".to_string()
        } else {
            host_display_name.trim().to_string()
        };

        let host_id = self.local_participant_id().await?;
        let session_id = format!("conn-{}", Uuid::new_v4());
        let short_code = Uuid::new_v4().simple().to_string();
        let join_code = format!("CAVIA-{}", &short_code[..6].to_ascii_uppercase());
        let seed = (Uuid::new_v4().as_u128() & 0xFFFF_FFFF_FFFF_FFFF) as u64;
        let alpha_val = alpha.unwrap_or(DEFAULT_FAIRNESS_ALPHA).clamp(0.0, 5.0);
        let preload_target = self.current_preload_target();

        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| format!("Failed to begin connect session transaction: {e}"))?;
        sqlx::query("UPDATE connect_sessions SET is_active = 0, updated_at = CURRENT_TIMESTAMP WHERE is_active = 1")
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to archive previous connect sessions: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO connect_sessions (
                session_id, join_code, name, host_participant_id, host_display_name,
                local_participant_id, local_role, fairness_alpha, preload_target,
                queue_revision, session_seed, is_active
            ) VALUES (?, ?, ?, ?, ?, ?, 'host', ?, ?, 0, ?, 1)
            "#,
        )
        .bind(&session_id)
        .bind(&join_code)
        .bind(&session_name)
        .bind(&host_id)
        .bind(&host_name)
        .bind(&host_id)
        .bind(alpha_val)
        .bind(preload_target as i64)
        .bind(seed as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to create connect session: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO connect_participants (session_id, participant_id, display_name, role, is_connected, penalty_credits)
            VALUES (?, ?, ?, 'host', 1, 0.0)
            "#,
        )
        .bind(&session_id)
        .bind(&host_id)
        .bind(&host_name)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to persist host participant: {e}"))?;

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit connect session: {e}"))?;

        let mut engine = FairQueueEngine::new(
            FairnessConfig {
                alpha: alpha_val,
                preferred_spacing: 2,
            },
            seed,
        );
        engine.upsert_participant(&host_id, &host_name, ParticipantRole::Host, true);

        {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            *guard = Some(ActiveConnectSession {
                session_id,
                join_code,
                name: session_name,
                host_participant_id: host_id.clone(),
                host_display_name: host_name,
                local_participant_id: host_id,
                local_role: ParticipantRole::Host,
                engine,
                last_sync_error: None,
            });
        }
        if let Ok(mut applied) = self.applied_event_ids.lock() {
            applied.clear();
        }

        let snap = self
            .get_snapshot()
            .await?
            .ok_or_else(|| "Created connect session disappeared".to_string())?;
        self.emit_snapshot(&snap);
        Ok(snap)
    }

    pub async fn join_session(
        &self,
        join_code: &str,
        participant_id: Option<&str>,
        display_name: &str,
    ) -> Result<ConnectSessionSnapshot, String> {
        let code = join_code.trim().to_ascii_uppercase();
        if code.is_empty() {
            return Err("Join code is required".to_string());
        }

        let default_pid = self.local_participant_id().await?;
        let pid = participant_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&default_pid)
            .to_string();
        let guest_name = if display_name.trim().is_empty() {
            "Guest".to_string()
        } else {
            display_name.trim().to_string()
        };

        // Check active local session first (supports local multi-participant sessions & reconnects).
        let matches_active = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(sess) = guard.as_mut()
                && sess.join_code.eq_ignore_ascii_case(&code)
            {
                let role = if pid == sess.host_participant_id {
                    ParticipantRole::Host
                } else {
                    ParticipantRole::Guest
                };
                sess.engine
                    .upsert_participant(&pid, &guest_name, role, true);
                Some((sess.session_id.clone(), role))
            } else {
                None
            }
        };

        if let Some((session_id, role)) = matches_active {
            let role_str = match role {
                ParticipantRole::Host => "host",
                ParticipantRole::Guest => "guest",
            };
            sqlx::query(
                r#"
                INSERT INTO connect_participants (session_id, participant_id, display_name, role, is_connected, penalty_credits)
                VALUES (?, ?, ?, ?, 1, 0.0)
                ON CONFLICT(session_id, participant_id) DO UPDATE SET
                    display_name = excluded.display_name,
                    is_connected = 1,
                    last_seen_at = CURRENT_TIMESTAMP
                "#,
            )
            .bind(&session_id)
            .bind(&pid)
            .bind(&guest_name)
            .bind(role_str)
            .execute(&self.db)
            .await
            .map_err(|e| format!("Failed to persist participant rejoin: {e}"))?;

            let snap = self
                .get_snapshot()
                .await?
                .ok_or_else(|| "Session unavailable".to_string())?;
            self.emit_snapshot(&snap);
            return Ok(snap);
        }

        // Check persisted local SQLite session by join code.
        let found_local: Option<String> = sqlx::query_scalar(
            "SELECT session_id FROM connect_sessions WHERE UPPER(join_code) = ? LIMIT 1",
        )
        .bind(&code)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| format!("Failed to search connect sessions: {e}"))?;

        if let Some(session_id) = found_local {
            sqlx::query("UPDATE connect_sessions SET is_active = CASE WHEN session_id = ? THEN 1 ELSE 0 END")
                .bind(&session_id)
                .execute(&self.db)
                .await
                .map_err(|e| format!("Failed to activate connect session: {e}"))?;
            sqlx::query(
                r#"
                INSERT INTO connect_participants (session_id, participant_id, display_name, role, is_connected, penalty_credits)
                VALUES (?, ?, ?, 'guest', 1, 0.0)
                ON CONFLICT(session_id, participant_id) DO UPDATE SET
                    display_name = excluded.display_name,
                    is_connected = 1,
                    last_seen_at = CURRENT_TIMESTAMP
                "#,
            )
            .bind(&session_id)
            .bind(&pid)
            .bind(&guest_name)
            .execute(&self.db)
            .await
            .map_err(|e| format!("Failed to join session: {e}"))?;
            return self
                .restore_active_session()
                .await?
                .ok_or_else(|| "Session not found after restore".to_string());
        }

        // Attempt remote Supabase lookup if configured & signed in.
        if let Some(sync) = &self.sync_engine
            && let Ok(status) = sync.get_status().await
            && status.authenticated
            && status.backend_configured
        {
            return Err(format!(
                "Remote Supabase Cavia Connect lookup for join code {code} requires cloud session record (none returned from Supabase)."
            ));
        }

        Err(format!(
            "No Cavia Connect session found for code \"{code}\""
        ))
    }

    pub async fn set_participant_connected(
        &self,
        participant_id: &str,
        connected: bool,
    ) -> Result<ConnectSessionSnapshot, String> {
        let session_id = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let sess = guard
                .as_mut()
                .ok_or_else(|| "No active Cavia Connect session".to_string())?;
            sess.engine
                .set_participant_connected(participant_id, connected);
            sess.session_id.clone()
        };

        sqlx::query(
            "UPDATE connect_participants SET is_connected = ?, last_seen_at = CURRENT_TIMESTAMP WHERE session_id = ? AND participant_id = ?",
        )
        .bind(if connected { 1 } else { 0 })
        .bind(&session_id)
        .bind(participant_id)
        .execute(&self.db)
        .await
        .map_err(|e| format!("Failed to update participant presence: {e}"))?;

        let snap = self
            .get_snapshot()
            .await?
            .ok_or_else(|| "No active Cavia Connect session".to_string())?;
        self.emit_snapshot(&snap);
        Ok(snap)
    }

    /// Contributes a track cleanly using fairness weights, canonical dedupe, and late-preload
    /// gating when the rolling preload window is already populated.
    pub async fn contribute_track(
        &self,
        contributor_id: Option<&str>,
        contributor_name: Option<&str>,
        role_override: Option<ParticipantRole>,
        track: Track,
        priority_override: bool,
    ) -> Result<ConnectContributionResult, String> {
        let event_id = format!("evt-{}", Uuid::new_v4());
        self.contribute_track_with_event(
            contributor_id,
            contributor_name,
            role_override,
            track,
            priority_override,
            &event_id,
        )
        .await
    }

    pub async fn contribute_track_with_event(
        &self,
        contributor_id: Option<&str>,
        contributor_name: Option<&str>,
        role_override: Option<ParticipantRole>,
        track: Track,
        priority_override: bool,
        event_id: &str,
    ) -> Result<ConnectContributionResult, String> {
        if self
            .applied_event_ids
            .lock()
            .map(|set| set.contains(event_id))
            .unwrap_or(false)
            && let Some(snap) = self.get_snapshot().await?
            && let Some(existing) = snap
                .shared_pool
                .iter()
                .find(|e| e.canonical_provider_id == canonical_provider_id(&track))
                .cloned()
        {
            return Ok(ConnectContributionResult {
                entry: existing,
                snapshot: snap,
            });
        }

        let preload_target = self.current_preload_target();
        let cached_ready = self.is_track_cached_locally(&track);

        let (entry, session_id, revision, participant_penalty) = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let sess = guard
                .as_mut()
                .ok_or_else(|| "No active Cavia Connect session".to_string())?;

            let pid = contributor_id
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(&sess.local_participant_id)
                .to_string();
            let role = role_override.unwrap_or_else(|| {
                if pid == sess.host_participant_id {
                    ParticipantRole::Host
                } else {
                    ParticipantRole::Guest
                }
            });
            let pname = contributor_name
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| {
                    sess.engine
                        .participant(&pid)
                        .map(|p| p.display_name.clone())
                })
                .unwrap_or_else(|| {
                    if role == ParticipantRole::Host {
                        sess.host_display_name.clone()
                    } else {
                        "Guest".to_string()
                    }
                });

            // Determine whether this is a late online contribution while the preload window is
            // already populated: uncached streaming contributions wait in `PendingPreload` until
            // sequentially downloaded & verified before entering the upcoming playback sequence.
            let current_upcoming_len = sess.engine.compute_upcoming_sequence().len();
            let requires_pending_preload = preload_target > 0
                && !cached_ready
                && extract_ytmusic_video_id(&track).is_some()
                && current_upcoming_len >= preload_target;

            let initial_status = if requires_pending_preload {
                QueueEntryStatus::PendingPreload
            } else {
                QueueEntryStatus::Ready
            };

            let entry = sess.engine.contribute_track(
                &pid,
                &pname,
                role,
                track,
                priority_override,
                initial_status,
                None,
            )?;

            // Commit index 0 immediately when playback isn't committed yet so subsequent arrivals
            // never replace the committed next song.
            if initial_status == QueueEntryStatus::Ready && current_upcoming_len == 0 {
                sess.engine.commit_next_and_window(1);
            }

            let penalty = sess
                .engine
                .participant(&pid)
                .map(|p| p.penalty_credits)
                .unwrap_or(0.0);
            (
                entry,
                sess.session_id.clone(),
                sess.engine.queue_revision,
                penalty,
            )
        };

        if let Ok(mut applied) = self.applied_event_ids.lock() {
            applied.insert(event_id.to_string());
        }
        self.persist_entry_and_event(&session_id, &entry, revision, participant_penalty, event_id)
            .await?;
        self.sync_preload_coordinator();

        let snap = self
            .get_snapshot()
            .await?
            .ok_or_else(|| "Active connect session missing".to_string())?;
        self.emit_snapshot(&snap);
        Ok(ConnectContributionResult {
            entry,
            snapshot: snap,
        })
    }

    pub async fn contribute_playlist_tracks(
        &self,
        contributor_id: Option<&str>,
        contributor_name: Option<&str>,
        role_override: Option<ParticipantRole>,
        tracks: Vec<Track>,
    ) -> Result<ConnectPlaylistContributionResult, String> {
        let mut accepted_count = 0_usize;
        let mut rejected_duplicates = 0_usize;
        let mut rejected_unsupported = 0_usize;
        let mut pending_preload_count = 0_usize;

        for track in tracks {
            match self
                .contribute_track(
                    contributor_id,
                    contributor_name,
                    role_override,
                    track,
                    false,
                )
                .await
            {
                Ok(res) => {
                    accepted_count += 1;
                    if res.entry.status == QueueEntryStatus::PendingPreload {
                        pending_preload_count += 1;
                    }
                }
                Err(err) if err.contains("already in the collaborative queue") => {
                    rejected_duplicates += 1;
                }
                Err(_) => {
                    rejected_unsupported += 1;
                }
            }
        }

        let snapshot = self
            .get_snapshot()
            .await?
            .ok_or_else(|| "No active Cavia Connect session".to_string())?;
        Ok(ConnectPlaylistContributionResult {
            accepted_count,
            rejected_duplicates,
            rejected_unsupported,
            pending_preload_count,
            snapshot,
        })
    }

    /// Marks a downloaded & verified late entry as ready and splices it into the upcoming sequence.
    pub async fn promote_pending_entry_ready(
        &self,
        queue_entry_id: &str,
    ) -> Result<Option<ConnectSessionSnapshot>, String> {
        let updated = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let Some(sess) = guard.as_mut() else {
                return Ok(None);
            };
            let promoted = sess.engine.mark_entry_ready(queue_entry_id)?;
            if !promoted {
                return Ok(None);
            }
            let seq = sess.engine.compute_upcoming_sequence();
            (sess.session_id.clone(), sess.engine.queue_revision, seq)
        };

        let (session_id, revision, seq) = updated;
        self.persist_upcoming_ranks(&session_id, revision, &seq)
            .await?;
        self.sync_preload_coordinator();
        let snap = self.get_snapshot().await?;
        if let Some(s) = &snap {
            self.emit_snapshot(s);
        }
        Ok(snap)
    }

    pub async fn remove_entry(
        &self,
        queue_entry_id: &str,
        apply_removal_penalty: bool,
    ) -> Result<ConnectSessionSnapshot, String> {
        let (session_id, revision, contributor_id, penalty) = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let sess = guard
                .as_mut()
                .ok_or_else(|| "No active Cavia Connect session".to_string())?;
            let contributor_id = sess
                .engine
                .shared_pool()
                .into_iter()
                .find(|e| e.queue_entry_id == queue_entry_id)
                .map(|e| e.contributor_id);
            if !sess
                .engine
                .remove_entry(queue_entry_id, apply_removal_penalty)
            {
                return Err("Queue entry was not active or not found".to_string());
            }
            let cid = contributor_id.unwrap_or_default();
            let penalty = sess
                .engine
                .participant(&cid)
                .map(|p| p.penalty_credits)
                .unwrap_or(0.0);
            (
                sess.session_id.clone(),
                sess.engine.queue_revision,
                cid,
                penalty,
            )
        };

        sqlx::query(
            "UPDATE connect_queue_entries SET status = 'removed', updated_at = CURRENT_TIMESTAMP WHERE queue_entry_id = ?",
        )
        .bind(queue_entry_id)
        .execute(&self.db)
        .await
        .map_err(|e| format!("Failed to persist removed entry: {e}"))?;

        if !contributor_id.is_empty() {
            sqlx::query(
                "UPDATE connect_participants SET penalty_credits = ? WHERE session_id = ? AND participant_id = ?",
            )
            .bind(penalty)
            .bind(&session_id)
            .bind(&contributor_id)
            .execute(&self.db)
            .await
            .map_err(|e| format!("Failed to persist participant penalty: {e}"))?;
        }

        sqlx::query(
            "UPDATE connect_sessions SET queue_revision = ?, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
        )
        .bind(revision as i64)
        .bind(&session_id)
        .execute(&self.db)
        .await
        .map_err(|e| format!("Failed to update connect session revision: {e}"))?;

        self.sync_preload_coordinator();
        let snap = self
            .get_snapshot()
            .await?
            .ok_or_else(|| "No active connect session".to_string())?;
        self.emit_snapshot(&snap);
        Ok(snap)
    }

    pub async fn host_reorder_upcoming(
        &self,
        from_index: usize,
        to_index: usize,
    ) -> Result<ConnectSessionSnapshot, String> {
        let (session_id, revision, seq) = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let sess = guard
                .as_mut()
                .ok_or_else(|| "No active Cavia Connect session".to_string())?;
            if sess.local_role != ParticipantRole::Host {
                return Err("Only the session host can reorder the collaborative queue".to_string());
            }
            if !sess.engine.host_reorder_upcoming(from_index, to_index) {
                return Err("Reorder index out of bounds".to_string());
            }
            (
                sess.session_id.clone(),
                sess.engine.queue_revision,
                sess.engine.compute_upcoming_sequence(),
            )
        };

        self.persist_upcoming_ranks(&session_id, revision, &seq)
            .await?;
        self.sync_preload_coordinator();
        let snap = self
            .get_snapshot()
            .await?
            .ok_or_else(|| "No active connect session".to_string())?;
        self.emit_snapshot(&snap);
        Ok(snap)
    }

    pub async fn set_fairness_alpha(&self, alpha: f64) -> Result<ConnectSessionSnapshot, String> {
        let (session_id, revision, clamped) = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let sess = guard
                .as_mut()
                .ok_or_else(|| "No active Cavia Connect session".to_string())?;
            if sess.local_role != ParticipantRole::Host {
                return Err("Only the session host can change fairness weighting".to_string());
            }
            sess.engine.set_alpha(alpha);
            (
                sess.session_id.clone(),
                sess.engine.queue_revision,
                sess.engine.config.alpha,
            )
        };

        sqlx::query(
            "UPDATE connect_sessions SET fairness_alpha = ?, queue_revision = ?, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
        )
        .bind(clamped)
        .bind(revision as i64)
        .bind(&session_id)
        .execute(&self.db)
        .await
        .map_err(|e| format!("Failed to persist fairness alpha: {e}"))?;

        self.sync_preload_coordinator();
        let snap = self
            .get_snapshot()
            .await?
            .ok_or_else(|| "No active connect session".to_string())?;
        self.emit_snapshot(&snap);
        Ok(snap)
    }

    /// Advances playback authoritatively on host, retiring current song and committing next.
    pub async fn advance_playback(&self) -> Result<Option<Track>, String> {
        let next_state = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let Some(sess) = guard.as_mut() else {
                return Ok(None);
            };
            let current_before = sess.engine.current_entry().map(|e| e.queue_entry_id);
            let next = sess.engine.advance_to_next();
            Some((
                sess.session_id.clone(),
                sess.engine.queue_revision,
                current_before,
                next,
                sess.engine.compute_upcoming_sequence(),
            ))
        };

        let Some((session_id, revision, prev_id, next_opt, seq)) = next_state else {
            return Ok(None);
        };

        if let Some(pid) = prev_id {
            let _ = sqlx::query(
                "UPDATE connect_queue_entries SET status = 'played', updated_at = CURRENT_TIMESTAMP WHERE queue_entry_id = ?",
            )
            .bind(pid)
            .execute(&self.db)
            .await;
        }
        if let Some(next_entry) = &next_opt {
            let _ = sqlx::query(
                "UPDATE connect_queue_entries SET status = 'playing', committed_rank = NULL, manual_position = NULL, updated_at = CURRENT_TIMESTAMP WHERE queue_entry_id = ?",
            )
            .bind(&next_entry.queue_entry_id)
            .execute(&self.db)
            .await;
        }
        self.persist_upcoming_ranks(&session_id, revision, &seq)
            .await?;
        self.sync_preload_coordinator();

        if let Some(snap) = self.get_snapshot().await? {
            self.emit_snapshot(&snap);
        }
        Ok(next_opt.map(|e| attach_entry_metadata(&e.track, &e)))
    }

    pub async fn sync_pending_events(&self) -> Result<ConnectSyncResult, String> {
        let (session_id, revision) = {
            let guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let Some(sess) = guard.as_ref() else {
                return Ok(ConnectSyncResult {
                    pushed_events: 0,
                    pulled_events: 0,
                    skipped_duplicates: 0,
                    revision: 0,
                    offline: false,
                    error_message: None,
                });
            };
            (sess.session_id.clone(), sess.engine.queue_revision)
        };

        let pending_rows = sqlx::query(
            r#"
            SELECT event_id, participant_id, action, payload_json, base_revision
            FROM connect_contribution_events
            WHERE session_id = ? AND synced = 0
            ORDER BY created_at ASC
            "#,
        )
        .bind(&session_id)
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("Failed to inspect connect outbox: {e}"))?;

        if pending_rows.is_empty() {
            return Ok(ConnectSyncResult {
                pushed_events: 0,
                pulled_events: 0,
                skipped_duplicates: 0,
                revision,
                offline: false,
                error_message: None,
            });
        }

        // Check whether remote cloud sync is configured & authenticated.
        let mut pushed_events = 0_usize;
        if let Some(sync) = &self.sync_engine
            && let Ok(status) = sync.get_status().await
            && status.authenticated
            && status.backend_configured
        {
            let event_ids: Vec<String> = pending_rows.iter().map(|r| r.get("event_id")).collect();
            match self
                .push_outbox_to_supabase(sync, &session_id, revision, &pending_rows)
                .await
            {
                Ok(count) => {
                    pushed_events = count;
                    for eid in event_ids {
                        let _ = sqlx::query(
                            "UPDATE connect_contribution_events SET synced = 1 WHERE event_id = ?",
                        )
                        .bind(eid)
                        .execute(&self.db)
                        .await;
                    }
                }
                Err(err) => {
                    let offline = is_connectivity_error(&err);
                    if let Ok(mut guard) = self.session.lock()
                        && let Some(sess) = guard.as_mut()
                    {
                        sess.last_sync_error = Some(err.clone());
                    }
                    return Ok(ConnectSyncResult {
                        pushed_events: 0,
                        pulled_events: 0,
                        skipped_duplicates: 0,
                        revision,
                        offline,
                        error_message: Some(err),
                    });
                }
            }
        }

        Ok(ConnectSyncResult {
            pushed_events,
            pulled_events: 0,
            skipped_duplicates: 0,
            revision,
            offline: false,
            error_message: None,
        })
    }

    async fn push_outbox_to_supabase(
        &self,
        sync: &SyncEngine,
        session_id: &str,
        revision: u64,
        rows: &[sqlx::sqlite::SqliteRow],
    ) -> Result<usize, String> {
        let Some((base_url, anon_key, access_token)) = sync.connect_auth_context().await? else {
            return Err("Supabase authentication expired".to_string());
        };
        let rest = crate::services::sync::client::SupabaseRestClient::new();
        let payload: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                let payload_str: String = r.get("payload_json");
                let parsed: serde_json::Value =
                    serde_json::from_str(&payload_str).unwrap_or_else(|_| json!({}));
                json!({
                    "event_id": r.get::<String, _>("event_id"),
                    "session_id": session_id,
                    "participant_id": r.get::<String, _>("participant_id"),
                    "action": r.get::<String, _>("action"),
                    "payload_json": parsed,
                    "base_revision": r.get::<i64, _>("base_revision"),
                })
            })
            .collect();

        rest.upsert_table_rows(
            &base_url,
            &anon_key,
            &access_token,
            "connect_events",
            &json!(payload),
            Some("event_id"),
        )
        .await?;

        let _ = rest
            .upsert_table_rows(
                &base_url,
                &anon_key,
                &access_token,
                "connect_sessions",
                &json!([{
                    "session_id": session_id,
                    "queue_revision": revision as i64,
                }]),
                Some("session_id"),
            )
            .await;

        Ok(rows.len())
    }

    pub async fn end_session(&self) -> Result<(), String> {
        let session_id = {
            let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            guard.take().map(|s| s.session_id)
        };
        if let Some(sid) = session_id {
            sqlx::query(
                "UPDATE connect_sessions SET is_active = 0, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
            )
            .bind(sid)
            .execute(&self.db)
            .await
            .map_err(|e| format!("Failed to close connect session: {e}"))?;
        }
        if let Some(app) = &self.app_handle {
            let _ = app.emit(
                "connect:state-changed",
                &Option::<ConnectSessionSnapshot>::None,
            );
        }
        Ok(())
    }

    pub fn upcoming_playback_tracks(&self) -> Option<(Option<Track>, Vec<Track>)> {
        let guard = self.session.lock().ok()?;
        let sess = guard.as_ref()?;
        let cur = sess
            .engine
            .current_entry()
            .map(|e| attach_entry_metadata(&e.track, &e));
        let upcoming = sess
            .engine
            .compute_upcoming_sequence()
            .into_iter()
            .map(|e| attach_entry_metadata(&e.track, &e))
            .collect();
        Some((cur, upcoming))
    }

    pub fn is_active(&self) -> bool {
        self.session.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    pub async fn get_snapshot(&self) -> Result<Option<ConnectSessionSnapshot>, String> {
        let preload_target = self.current_preload_target();
        let (sid, partial) = {
            let guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let Some(sess) = guard.as_ref() else {
                return Ok(None);
            };
            let view = sess.engine.build_view(preload_target);
            (
                sess.session_id.clone(),
                ConnectSessionSnapshot {
                    active: true,
                    session_id: sess.session_id.clone(),
                    join_code: sess.join_code.clone(),
                    name: sess.name.clone(),
                    local_participant_id: sess.local_participant_id.clone(),
                    local_role: sess.local_role,
                    host_participant_id: sess.host_participant_id.clone(),
                    host_display_name: sess.host_display_name.clone(),
                    fairness_alpha: sess.engine.config.alpha,
                    preload_target,
                    queue_revision: sess.engine.queue_revision,
                    current_entry: view.current_entry,
                    shared_pool: view.shared_pool,
                    upcoming_sequence: view.upcoming_sequence,
                    preload_window: view.preload_window,
                    pending_preload: view.pending_preload,
                    weights: view.weights,
                    unsynced_event_count: 0,
                    last_sync_error: sess.last_sync_error.clone(),
                },
            )
        };

        let unsynced: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM connect_contribution_events WHERE session_id = ? AND synced = 0",
        )
        .bind(&sid)
        .fetch_one(&self.db)
        .await
        .unwrap_or(0);

        let mut full = partial;
        full.unsynced_event_count = unsynced.max(0) as usize;
        Ok(Some(full))
    }

    fn sync_preload_coordinator(&self) {
        let preload_opt = self.preload.lock().ok().and_then(|g| g.clone());
        let Some(preload) = preload_opt else {
            return;
        };
        let target = preload.snapshot().target_count;
        let payload = {
            let guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
            guard.as_ref().map(|sess| {
                let view = sess.engine.build_view(target);
                let cur = view
                    .current_entry
                    .map(|e| attach_entry_metadata(&e.track, &e));
                let upcoming: Vec<Track> = view
                    .upcoming_sequence
                    .into_iter()
                    .map(|e| attach_entry_metadata(&e.track, &e))
                    .collect();
                let late: Vec<Track> = view
                    .pending_preload
                    .into_iter()
                    .map(|e| attach_entry_metadata(&e.track, &e))
                    .collect();
                (cur, upcoming, late)
            })
        };
        if let Some((cur, upcoming, late)) = payload {
            preload.update_window(cur, upcoming, late);
        }
    }

    fn emit_snapshot(&self, snap: &ConnectSessionSnapshot) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit("connect:state-changed", snap);
        }
    }

    async fn persist_entry_and_event(
        &self,
        session_id: &str,
        entry: &SharedQueueEntry,
        revision: u64,
        participant_penalty: f64,
        event_id: &str,
    ) -> Result<(), String> {
        let track_json = serde_json::to_string(&entry.track)
            .map_err(|e| format!("Failed to serialize connect track: {e}"))?;
        let status_str = match entry.status {
            QueueEntryStatus::PendingPreload => "pending_preload",
            QueueEntryStatus::Ready => "ready",
            QueueEntryStatus::Playing => "playing",
            QueueEntryStatus::Played => "played",
            QueueEntryStatus::Removed => "removed",
        };
        let role_str = match entry.role {
            ParticipantRole::Host => "host",
            ParticipantRole::Guest => "guest",
        };

        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| format!("Failed to begin connect entry tx: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO connect_participants (session_id, participant_id, display_name, role, is_connected, penalty_credits)
            VALUES (?, ?, ?, ?, 1, ?)
            ON CONFLICT(session_id, participant_id) DO UPDATE SET
                display_name = excluded.display_name,
                is_connected = 1,
                penalty_credits = excluded.penalty_credits,
                last_seen_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(session_id)
        .bind(&entry.contributor_id)
        .bind(&entry.contributor_name)
        .bind(role_str)
        .bind(participant_penalty)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to upsert connect participant: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO connect_queue_entries (
                queue_entry_id, session_id, track_json, track_id, provider_id,
                canonical_provider_id, contributor_id, contributor_name, status,
                priority_override, manual_position, committed_rank, created_at_seq
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&entry.queue_entry_id)
        .bind(session_id)
        .bind(&track_json)
        .bind(&entry.track.id)
        .bind(&entry.track.provider_id)
        .bind(&entry.canonical_provider_id)
        .bind(&entry.contributor_id)
        .bind(&entry.contributor_name)
        .bind(status_str)
        .bind(if entry.priority_override { 1 } else { 0 })
        .bind(entry.manual_position.map(|v| v as i64))
        .bind(entry.committed_rank.map(|v| v as i64))
        .bind(entry.created_at_seq as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to persist connect queue entry: {e}"))?;

        let payload_json = json!({
            "queueEntryId": entry.queue_entry_id,
            "track": entry.track,
            "priorityOverride": entry.priority_override,
        })
        .to_string();

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO connect_contribution_events (
                event_id, session_id, participant_id, canonical_provider_id,
                action, payload_json, base_revision, synced
            ) VALUES (?, ?, ?, ?, 'add', ?, ?, 0)
            "#,
        )
        .bind(event_id)
        .bind(session_id)
        .bind(&entry.contributor_id)
        .bind(&entry.canonical_provider_id)
        .bind(payload_json)
        .bind(revision as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to persist connect contribution event: {e}"))?;

        sqlx::query(
            "UPDATE connect_sessions SET queue_revision = ?, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
        )
        .bind(revision as i64)
        .bind(session_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update connect session revision: {e}"))?;

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit connect entry tx: {e}"))?;

        Ok(())
    }

    async fn persist_upcoming_ranks(
        &self,
        session_id: &str,
        revision: u64,
        upcoming: &[SharedQueueEntry],
    ) -> Result<(), String> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| format!("Failed to begin connect reorder tx: {e}"))?;
        for entry in upcoming {
            sqlx::query(
                r#"
                UPDATE connect_queue_entries
                SET status = 'ready',
                    manual_position = ?,
                    committed_rank = ?,
                    updated_at = CURRENT_TIMESTAMP
                WHERE queue_entry_id = ?
                "#,
            )
            .bind(entry.manual_position.map(|v| v as i64))
            .bind(entry.committed_rank.map(|v| v as i64))
            .bind(&entry.queue_entry_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to update connect rank: {e}"))?;
        }
        sqlx::query(
            "UPDATE connect_sessions SET queue_revision = ?, updated_at = CURRENT_TIMESTAMP WHERE session_id = ?",
        )
        .bind(revision as i64)
        .bind(session_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update session revision: {e}"))?;
        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit connect rank tx: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn test_db() -> SqlitePool {
        let opts = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn yt(vid: &str, title: &str) -> Track {
        Track {
            id: format!("ytmusic:{vid}"),
            title: title.to_string(),
            artist: "Connect Artist".into(),
            album: "Connect Album".into(),
            album_artist: None,
            duration_ms: 180_000,
            artwork_uri: None,
            source_kind: "provider".into(),
            provider_id: Some("youtube_music".into()),
            file_uri: None,
            release_year: None,
            track_number: None,
            disc_number: None,
            genre: None,
            is_liked: false,
            play_count: 0,
            date_added: String::new(),
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        }
    }

    #[tokio::test]
    async fn host_authoritative_session_persists_across_restart_and_prevents_guest_spam() {
        let pool = test_db().await;
        let connect = ConnectService::new(pool.clone(), None, None, None);

        let session = connect
            .create_session("Friday Lounge", "HostDJ", Some(1.0))
            .await
            .unwrap();
        assert!(session.join_code.starts_with("CAVIA-"));

        connect
            .join_session(&session.join_code, Some("guest-alice"), "Alice")
            .await
            .unwrap();

        // Guest playlist import counts each track individually and rejects non-ytmusic tracks.
        let mut local_track = yt("x1", "Local Disallowed");
        local_track.source_kind = "local".into();
        local_track.provider_id = None;
        local_track.id = "local-1".into();

        let pl_res = connect
            .contribute_playlist_tracks(
                Some("guest-alice"),
                Some("Alice"),
                Some(ParticipantRole::Guest),
                vec![
                    yt("gVid1", "One"),
                    yt("gVid1", "One Duplicate"),
                    local_track,
                    yt("gVid2", "Two"),
                ],
            )
            .await
            .unwrap();
        assert_eq!(pl_res.accepted_count, 2);
        assert_eq!(pl_res.rejected_duplicates, 1);
        assert_eq!(pl_res.rejected_unsupported, 1);

        // Remove-and-readd retains penalty and survives full ConnectService restart.
        let first_id = pl_res.snapshot.shared_pool[0].queue_entry_id.clone();
        connect.remove_entry(&first_id, true).await.unwrap();

        let connect_restarted = ConnectService::new(pool.clone(), None, None, None);
        let restored = connect_restarted
            .restore_active_session()
            .await
            .unwrap()
            .expect("Active session must restore from SQLite");
        let alice_weight = restored
            .weights
            .iter()
            .find(|w| w.participant_id == "guest-alice")
            .unwrap();
        assert!(alice_weight.penalty_credits >= 1.0);
        assert_eq!(restored.unsynced_event_count, 2);
    }

    #[tokio::test]
    async fn idempotent_event_id_prevents_duplicate_replay_on_reconnect() {
        let pool = test_db().await;
        let connect = ConnectService::new(pool, None, None, None);
        connect
            .create_session("Sync Test", "Host", None)
            .await
            .unwrap();

        let first = connect
            .contribute_track_with_event(
                Some("guest-1"),
                Some("Guest 1"),
                Some(ParticipantRole::Guest),
                yt("syncVid1", "Sync Song"),
                false,
                "evt-fixed-123",
            )
            .await
            .unwrap();
        let replay = connect
            .contribute_track_with_event(
                Some("guest-1"),
                Some("Guest 1"),
                Some(ParticipantRole::Guest),
                yt("syncVid1", "Sync Song"),
                false,
                "evt-fixed-123",
            )
            .await
            .unwrap();

        assert_eq!(first.entry.queue_entry_id, replay.entry.queue_entry_id);
        assert_eq!(replay.snapshot.shared_pool.len(), 1);
    }

    #[tokio::test]
    async fn late_uncached_guest_contribution_waits_for_verified_preload_before_entering_sequence()
    {
        let pool = test_db().await;
        let cache_dir =
            std::env::temp_dir().join(format!("cavia-conn-late-{}", uuid::Uuid::new_v4()));
        let ytmusic =
            Arc::new(crate::services::ytmusic::YtMusicService::new(cache_dir.clone()).unwrap());
        let wav = minimal_test_wav();
        // Pre-populate 10 verified cached tracks so preload window (target=10) is full.
        for i in 0..10 {
            std::fs::write(cache_dir.join(format!("readyVid{i:02}.m4a")), &wav).unwrap();
        }

        let late_promoted = Arc::new(Mutex::new(Vec::<String>::new()));
        let promoted_cb = Arc::clone(&late_promoted);
        let preload = PreloadCoordinator::new(
            ytmusic.clone(),
            None,
            10,
            crate::services::preload::PreloadCallbacks {
                on_status_changed: Arc::new(|_| {}),
                on_late_verified: Arc::new(move |id, _p| {
                    promoted_cb.lock().unwrap().push(id);
                }),
                on_connectivity_lost: Arc::new(|_| {}),
            },
        );

        let connect = ConnectService::new(pool, None, Some(ytmusic), None);
        connect.attach_preload(preload);
        connect
            .create_session("Late Test", "Host", Some(1.0))
            .await
            .unwrap();

        for i in 0..10 {
            connect
                .contribute_track(
                    Some("heavy"),
                    Some("Heavy"),
                    Some(ParticipantRole::Guest),
                    yt(&format!("readyVid{i:02}"), &format!("Ready {i}")),
                    false,
                )
                .await
                .unwrap();
        }

        let before = connect.get_snapshot().await.unwrap().unwrap();
        assert_eq!(before.upcoming_sequence.len(), 10);
        let committed_next_id = before.upcoming_sequence[0].queue_entry_id.clone();

        // Newcomer contributes uncached song while 10 tracks are already in preload window ->
        // must enter PendingPreload and NOT appear in upcomingSequence yet!
        let late_res = connect
            .contribute_track(
                Some("newcomer"),
                Some("Newcomer"),
                Some(ParticipantRole::Guest),
                yt("uncached99", "Late Newcomer"),
                false,
            )
            .await
            .unwrap();
        assert_eq!(late_res.entry.status, QueueEntryStatus::PendingPreload);
        assert!(
            !late_res
                .snapshot
                .upcoming_sequence
                .iter()
                .any(|e| e.queue_entry_id == late_res.entry.queue_entry_id)
        );
        assert_eq!(late_res.snapshot.pending_preload.len(), 1);

        // Once verified & promoted, it enters the upcoming window fairly after committed[0].
        let promoted = connect
            .promote_pending_entry_ready(&late_res.entry.queue_entry_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            promoted.upcoming_sequence[0].queue_entry_id,
            committed_next_id
        );
        let pos = promoted
            .upcoming_sequence
            .iter()
            .position(|e| e.queue_entry_id == late_res.entry.queue_entry_id)
            .unwrap();
        assert!((1..10).contains(&pos));
        assert!(promoted.pending_preload.is_empty());

        let _ = std::fs::remove_dir_all(cache_dir);
    }

    fn minimal_test_wav() -> Vec<u8> {
        let mut bytes = Vec::with_capacity(44 + 64);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36u32 + 64).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&44100u32.to_le_bytes());
        bytes.extend_from_slice(&(44100u32 * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&64u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 64]);
        bytes
    }
}
