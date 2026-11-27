use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use crate::domain::Track;
use crate::services::preload::{PreloadCoordinator, extract_ytmusic_video_id};
use crate::services::queue::SavedQueueSnapshot;
use crate::services::ytmusic::YtMusicService;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineFallbackStatus {
    pub is_offline: bool,
    pub outage_id: u64,
    pub prompt_visible: bool,
    pub prompt_message: Option<String>,
    pub fallback_active: bool,
    pub fallback_exhausted: bool,
    pub eligible_cached_count: usize,
    pub played_fallback_count: usize,
    pub remaining_fallback_count: usize,
    pub reconnect_pending_finish: bool,
    pub status_message: Option<String>,
}

struct OfflineInner {
    is_offline: bool,
    outage_id: u64,
    prompt_visible: bool,
    declined_outage_id: Option<u64>,
    prompt_message: Option<String>,
    fallback_active: bool,
    fallback_exhausted: bool,
    saved_original: Option<SavedQueueSnapshot>,
    fallback_pool_ids: Vec<String>,
    played_fallback_entry_ids: HashSet<String>,
    played_fallback_track_ids: HashSet<String>,
    reconnect_pending_finish: bool,
    status_message: Option<String>,
}

pub struct OfflineFallbackService {
    db: SqlitePool,
    ytmusic: Arc<YtMusicService>,
    app_handle: Option<AppHandle>,
    inner: Mutex<OfflineInner>,
}

