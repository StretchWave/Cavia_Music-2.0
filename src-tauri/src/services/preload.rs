use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, watch};

use crate::domain::Track;
use crate::services::ytmusic::YtMusicService;

pub const DEFAULT_PRELOAD_TARGET: usize = 10;
pub const MAX_PRELOAD_TARGET: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreloadItemStatus {
    Ready,
    Downloading,
    Queued,
    PendingPreload,
    Failed,
    OfflinePaused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreloadItemView {
    pub queue_entry_id: String,
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub contributor_name: Option<String>,
    pub source_kind: String,
    pub status: PreloadItemStatus,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreloadStatusSnapshot {
    pub enabled: bool,
    pub target_count: usize,
    pub ready_count: usize,
    pub queued_window_count: usize,
    pub late_pending_count: usize,
    pub is_downloading: bool,
    pub paused_for_playback: bool,
    pub offline_paused: bool,
    pub active_title: Option<String>,
    pub cache_used_bytes: u64,
    pub cache_limit_bytes: u64,
    pub items: Vec<PreloadItemView>,
    pub late_items: Vec<PreloadItemView>,
}

/// Abstracted cache fetcher so sequential preload, preemption, verification, and failure recovery
/// can be exercised both against live `YtMusicService` and deterministic mocks in tests.
pub trait PreloadFetcher: Send + Sync {
    fn cached_playable_path(&self, video_id: &str) -> Option<PathBuf>;
    fn set_protected_video_ids(&self, ids: HashSet<String>);
    fn cleanup_partial_files(&self);
    fn cache_usage_bytes(&self) -> (u64, u64);
    fn preload_track<'a>(
        &'a self,
        video_id: &'a str,
        cancel_rx: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<PathBuf, String>> + Send + 'a>>;
}

impl PreloadFetcher for YtMusicService {
    fn cached_playable_path(&self, video_id: &str) -> Option<PathBuf> {
        YtMusicService::cached_playable_path(self, video_id)
    }

    fn set_protected_video_ids(&self, ids: HashSet<String>) {
        YtMusicService::set_protected_video_ids(self, ids);
    }

    fn cleanup_partial_files(&self) {
        YtMusicService::cleanup_partial_files(self);
    }

    fn cache_usage_bytes(&self) -> (u64, u64) {
        YtMusicService::cache_usage_bytes(self)
    }

    fn preload_track<'a>(
        &'a self,
        video_id: &'a str,
        cancel_rx: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<PathBuf, String>> + Send + 'a>> {
        Box::pin(async move { YtMusicService::preload_video(self, video_id, cancel_rx).await })
    }
}

pub fn extract_ytmusic_video_id(track: &Track) -> Option<String> {
    if track.source_kind != "provider"
        && track.provider_id.as_deref() != Some("youtube_music")
        && !track.id.starts_with("ytmusic:")
    {
        return None;
    }
    let raw = track.id.strip_prefix("ytmusic:").unwrap_or(&track.id);
    let vid = raw.split(&['?', '&', '#'][..]).next()?.trim();
    if vid.is_empty() {
        None
    } else {
        Some(vid.to_string())
    }
}

pub fn is_connectivity_error(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("connectivity failed")
        || lower.contains("could not connect")
        || lower.contains("could not reach")
        || lower.contains("error sending request")
        || lower.contains("timed out")
        || lower.contains("dns")
        || lower.contains("network is unreachable")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("connection aborted")
        || lower.contains("broken pipe")
}

#[derive(Clone)]
pub struct PreloadCallbacks {
    pub on_status_changed: Arc<dyn Fn(PreloadStatusSnapshot) + Send + Sync>,
    pub on_late_verified: Arc<dyn Fn(String, PathBuf) + Send + Sync>,
    pub on_connectivity_lost: Arc<dyn Fn(String) + Send + Sync>,
}

impl Default for PreloadCallbacks {
    fn default() -> Self {
        Self {
            on_status_changed: Arc::new(|_| {}),
            on_late_verified: Arc::new(|_, _| {}),
            on_connectivity_lost: Arc::new(|_| {}),
        }
    }
}

struct PreloadState {
    target_count: usize,
    paused_for_playback_count: usize,
    offline_paused: bool,
    current_track: Option<Track>,
    upcoming_window: Vec<Track>,
    late_pending: Vec<Track>,
    failed_entries: HashMap<String, String>,
    active_entry_id: Option<String>,
    active_title: Option<String>,
    active_cancel_tx: Option<watch::Sender<bool>>,
    last_snapshot: PreloadStatusSnapshot,
}

pub struct PreloadCoordinator {
    fetcher: Arc<dyn PreloadFetcher>,
    db: Option<SqlitePool>,
    callbacks: PreloadCallbacks,
    state: Mutex<PreloadState>,
    notify: Notify,
}

impl PreloadCoordinator {
    pub async fn load_persisted_target(db: &SqlitePool) -> usize {
        let row = sqlx::query("SELECT value_json FROM app_settings WHERE key = 'preload_target'")
            .fetch_optional(db)
            .await
            .ok()
            .flatten();
        row.and_then(|r| {
            let raw: String = r.get("value_json");
            serde_json::from_str::<usize>(&raw)
                .ok()
                .or_else(|| raw.trim().parse::<usize>().ok())
        })
        .map(|n| n.min(MAX_PRELOAD_TARGET))
        .unwrap_or(DEFAULT_PRELOAD_TARGET)
    }

    pub async fn persist_target(db: &SqlitePool, target: usize) -> Result<(), String> {
        let clamped = target.min(MAX_PRELOAD_TARGET);
        let raw = clamped.to_string();
        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value_json, updated_at)
            VALUES ('preload_target', ?, CURRENT_TIMESTAMP)
            ON CONFLICT(key) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(raw)
        .execute(db)
        .await
        .map(|_| ())
        .map_err(|e| format!("Failed to persist preload target: {e}"))
    }

    pub fn new(
        fetcher: Arc<dyn PreloadFetcher>,
        db: Option<SqlitePool>,
        initial_target: usize,
        callbacks: PreloadCallbacks,
    ) -> Arc<Self> {
        let target = initial_target.min(MAX_PRELOAD_TARGET);
        let (cache_used_bytes, cache_limit_bytes) = fetcher.cache_usage_bytes();
        let initial_snap = PreloadStatusSnapshot {
            enabled: target > 0,
            target_count: target,
            ready_count: 0,
            queued_window_count: 0,
            late_pending_count: 0,
            is_downloading: false,
            paused_for_playback: false,
            offline_paused: false,
            active_title: None,
            cache_used_bytes,
            cache_limit_bytes,
            items: Vec::new(),
            late_items: Vec::new(),
        };

        Arc::new(Self {
            fetcher,
            db,
            callbacks,
            state: Mutex::new(PreloadState {
                target_count: target,
                paused_for_playback_count: 0,
                offline_paused: false,
                current_track: None,
                upcoming_window: Vec::new(),
                late_pending: Vec::new(),
                failed_entries: HashMap::new(),
                active_entry_id: None,
                active_title: None,
                active_cancel_tx: None,
                last_snapshot: initial_snap,
            }),
            notify: Notify::new(),
        })
    }

    /// Spawns the bounded concurrency=1 preload worker loop on the current Tokio runtime.
    pub fn start_worker(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let coordinator = Arc::clone(self);
        tokio::spawn(async move {
            coordinator.worker_loop().await;
        })
    }

    pub fn snapshot(&self) -> PreloadStatusSnapshot {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last_snapshot
            .clone()
    }

    pub async fn set_target(
        self: &Arc<Self>,
        target: usize,
    ) -> Result<PreloadStatusSnapshot, String> {
        let clamped = target.min(MAX_PRELOAD_TARGET);
        if let Some(db) = &self.db {
            Self::persist_target(db, clamped).await?;
        }
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.target_count = clamped;
            if clamped == 0
                && let Some(tx) = guard.active_cancel_tx.take()
            {
                let _ = tx.send(true);
            }
        }
        if clamped == 0 {
            self.fetcher.cleanup_partial_files();
        }
        let snap = self.recompute_and_emit();
        self.notify.notify_one();
        Ok(snap)
    }

    /// Updates rolling preload inputs from the current playback + 3-tier queue state.
    pub fn update_window(
        &self,
        current_track: Option<Track>,
        upcoming_sequence: Vec<Track>,
        late_pending: Vec<Track>,
    ) {
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.current_track = current_track;
            guard.upcoming_window = upcoming_sequence;
            guard.late_pending = late_pending;
        }
        self.sync_cache_protection();
        self.recompute_and_emit();
        self.notify.notify_one();
    }

    pub fn begin_active_playback_resolution(&self) {
        let mut cancel_needed = false;
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.paused_for_playback_count = guard.paused_for_playback_count.saturating_add(1);
            if let Some(tx) = guard.active_cancel_tx.take() {
                let _ = tx.send(true);
                cancel_needed = true;
            }
        }
        if cancel_needed {
            self.fetcher.cleanup_partial_files();
        }
        self.recompute_and_emit();
    }

    pub fn end_active_playback_resolution(&self) {
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.paused_for_playback_count = guard.paused_for_playback_count.saturating_sub(1);
        }
        self.recompute_and_emit();
        self.notify.notify_one();
    }

    pub fn set_offline_paused(&self, paused: bool) {
        let mut cancelled = false;
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.offline_paused = paused;
            if paused {
                if let Some(tx) = guard.active_cancel_tx.take() {
                    let _ = tx.send(true);
                    cancelled = true;
                }
            } else {
                guard.failed_entries.clear();
            }
        }
        if paused && cancelled {
            self.fetcher.cleanup_partial_files();
        }
        self.recompute_and_emit();
        if !paused {
            self.notify.notify_one();
        }
    }

    pub fn retry_failed(&self) {
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.offline_paused = false;
            guard.failed_entries.clear();
        }
        self.recompute_and_emit();
        self.notify.notify_one();
    }

    fn sync_cache_protection(&self) {
        let (target, current, upcoming, late) = {
            let guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            (
                guard.target_count,
                guard.current_track.clone(),
                guard.upcoming_window.clone(),
                guard.late_pending.clone(),
            )
        };
        let mut protected = HashSet::new();
        if let Some(cur) = current.as_ref().and_then(extract_ytmusic_video_id) {
            protected.insert(cur);
        }
        for track in upcoming.iter().take(target) {
            if let Some(vid) = extract_ytmusic_video_id(track) {
                protected.insert(vid);
            }
        }
        // Protect any actively downloaded late entry until it enters or leaves window.
        if let Some(first_late) = late.first().and_then(extract_ytmusic_video_id) {
            protected.insert(first_late);
        }
        self.fetcher.set_protected_video_ids(protected);
    }

    fn is_track_verified_ready(&self, track: &Track) -> bool {
        if let Some(vid) = extract_ytmusic_video_id(track) {
            return self.fetcher.cached_playable_path(&vid).is_some();
        }
        track
            .file_uri
            .as_deref()
            .is_some_and(|uri| Path::new(uri).is_file())
    }

    fn recompute_and_emit(&self) -> PreloadStatusSnapshot {
        let (cache_used_bytes, cache_limit_bytes) = self.fetcher.cache_usage_bytes();
        let snap = {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let target = guard.target_count.min(MAX_PRELOAD_TARGET);
            let window: Vec<Track> = guard.upcoming_window.iter().take(target).cloned().collect();
            let mut ready_count = 0_usize;
            let mut items = Vec::with_capacity(window.len());

            for track in &window {
                let entry_id = track
                    .queue_entry_id
                    .clone()
                    .unwrap_or_else(|| track.id.clone());
                let is_ready = self.is_track_verified_ready(track);
                let (status, error_message) = if is_ready {
                    ready_count += 1;
                    (PreloadItemStatus::Ready, None)
                } else if guard.active_entry_id.as_deref() == Some(entry_id.as_str()) {
                    (PreloadItemStatus::Downloading, None)
                } else if guard.offline_paused {
                    (PreloadItemStatus::OfflinePaused, None)
                } else if let Some(err) = guard.failed_entries.get(&entry_id) {
                    (PreloadItemStatus::Failed, Some(err.clone()))
                } else {
                    (PreloadItemStatus::Queued, None)
                };

                items.push(PreloadItemView {
                    queue_entry_id: entry_id,
                    track_id: track.id.clone(),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    contributor_name: track.contributor_name.clone(),
                    source_kind: track.source_kind.clone(),
                    status,
                    error_message,
                });
            }

            let mut late_items = Vec::with_capacity(guard.late_pending.len());
            for track in &guard.late_pending {
                let entry_id = track
                    .queue_entry_id
                    .clone()
                    .unwrap_or_else(|| track.id.clone());
                let (status, error_message) =
                    if guard.active_entry_id.as_deref() == Some(entry_id.as_str()) {
                        (PreloadItemStatus::Downloading, None)
                    } else if let Some(err) = guard.failed_entries.get(&entry_id) {
                        (PreloadItemStatus::Failed, Some(err.clone()))
                    } else {
                        (PreloadItemStatus::PendingPreload, None)
                    };
                late_items.push(PreloadItemView {
                    queue_entry_id: entry_id,
                    track_id: track.id.clone(),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    contributor_name: track.contributor_name.clone(),
                    source_kind: track.source_kind.clone(),
                    status,
                    error_message,
                });
            }

            let next_snap = PreloadStatusSnapshot {
                enabled: target > 0,
                target_count: target,
                ready_count,
                queued_window_count: window.len(),
                late_pending_count: late_items.len(),
                is_downloading: guard.active_entry_id.is_some(),
                paused_for_playback: guard.paused_for_playback_count > 0,
                offline_paused: guard.offline_paused,
                active_title: guard.active_title.clone(),
                cache_used_bytes,
                cache_limit_bytes,
                items,
                late_items,
            };
            guard.last_snapshot = next_snap.clone();
            next_snap
        };

        (self.callbacks.on_status_changed)(snap.clone());
        snap
    }

    /// Runs one sequential step. Exposed `pub async` so tests can verify deterministic single-step
    /// behavior as well as continuous worker operation. Returns `true` if work was attempted.
    pub async fn run_next_step(&self) -> bool {
        let next_job = {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if guard.target_count == 0
                || guard.paused_for_playback_count > 0
                || guard.offline_paused
            {
                return false;
            }

            let target = guard.target_count.min(MAX_PRELOAD_TARGET);
            // Late pending contributions wait cleanly until the current sequential download finishes
            // and are prioritized alongside missing tail window slots (after the immediate next
            // uncached track if slot 0 is missing!).
            let window_missing = guard
                .upcoming_window
                .iter()
                .take(target)
                .enumerate()
                .find(|(_, t)| {
                    let entry_id = t.queue_entry_id.as_deref().unwrap_or(&t.id);
                    !self.is_track_verified_ready(t)
                        && !guard.failed_entries.contains_key(entry_id)
                        && extract_ytmusic_video_id(t).is_some()
                })
                .map(|(idx, t)| (idx, t.clone()));

            let late_candidate = guard
                .late_pending
                .iter()
                .find(|t| {
                    let entry_id = t.queue_entry_id.as_deref().unwrap_or(&t.id);
                    !guard.failed_entries.contains_key(entry_id)
                        && extract_ytmusic_video_id(t).is_some()
                })
                .cloned();

            let picked = match (window_missing, late_candidate) {
                // Guarantee committed next track (index 0) prepares first if missing.
                (Some((0, t)), _) => Some((t, false)),
                // Otherwise prepare the highest-priority pending late contribution next.
                (_, Some(late_t)) => Some((late_t, true)),
                (Some((_, t)), None) => Some((t, false)),
                (None, None) => None,
            };

            let Some((track, is_late)) = picked else {
                guard.active_entry_id = None;
                guard.active_title = None;
                return false;
            };

            let entry_id = track
                .queue_entry_id
                .clone()
                .unwrap_or_else(|| track.id.clone());
            let Some(video_id) = extract_ytmusic_video_id(&track) else {
                return false;
            };

            let (cancel_tx, cancel_rx) = watch::channel(false);
            guard.active_entry_id = Some(entry_id.clone());
            guard.active_title = Some(track.title.clone());
            guard.active_cancel_tx = Some(cancel_tx);

            (entry_id, video_id, is_late, cancel_rx)
        };

        let (entry_id, video_id, is_late, cancel_rx) = next_job;
        if is_late && let Some(existing) = self.fetcher.cached_playable_path(&video_id) {
            (self.callbacks.on_late_verified)(entry_id, existing);
            self.sync_cache_protection();
            self.recompute_and_emit();
            return true;
        }
        self.sync_cache_protection();
        self.recompute_and_emit();

        let result = self.fetcher.preload_track(&video_id, cancel_rx).await;

        let mut connectivity_err: Option<String> = None;
        let mut promoted_path: Option<PathBuf> = None;
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.active_entry_id = None;
            guard.active_title = None;
            guard.active_cancel_tx = None;

            match result {
                Ok(path) => {
                    guard.failed_entries.remove(&entry_id);
                    if is_late {
                        promoted_path = Some(path);
                    }
                }
                Err(err) => {
                    if err == "Preload cancelled" {
                        // Do not mark cancelled tasks as permanent failures.
                    } else if is_connectivity_error(&err) {
                        guard.offline_paused = true;
                        connectivity_err = Some(err);
                    } else {
                        guard.failed_entries.insert(entry_id.clone(), err);
                    }
                }
            }
        }

        if let Some(err) = connectivity_err {
            self.fetcher.cleanup_partial_files();
            (self.callbacks.on_connectivity_lost)(err);
        } else if let Some(path) = promoted_path {
            (self.callbacks.on_late_verified)(entry_id, path);
        }

        self.sync_cache_protection();
        self.recompute_and_emit();
        true
    }

    async fn worker_loop(self: Arc<Self>) {
        loop {
            while self.run_next_step().await {
                tokio::task::yield_now().await;
            }
            self.notify.notified().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockFetcher {
        dir: PathBuf,
        in_flight: AtomicUsize,
        max_concurrent: AtomicUsize,
        download_order: Mutex<Vec<String>>,
        fail_with: Mutex<Option<String>>,
        protected: Mutex<HashSet<String>>,
        delay_ms: u64,
    }

    impl MockFetcher {
        fn new(dir: PathBuf, delay_ms: u64) -> Self {
            std::fs::create_dir_all(&dir).unwrap();
            Self {
                dir,
                in_flight: AtomicUsize::new(0),
                max_concurrent: AtomicUsize::new(0),
                download_order: Mutex::new(Vec::new()),
                fail_with: Mutex::new(None),
                protected: Mutex::new(HashSet::new()),
                delay_ms,
            }
        }

        fn path_for(&self, vid: &str) -> PathBuf {
            self.dir.join(format!("{vid}.m4a"))
        }
    }

    impl PreloadFetcher for MockFetcher {
        fn cached_playable_path(&self, video_id: &str) -> Option<PathBuf> {
            let p = self.path_for(video_id);
            (p.is_file() && p.metadata().map(|m| m.len() > 0).unwrap_or(false)).then_some(p)
        }

        fn set_protected_video_ids(&self, ids: HashSet<String>) {
            *self.protected.lock().unwrap() = ids;
        }

        fn cleanup_partial_files(&self) {
            if let Ok(entries) = std::fs::read_dir(&self.dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("part") {
                        let _ = std::fs::remove_file(p);
                    }
                }
            }
        }

        fn cache_usage_bytes(&self) -> (u64, u64) {
            (0, 500 * 1024 * 1024)
        }

        fn preload_track<'a>(
            &'a self,
            video_id: &'a str,
            mut cancel_rx: watch::Receiver<bool>,
        ) -> Pin<Box<dyn Future<Output = Result<PathBuf, String>> + Send + 'a>> {
            Box::pin(async move {
                let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_concurrent.fetch_max(current, Ordering::SeqCst);

                let part = self.dir.join(format!("{video_id}.part"));
                std::fs::write(&part, b"partial-bytes").unwrap();
                if self.delay_ms > 0 {
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)) => {}
                        _ = cancel_rx.changed() => {
                            let _ = std::fs::remove_file(&part);
                            self.in_flight.fetch_sub(1, Ordering::SeqCst);
                            return Err("Preload cancelled".into());
                        }
                    }
                }

                self.in_flight.fetch_sub(1, Ordering::SeqCst);
                if let Some(err) = self.fail_with.lock().unwrap().clone() {
                    let _ = std::fs::remove_file(&part);
                    return Err(err);
                }
                self.download_order
                    .lock()
                    .unwrap()
                    .push(video_id.to_string());
                let final_path = self.path_for(video_id);
                std::fs::write(&final_path, b"verified-audio-bytes").unwrap();
                let _ = std::fs::remove_file(&part);
                Ok(final_path)
            })
        }
    }

    fn yt(vid: &str) -> Track {
        Track {
            id: format!("ytmusic:{vid}"),
            title: format!("Title {vid}"),
            artist: "Artist".into(),
            album: "Album".into(),
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
            queue_entry_id: Some(format!("qe-{vid}")),
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        }
    }

    #[tokio::test]
    async fn sequential_preload_concurrency_one_and_rolling_readiness() {
        let dir = std::env::temp_dir().join(format!("cavia-preload-seq-{}", uuid::Uuid::new_v4()));
        let fetcher = Arc::new(MockFetcher::new(dir.clone(), 5));
        let coord = PreloadCoordinator::new(fetcher.clone(), None, 4, PreloadCallbacks::default());

        let queue: Vec<Track> = (0..6).map(|i| yt(&format!("v{i}"))).collect();
        coord.update_window(Some(queue[0].clone()), queue[1..].to_vec(), Vec::new());

        while coord.run_next_step().await {}

        assert_eq!(fetcher.max_concurrent.load(Ordering::SeqCst), 1);
        let order = fetcher.download_order.lock().unwrap().clone();
        assert_eq!(order, vec!["v1", "v2", "v3", "v4"]);
        let snap = coord.snapshot();
        assert_eq!(snap.ready_count, 4);
        assert_eq!(snap.target_count, 4);

        // Advance window so v5 enters window: only v5 downloads; v2..v4 are reused without redownload.
        coord.update_window(Some(queue[1].clone()), queue[2..].to_vec(), Vec::new());
        while coord.run_next_step().await {}
        let order = fetcher.download_order.lock().unwrap().clone();
        assert_eq!(order, vec!["v1", "v2", "v3", "v4", "v5"]);
        assert!(fetcher.protected.lock().unwrap().contains("v1"));
        assert!(!fetcher.protected.lock().unwrap().contains("v0"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn active_playback_resolution_preempts_and_pauses_preload() {
        let dir =
            std::env::temp_dir().join(format!("cavia-preload-preempt-{}", uuid::Uuid::new_v4()));
        let fetcher = Arc::new(MockFetcher::new(dir.clone(), 80));
        let coord = PreloadCoordinator::new(fetcher.clone(), None, 3, PreloadCallbacks::default());
        coord.update_window(None, vec![yt("p1"), yt("p2")], Vec::new());

        let coord_bg = Arc::clone(&coord);
        let handle = tokio::spawn(async move { coord_bg.run_next_step().await });
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        coord.begin_active_playback_resolution();
        let _ = handle.await;

        assert!(
            !dir.join("p1.part").exists(),
            "partial file must be removed on preemption"
        );
        assert!(fetcher.cached_playable_path("p1").is_none());
        assert!(
            !coord.run_next_step().await,
            "preload must stay paused during active resolution"
        );

        coord.end_active_playback_resolution();
        assert!(
            coord.run_next_step().await,
            "preload resumes after active resolution finishes"
        );
        assert!(fetcher.cached_playable_path("p1").is_some());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn ytmusic_cache_protection_preserves_window_and_excludes_parts() {
        let dir = std::env::temp_dir().join(format!("cavia-ytcache-prot-{}", uuid::Uuid::new_v4()));
        let service = YtMusicService::new(dir.clone()).unwrap();

        let valid_wav = minimal_test_wav();
        let old_protected = dir.join("protVid1234.m4a");
        let old_unprotected = dir.join("evictVid999.m4a");
        let partial = dir.join("incomplete1.part");
        std::fs::write(&old_protected, &valid_wav).unwrap();
        std::fs::write(&old_unprotected, &valid_wav).unwrap();
        std::fs::write(&partial, b"partial").unwrap();

        let mut protected = HashSet::new();
        protected.insert("protVid1234".to_string());
        service.set_protected_video_ids(protected);

        service.prune_cache(1).unwrap();
        assert!(
            old_protected.exists(),
            "protected window track must survive cache prune"
        );
        assert!(
            !old_unprotected.exists(),
            "unprotected older track should be evicted first"
        );
        assert!(
            service
                .list_verified_cached_video_ids()
                .contains(&"protVid1234".to_string())
        );
        assert!(
            !service
                .list_verified_cached_video_ids()
                .contains(&"incomplete1".to_string())
        );

        // Releasing protection allows normal LRU eviction.
        service.set_protected_video_ids(HashSet::new());
        service.prune_cache(1).unwrap();
        assert!(
            !old_protected.exists(),
            "releasing protection must allow eviction"
        );

        let _ = std::fs::remove_dir_all(dir);
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