impl OfflineFallbackService {
    pub fn new(
        db: SqlitePool,
        ytmusic: Arc<YtMusicService>,
        app_handle: Option<AppHandle>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            ytmusic,
            app_handle,
            inner: Mutex::new(OfflineInner {
                is_offline: false,
                outage_id: 0,
                prompt_visible: false,
                declined_outage_id: None,
                prompt_message: None,
                fallback_active: false,
                fallback_exhausted: false,
                saved_original: None,
                fallback_pool_ids: Vec::new(),
                played_fallback_entry_ids: HashSet::new(),
                played_fallback_track_ids: HashSet::new(),
                reconnect_pending_finish: false,
                status_message: None,
            }),
        })
    }

    pub fn snapshot(&self) -> OfflineFallbackStatus {
        let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let played_count = guard.played_fallback_track_ids.len();
        let remaining = guard
            .fallback_pool_ids
            .iter()
            .filter(|id| !guard.played_fallback_track_ids.contains(*id))
            .count();

        OfflineFallbackStatus {
            is_offline: guard.is_offline,
            outage_id: guard.outage_id,
            prompt_visible: guard.prompt_visible,
            prompt_message: guard.prompt_message.clone(),
            fallback_active: guard.fallback_active,
            fallback_exhausted: guard.fallback_exhausted,
            eligible_cached_count: guard.fallback_pool_ids.len(),
            played_fallback_count: played_count,
            remaining_fallback_count: remaining,
            reconnect_pending_finish: guard.reconnect_pending_finish,
            status_message: guard.status_message.clone(),
        }
    }

    fn emit_status(&self) {
        let snap = self.snapshot();
        if let Some(app) = &self.app_handle {
            let _ = app.emit("playback:offline-changed", &snap);
        }
    }

    /// Triggers outage detection and opens host fallback prompt AT MOST ONCE per outage.
    pub async fn notify_connectivity_lost(
        &self,
        current_queue_tracks: &[Track],
    ) -> OfflineFallbackStatus {
        let eligible_count = self
            .build_verified_offline_pool(current_queue_tracks, 0)
            .await
            .len();

        let should_emit = {
            let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let mut changed = false;
            if !guard.is_offline {
                guard.is_offline = true;
                guard.outage_id = guard.outage_id.saturating_add(1);
                changed = true;
            }
            guard.reconnect_pending_finish = false;
            if guard.fallback_pool_ids.is_empty() && eligible_count > 0 {
                // Keep count fresh in status even before user activates fallback.
                guard.status_message = Some(format!(
                    "{eligible_count} verified offline tracks available in cache and local library."
                ));
            }
            if !guard.fallback_active
                && guard.declined_outage_id != Some(guard.outage_id)
                && !guard.prompt_visible
            {
                guard.prompt_visible = true;
                guard.prompt_message = Some(
                    "Internet is unavailable. Would you like to continue with cached songs?"
                        .to_string(),
                );
                changed = true;
            }
            changed
        };

        if should_emit {
            self.emit_status();
        }
        self.snapshot()
    }

    /// Host clicks "Don't Switch" -> leaves original queue untouched and prevents re-prompting for
    /// this outage until connectivity recovers and later drops again.
    pub fn decline_fallback(&self) -> OfflineFallbackStatus {
        {
            let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            guard.prompt_visible = false;
            guard.declined_outage_id = Some(guard.outage_id);
            guard.status_message =
                Some("Keeping current queue order; cached tracks remain playable.".to_string());
        }
        self.emit_status();
        self.snapshot()
    }

    /// Prepares the shuffled verified cached + local fallback list without mutating the caller's
    /// original queue until the caller activates it, saving `saved_original` safely.
    pub async fn activate_fallback(
        &self,
        original_snapshot: SavedQueueSnapshot,
        current_playing_track_id: Option<&str>,
    ) -> Result<Vec<Track>, String> {
        let seed = {
            let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            guard.outage_id.saturating_mul(0x9E37_79B9).max(1)
        };

        let mut pool = self
            .build_verified_offline_pool(&original_snapshot.tracks, seed)
            .await;

        // Avoid repeating the song that was already playing or already played in this outage.
        let played_already: HashSet<String> = {
            let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            guard.played_fallback_track_ids.clone()
        };
        pool.retain(|t| {
            !played_already.contains(&t.id)
                && current_playing_track_id.is_none_or(|cur_id| cur_id != t.id)
        });

        {
            let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            guard.prompt_visible = false;
            guard.prompt_message = None;
            if guard.saved_original.is_none() {
                guard.saved_original = Some(original_snapshot);
            }
            if pool.is_empty() {
                guard.fallback_active = false;
                guard.fallback_exhausted = true;
                guard.fallback_pool_ids.clear();
                guard.status_message = Some(
                    "No verified cached or local songs are available for offline playback."
                        .to_string(),
                );
                drop(guard);
                self.emit_status();
                return Err(
                    "No verified cached or local songs are available for offline playback."
                        .to_string(),
                );
            }

            guard.fallback_active = true;
            guard.fallback_exhausted = false;
            guard.reconnect_pending_finish = false;
            guard.fallback_pool_ids = pool.iter().map(|t| t.id.clone()).collect();
            guard.status_message = Some(format!(
                "Playing from {} verified offline tracks without modifying your queue.",
                pool.len()
            ));
        }
        self.emit_status();
        Ok(pool)
    }

    pub fn record_track_played(&self, track: &Track) {
        let mut changed = false;
        {
            let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            if guard.fallback_active {
                if guard.played_fallback_track_ids.insert(track.id.clone()) {
                    changed = true;
                }
                if let Some(eid) = &track.queue_entry_id {
                    guard.played_fallback_entry_ids.insert(eid.clone());
                }
            }
        }
        if changed {
            self.emit_status();
        }
    }

    pub fn mark_fallback_exhausted(&self) {
        {
            let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            if guard.fallback_active {
                guard.fallback_exhausted = true;
                guard.status_message = Some(
                    "All verified cached tracks in the offline pool have been played.".to_string(),
                );
            }
        }
        self.emit_status();
    }

    /// Marks connectivity restored. If offline fallback is actively playing a song, marks
    /// `reconnect_pending_finish = true` so the active track finishes cleanly before resuming the
    /// original queue (unless the user explicitly skips).
    pub fn mark_reconnected(&self, currently_playing_audio: bool) -> OfflineFallbackStatus {
        let mut changed = false;
        {
            let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            if guard.is_offline || guard.prompt_visible {
                guard.is_offline = false;
                guard.prompt_visible = false;
                guard.prompt_message = None;
                changed = true;
                if guard.fallback_active && currently_playing_audio {
                    guard.reconnect_pending_finish = true;
                    guard.status_message = Some(
                        "Connection restored — finishing current track before resuming your queue."
                            .to_string(),
                    );
                } else if !guard.fallback_active {
                    guard.status_message = None;
                }
            }
        }
        if changed {
            self.emit_status();
        }
        self.snapshot()
    }

    /// Takes the saved original queue + played fallback tracking when finishing/skipping back to
    /// online original queue mode.
    pub fn take_restore_bundle(
        &self,
    ) -> Option<(SavedQueueSnapshot, HashSet<String>, HashSet<String>)> {
        let bundle = {
            let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let saved = guard.saved_original.take()?;
            let entry_ids = std::mem::take(&mut guard.played_fallback_entry_ids);
            let track_ids = std::mem::take(&mut guard.played_fallback_track_ids);
            guard.fallback_active = false;
            guard.fallback_exhausted = false;
            guard.reconnect_pending_finish = false;
            guard.fallback_pool_ids.clear();
            guard.status_message = Some("Returned to original playback queue.".to_string());
            (saved, entry_ids, track_ids)
        };
        self.emit_status();
        Some(bundle)
    }

    pub fn is_fallback_active(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.fallback_active)
            .unwrap_or(false)
    }

    pub fn should_restore_on_advance(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.fallback_active && !g.is_offline)
            .unwrap_or(false)
    }

    /// Assembles all verified cached YouTube Music tracks plus valid local library tracks.
    pub async fn build_verified_offline_pool(
        &self,
        queue_tracks: &[Track],
        seed: u64,
    ) -> Vec<Track> {
        let mut by_track_id: HashMap<String, Track> = HashMap::new();
        let mut by_video_id: HashMap<String, Track> = HashMap::new();

        for t in queue_tracks {
            by_track_id.entry(t.id.clone()).or_insert_with(|| t.clone());
            if let Some(vid) = extract_ytmusic_video_id(t) {
                by_video_id.entry(vid).or_insert_with(|| t.clone());
            }
        }

        // Lookup recent/playlist cached metadata in SQLite for cached video IDs not currently in queue.
        let history_rows = sqlx::query(
            r#"
            SELECT track_id, title, artist, album, album_artist, duration_ms, artwork_uri, source_kind, provider_id
            FROM recent_plays
            UNION ALL
            SELECT id AS track_id, title, artist, album, album_artist, duration_ms, artwork_uri, source_kind, provider_id
            FROM playlist_provider_tracks
            "#,
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        for r in history_rows {
            let tid: String = r.get("track_id");
            let t = Track {
                id: tid.clone(),
                title: r.get("title"),
                artist: r.get("artist"),
                album: r.get("album"),
                album_artist: r.get("album_artist"),
                duration_ms: r.get("duration_ms"),
                artwork_uri: r.get("artwork_uri"),
                source_kind: r.get("source_kind"),
                provider_id: r.get("provider_id"),
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
            };
            if let Some(vid) = extract_ytmusic_video_id(&t) {
                by_video_id.entry(vid).or_insert(t);
            }
        }

        let mut pool: Vec<Track> = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        // 1. All verified cached YouTube Music tracks from stream cache.
        for vid in self.ytmusic.list_verified_cached_video_ids() {
            let Some(path) = self.ytmusic.cached_playable_path(&vid) else {
                continue;
            };
            let track_id = format!("ytmusic:{vid}");
            if !seen_ids.insert(track_id.clone()) {
                continue;
            }
            let mut track = by_video_id.get(&vid).cloned().unwrap_or_else(|| Track {
                id: track_id,
                title: format!("Cached Track ({vid})"),
                artist: "Cached Offline".to_string(),
                album: String::new(),
                album_artist: None,
                duration_ms: 0,
                artwork_uri: None,
                source_kind: "provider".to_string(),
                provider_id: Some("youtube_music".to_string()),
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
            });
            track.file_uri = Some(path.to_string_lossy().to_string());
            pool.push(track);
        }

        // 2. All playable local library tracks verified present on disk.
        let local_rows = sqlx::query(
            r#"
            SELECT id, title, artist, album, album_artist, duration_ms, artwork_uri,
                   source_kind, provider_id, file_uri, release_year, track_number,
                   disc_number, genre, is_liked, play_count, date_added
            FROM tracks
            WHERE is_deleted = 0 AND source_kind = 'local' AND file_uri IS NOT NULL
            "#,
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        for row in local_rows {
            let id: String = row.get("id");
            let file_uri: String = row.get("file_uri");
            if !Path::new(&file_uri).is_file() || !seen_ids.insert(id.clone()) {
                continue;
            }
            let queue_entry_id = by_track_id.get(&id).and_then(|t| t.queue_entry_id.clone());
            pool.push(Track {
                id,
                title: row.get("title"),
                artist: row.get("artist"),
                album: row.get("album"),
                album_artist: row.get("album_artist"),
                duration_ms: row.get("duration_ms"),
                artwork_uri: row.get("artwork_uri"),
                source_kind: row.get("source_kind"),
                provider_id: row.get("provider_id"),
                file_uri: Some(file_uri),
                release_year: row.get("release_year"),
                track_number: row.get("track_number"),
                disc_number: row.get("disc_number"),
                genre: row.get("genre"),
                is_liked: row.get::<i64, _>("is_liked") == 1,
                play_count: row.get("play_count"),
                date_added: row.get("date_added"),
                queue_entry_id,
                contributor_id: None,
                contributor_name: None,
                priority_override: None,
            });
        }

        // Deterministic Fisher-Yates shuffle seeded by `seed`.
        if pool.len() > 1 {
            let mut state = if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            };
            for i in (1..pool.len()).rev() {
                state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^= z >> 31;
                let j = (z as usize) % (i + 1);
                pool.swap(i, j);
            }
        }

        pool
    }
}

/// Lightweight periodic connectivity check used strictly when `offline_paused` or `is_offline` is
/// active so reconnection is detected automatically without blocking audio playback.
pub async fn probe_internet_connectivity() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(4))
        .build()
    else {
        return false;
    };
    match client
        .get("https://music.youtube.com/generate_204")
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success() || resp.status().as_u16() == 204,
        Err(_) => false,
    }
}

/// Updates `PreloadCoordinator` non-blockingly whenever playback queue transitions occur outside
/// Cavia Connect sessions.
pub fn sync_solo_preload_window(
    preload: &PreloadCoordinator,
    current: Option<Track>,
    upcoming: Vec<Track>,
) {
    preload.update_window(current, upcoming, Vec::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::queue::RepeatMode;
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

    fn make_track(id: &str, entry_id: &str) -> Track {
        Track {
            id: id.to_string(),
            title: id.to_string(),
            artist: "Artist".into(),
            album: "Album".into(),
            album_artist: None,
            duration_ms: 120_000,
            artwork_uri: None,
            source_kind: if id.starts_with("ytmusic:") {
                "provider"
            } else {
                "local"
            }
            .into(),
            provider_id: id.starts_with("ytmusic:").then(|| "youtube_music".into()),
            file_uri: None,
            release_year: None,
            track_number: None,
            disc_number: None,
            genre: None,
            is_liked: false,
            play_count: 0,
            date_added: String::new(),
            queue_entry_id: Some(entry_id.into()),
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        }
    }

    #[tokio::test]
    async fn offline_prompt_shows_once_per_outage_and_preserves_original_queue() {
        let pool = test_db().await;
        let cache_dir =
            std::env::temp_dir().join(format!("cavia-offline-cache-{}", uuid::Uuid::new_v4()));
        let yt = Arc::new(YtMusicService::new(cache_dir.clone()).unwrap());
        std::fs::write(cache_dir.join("cachedVidA1.m4a"), minimal_test_wav()).unwrap();
        std::fs::write(cache_dir.join("cachedVidB2.m4a"), minimal_test_wav()).unwrap();
        std::fs::write(cache_dir.join("incomplete.part"), b"half").unwrap();

        let service = OfflineFallbackService::new(pool, yt, None);
        let original = SavedQueueSnapshot {
            tracks: vec![
                make_track("ytmusic:cachedVidA1", "qe-1"),
                make_track("ytmusic:uncachedVid3", "qe-2"),
                make_track("ytmusic:cachedVidB2", "qe-3"),
            ],
            current_index: Some(0),
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            shuffle_order: vec![],
            shuffle_pos: 0,
        };

        // First outage event opens prompt once.
        let s1 = service.notify_connectivity_lost(&original.tracks).await;
        assert!(s1.is_offline);
        assert!(s1.prompt_visible);
        assert_eq!(s1.outage_id, 1);

        // User chooses Don't Switch -> never prompts again in same outage.
        let s2 = service.decline_fallback();
        assert!(!s2.prompt_visible);
        let s3 = service.notify_connectivity_lost(&original.tracks).await;
        assert!(!s3.prompt_visible, "Must not nag repeatedly in same outage");

        // Reconnect and suffer second distinct outage -> prompts cleanly once again.
        service.mark_reconnected(false);
        let s4 = service.notify_connectivity_lost(&original.tracks).await;
        assert!(s4.prompt_visible);
        assert_eq!(s4.outage_id, 2);

        // Activate fallback: shuffles verified cached songs only (excludes .part & current track), preserves original.
        let pool_tracks = service
            .activate_fallback(original.clone(), Some("ytmusic:cachedVidA1"))
            .await
            .unwrap();
        assert_eq!(pool_tracks.len(), 1);
        assert_eq!(pool_tracks[0].id, "ytmusic:cachedVidB2");

        // Recording played fallback tracks & exhausting pool updates status honestly without looping.
        service.record_track_played(&pool_tracks[0]);
        service.mark_fallback_exhausted();
        let exhausted = service.snapshot();
        assert!(exhausted.fallback_exhausted);
        assert_eq!(exhausted.remaining_fallback_count, 0);

        // Reconnection during active fallback marks reconnect_pending_finish first, then cleanly restores original.
        let reconn = service.mark_reconnected(true);
        assert!(reconn.reconnect_pending_finish);
        let (restored, _entries, played_track_ids) = service.take_restore_bundle().unwrap();
        assert_eq!(restored.tracks.len(), 3);
        assert_eq!(restored.current_index, Some(0));
        assert!(played_track_ids.contains("ytmusic:cachedVidB2"));

        let _ = std::fs::remove_dir_all(cache_dir);
    }
}
