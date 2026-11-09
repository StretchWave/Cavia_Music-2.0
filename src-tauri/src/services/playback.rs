use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::audio::{AudioBackend, PlaybackStatus, create_platform_backend};
use crate::domain::Track;
use crate::services::queue::{QueueManager, RepeatMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub generation: u64,
    pub status: String,
    pub current_track: Option<Track>,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub volume: u32,
    pub is_muted: bool,
    pub repeat_mode: String,
    pub is_shuffle: bool,
    pub playback_speed: f32,
    pub queue: Vec<Track>,
    pub error_message: Option<String>,
    pub device_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionUpdate {
    pub generation: u64,
    pub track_id: Option<String>,
    pub position_ms: i64,
    pub duration_ms: i64,
}

type StateEmitter = Box<dyn Fn(PlaybackSnapshot) + Send + Sync>;
type PositionEmitter = Box<dyn Fn(PositionUpdate) + Send + Sync>;
type MediaActionEmitter = Box<dyn Fn(&str) + Send + Sync>;

pub struct PlaybackService {
    backend: Box<dyn AudioBackend>,
    queue: QueueManager,
    volume: u32,
    is_muted: bool,
    previous_volume: u32,
    playback_speed: f32,
    database: SqlitePool,
    state_emitter: Option<StateEmitter>,
    position_emitter: Option<PositionEmitter>,
    media_action_emitter: Option<MediaActionEmitter>,
    last_error: Option<String>,
    generation: u64,
    media_controls: Option<souvlaki::MediaControls>,
    shutdown: Arc<AtomicBool>,
}

pub type SharedPlaybackService = Arc<Mutex<PlaybackService>>;

impl PlaybackService {
    pub fn new(database: SqlitePool) -> Result<SharedPlaybackService, String> {
        let backend = create_platform_backend()
            .map_err(|e| format!("Failed to create audio backend: {e}"))?;

        let shutdown = Arc::new(AtomicBool::new(false));

        let service = Self {
            backend,
            queue: QueueManager::new(),
            volume: 85,
            is_muted: false,
            previous_volume: 85,
            playback_speed: 1.0,
            database,
            state_emitter: None,
            position_emitter: None,
            media_action_emitter: None,
            last_error: None,
            generation: 1,
            media_controls: None,
            shutdown: shutdown.clone(),
        };

        let shared = Arc::new(Mutex::new(service));

        // Start background position and completion monitor loop
        let service_clone = Arc::clone(&shared);
        std::thread::spawn(move || {
            while !shutdown.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(250));
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(mut svc) = service_clone.lock() {
                    svc.tick();
                }
            }
        });

        Ok(shared)
    }

    #[cfg(test)]
    pub fn new_for_testing(backend: Box<dyn AudioBackend>, database: SqlitePool) -> Self {
        let shutdown = Arc::new(AtomicBool::new(true));
        Self {
            backend,
            queue: QueueManager::new(),
            volume: 85,
            is_muted: false,
            previous_volume: 85,
            playback_speed: 1.0,
            database,
            state_emitter: None,
            position_emitter: None,
            media_action_emitter: None,
            last_error: None,
            generation: 1,
            media_controls: None,
            shutdown,
        }
    }

    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        let app1 = app_handle.clone();
        self.state_emitter = Some(Box::new(move |snap| {
            let _ = app1.emit("playback:state-changed", snap);
        }));

        let app2 = app_handle.clone();
        self.position_emitter = Some(Box::new(move |pos| {
            let _ = app2.emit("playback:position", pos);
        }));

        let app3 = app_handle;
        self.media_action_emitter = Some(Box::new(move |action| {
            let _ = app3.emit("cavia://media-control", action);
        }));
    }

    pub fn init_media_controls(&mut self, hwnd: Option<*mut std::ffi::c_void>) {
        #[cfg(target_os = "windows")]
        {
            let config = souvlaki::PlatformConfig {
                dbus_name: "cavia_music",
                display_name: "Cavia Music",
                hwnd,
            };

            match souvlaki::MediaControls::new(config) {
                Ok(mut controls) => {
                    let _ = controls.attach(|event| {
                        // Media control events are handled by OS
                        let _ = event;
                    });
                    self.media_controls = Some(controls);
                }
                Err(e) => {
                    eprintln!(
                        "[PlaybackService] SMTC media controls initialization skipped: {e:?}"
                    );
                }
            }
        }
    }

    pub async fn restore_saved_state(&mut self) -> Result<(), String> {
        let row_opt = sqlx::query(
            "SELECT queue_json, current_track_id, repeat_mode, shuffle_enabled, playback_speed \
             FROM playback_state WHERE id = 1",
        )
        .fetch_optional(&self.database)
        .await
        .map_err(|e| format!("Failed to read saved playback state: {e}"))?;

        if let Some(row) = row_opt {
            let queue_json: String = row.try_get("queue_json").unwrap_or_else(|_| "[]".into());
            let current_track_id: Option<String> = row.try_get("current_track_id").ok();
            let repeat_mode: String = row.try_get("repeat_mode").unwrap_or_else(|_| "off".into());
            let shuffle_enabled: i64 = row.try_get("shuffle_enabled").unwrap_or(0);
            let playback_speed: f64 = row.try_get("playback_speed").unwrap_or(1.0);

            if let Ok(tracks) = serde_json::from_str::<Vec<Track>>(&queue_json) {
                // Verify that stored tracks exist on disk; filter out removed/unmounted files
                let valid_tracks: Vec<Track> = tracks
                    .into_iter()
                    .filter(|t| {
                        if let Some(ref uri) = t.file_uri {
                            Path::new(uri).exists()
                        } else {
                            false
                        }
                    })
                    .collect();

                if !valid_tracks.is_empty() {
                    let valid_start_id = current_track_id.as_deref().and_then(|id| {
                        if valid_tracks.iter().any(|t| t.id == id) {
                            Some(id)
                        } else {
                            None
                        }
                    });
                    self.queue.set_queue(valid_tracks, valid_start_id);
                }
            }

            self.queue
                .set_repeat_mode(RepeatMode::from(repeat_mode.as_str()));
            self.queue.set_shuffle(shuffle_enabled == 1);
            self.playback_speed = playback_speed as f32;
            let _ = self.backend.set_speed(self.playback_speed);
            self.generation += 1;
        }

        Ok(())
    }

    pub fn persist_state(&self) {
        let pool = self.database.clone();
        let queue_json = serde_json::to_string(self.queue.tracks()).unwrap_or_else(|_| "[]".into());
        let current_track_id = self.queue.current_track().map(|t| t.id.clone());
        let pos_ms = self.backend.position_ms() as i64;
        let repeat_mode = self.queue.repeat_mode().as_str().to_string();
        let shuffle_enabled = if self.queue.is_shuffle() { 1i64 } else { 0i64 };
        let speed = self.playback_speed as f64;

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = sqlx::query(
                    "INSERT INTO playback_state (id, queue_json, current_track_id, position_ms, repeat_mode, shuffle_enabled, playback_speed, updated_at) \
                     VALUES (1, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP) \
                     ON CONFLICT(id) DO UPDATE SET \
                     queue_json = excluded.queue_json, \
                     current_track_id = excluded.current_track_id, \
                     position_ms = excluded.position_ms, \
                     repeat_mode = excluded.repeat_mode, \
                     shuffle_enabled = excluded.shuffle_enabled, \
                     playback_speed = excluded.playback_speed, \
                     updated_at = CURRENT_TIMESTAMP"
                )
                .bind(queue_json)
                .bind(current_track_id)
                .bind(pos_ms)
                .bind(repeat_mode)
                .bind(shuffle_enabled)
                .bind(speed)
                .execute(&pool)
                .await;
            });
        }
    }

    /// Atomically transitions playback to the requested track ID.
    ///
    /// Non-negotiable correctness rules:
    /// 1. File accessibility and decoding are verified BEFORE queue selection is committed.
    /// 2. Previous audio is stopped so old audio cannot play under new track metadata.
    /// 3. If loading fails, queue selection is preserved and an honest error state is emitted.
    pub fn play_track(&mut self, track_id: &str) -> Result<(), String> {
        let track = self
            .queue
            .find_track_by_id(track_id)
            .cloned()
            .ok_or_else(|| format!("Track not found in active queue: {track_id}"))?;

        let file_path = track
            .file_uri
            .clone()
            .ok_or_else(|| "Track has no associated file path".to_string())?;

        let path = Path::new(&file_path);
        if !path.exists() {
            let err = format!("Audio file does not exist at path: {file_path}");
            self.last_error = Some(err.clone());
            let _ = self.backend.stop();
            self.generation += 1;
            self.emit_state();
            return Err(err);
        }

        // Stop current audio before loading new track to prevent audio bleed
        let _ = self.backend.stop();

        // Attempt decoder loading on backend
        if let Err(e) = self.backend.load(&file_path, 0) {
            let err = format!("Failed to load audio: {e}");
            self.last_error = Some(err.clone());
            self.generation += 1;
            self.emit_state();
            return Err(err);
        }

        // Loading succeeded: commit the queue selection and state
        self.queue.select_track_by_id(track_id);
        self.last_error = None;
        self.generation += 1;

        let _ = self.backend.set_volume(if self.is_muted {
            0.0
        } else {
            self.volume as f32 / 100.0
        });
        let _ = self.backend.set_speed(self.playback_speed);

        self.update_media_metadata(&track);
        self.persist_state();
        self.emit_state();
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), String> {
        if self.backend.status() == PlaybackStatus::Paused {
            self.backend.play().map_err(|e| e.to_string())?;
            self.sync_media_playback_state();
            self.emit_state();
            return Ok(());
        }

        if let Some(track) = self.queue.current_track().cloned() {
            self.play_track(&track.id)
        } else if let Some(first) = self.queue.tracks().first().cloned() {
            self.play_track(&first.id)
        } else {
            Err("Cannot play: queue is empty".into())
        }
    }

    pub fn pause(&mut self) -> Result<(), String> {
        self.backend.pause().map_err(|e| e.to_string())?;
        self.sync_media_playback_state();
        self.emit_state();
        Ok(())
    }

    pub fn play_pause(&mut self) -> Result<(), String> {
        match self.backend.status() {
            PlaybackStatus::Playing => self.pause(),
            PlaybackStatus::Paused => self.play(),
            PlaybackStatus::Idle | PlaybackStatus::Stopped => self.play(),
            PlaybackStatus::Loading => Ok(()),
            PlaybackStatus::Error => {
                if let Some(track) = self.queue.current_track().cloned() {
                    self.play_track(&track.id)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.backend.stop().map_err(|e| e.to_string())?;
        self.sync_media_playback_state();
        self.generation += 1;
        self.emit_state();
        Ok(())
    }

    pub fn seek(&mut self, position_ms: u64) -> Result<(), String> {
        self.backend.seek(position_ms).map_err(|e| e.to_string())?;
        self.emit_position();
        Ok(())
    }

    pub fn set_volume(&mut self, volume: u32) -> Result<(), String> {
        let clamped = volume.min(100);
        self.volume = clamped;
        self.is_muted = false;
        let backend_vol = clamped as f32 / 100.0;
        self.backend
            .set_volume(backend_vol)
            .map_err(|e| e.to_string())?;
        self.emit_state();
        Ok(())
    }

    pub fn toggle_mute(&mut self) -> Result<(), String> {
        if self.is_muted {
            self.is_muted = false;
            let vol = if self.previous_volume == 0 {
                85
            } else {
                self.previous_volume
            };
            self.volume = vol;
            self.backend
                .set_volume(vol as f32 / 100.0)
                .map_err(|e| e.to_string())?;
        } else {
            self.previous_volume = self.volume;
            self.is_muted = true;
            self.volume = 0;
            self.backend.set_volume(0.0).map_err(|e| e.to_string())?;
        }
        self.emit_state();
        Ok(())
    }

    pub fn set_speed(&mut self, speed: f32) -> Result<(), String> {
        let clamped = speed.clamp(0.25, 4.0);
        self.playback_speed = clamped;
        self.backend.set_speed(clamped).map_err(|e| e.to_string())?;
        self.persist_state();
        self.emit_state();
        Ok(())
    }

    pub fn set_repeat_mode(&mut self, mode: &str) -> Result<(), String> {
        self.queue.set_repeat_mode(RepeatMode::from(mode));
        self.persist_state();
        self.emit_state();
        Ok(())
    }

    pub fn set_shuffle(&mut self, enabled: bool) -> Result<(), String> {
        self.queue.set_shuffle(enabled);
        self.persist_state();
        self.emit_state();
        Ok(())
    }

    pub fn next(&mut self) -> Result<(), String> {
        if let Some(next_track) = self.queue.next().cloned() {
            self.play_track(&next_track.id)
        } else {
            let _ = self.backend.stop();
            self.sync_media_playback_state();
            self.emit_state();
            Ok(())
        }
    }

    pub fn previous(&mut self) -> Result<(), String> {
        let pos = self.backend.position_ms();
        if let Some(prev_track) = self.queue.previous(pos).cloned() {
            self.play_track(&prev_track.id)
        } else {
            let _ = self.backend.seek(0);
            self.emit_position();
            Ok(())
        }
    }

    pub fn set_queue(
        &mut self,
        tracks: Vec<Track>,
        start_track_id: Option<&str>,
    ) -> Result<(), String> {
        let target_id = start_track_id
            .map(|s| s.to_string())
            .or_else(|| tracks.first().map(|t| t.id.clone()));

        self.queue.set_queue(tracks, target_id.as_deref());
        self.generation += 1;
        self.persist_state();

        if let Some(id) = target_id {
            self.play_track(&id)?;
        } else {
            self.emit_state();
        }
        Ok(())
    }

    pub fn add_to_queue(&mut self, track: Track) -> Result<(), String> {
        self.queue.add_track(track);
        self.generation += 1;
        self.persist_state();
        self.emit_state();
        Ok(())
    }

    pub fn remove_from_queue(&mut self, track_id: &str) -> Result<(), String> {
        let is_current = self
            .queue
            .current_track()
            .map(|t| t.id == track_id)
            .unwrap_or(false);

        if is_current {
            let _ = self.backend.stop();
        }

        self.queue.remove_track(track_id);
        self.generation += 1;
        self.persist_state();
        self.emit_state();
        Ok(())
    }

    pub fn remove_from_queue_by_index(&mut self, index: usize) -> Result<(), String> {
        let is_current = self.queue.current_index() == Some(index);

        if is_current {
            let _ = self.backend.stop();
        }

        self.queue.remove_at(index);
        self.generation += 1;
        self.persist_state();
        self.emit_state();
        Ok(())
    }

    pub fn reorder_queue(&mut self, from: usize, to: usize) -> Result<(), String> {
        if self.queue.reorder(from, to) {
            self.generation += 1;
            self.persist_state();
            self.emit_state();
            Ok(())
        } else {
            Err("Invalid queue reorder indices".into())
        }
    }

    pub fn clear_queue(&mut self) -> Result<(), String> {
        let _ = self.backend.stop();
        self.queue.clear();
        self.generation += 1;
        self.sync_media_playback_state();
        self.persist_state();
        self.emit_state();
        Ok(())
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        let status_str = match self.backend.status() {
            PlaybackStatus::Idle => "idle",
            PlaybackStatus::Loading => "loading",
            PlaybackStatus::Playing => "playing",
            PlaybackStatus::Paused => "paused",
            PlaybackStatus::Stopped => "stopped",
            PlaybackStatus::Error => "error",
        };

        let current_track = self.queue.current_track().cloned();
        let duration_ms = if self.backend.duration_ms() > 0 {
            self.backend.duration_ms() as i64
        } else {
            current_track.as_ref().map(|t| t.duration_ms).unwrap_or(0)
        };

        PlaybackSnapshot {
            generation: self.generation,
            status: status_str.to_string(),
            current_track,
            position_ms: self.backend.position_ms() as i64,
            duration_ms,
            volume: self.volume,
            is_muted: self.is_muted,
            repeat_mode: self.queue.repeat_mode().as_str().to_string(),
            is_shuffle: self.queue.is_shuffle(),
            playback_speed: self.playback_speed,
            queue: self.queue.tracks().to_vec(),
            error_message: self.last_error.clone(),
            device_ready: self.backend.is_device_ready(),
        }
    }

    pub fn backend_health_status(&self) -> &'static str {
        if !self.backend.is_device_ready() {
            "device_unavailable"
        } else if self.backend.name().contains("unintegrated") {
            "not_supported"
        } else {
            "ready"
        }
    }

    /// Automatically advances to the next playable track when audio ends.
    ///
    /// Bounds advance attempts to queue length to prevent repeat-mode retry livelock.
    pub fn auto_advance(&mut self) {
        let max_attempts = self.queue.len();
        if max_attempts == 0 {
            let _ = self.backend.stop();
            self.emit_state();
            return;
        }

        if self.queue.repeat_mode() == RepeatMode::One {
            if let Some(track) = self.queue.current_track().cloned() {
                if let Err(e) = self.play_track(&track.id) {
                    eprintln!("[PlaybackService] Repeat-one failed to reload: {e}");
                    let _ = self.backend.stop();
                    self.last_error = Some(format!("Repeat-one failed: {e}"));
                    self.emit_state();
                }
            }
            return;
        }

        let mut attempts = 0;
        while attempts < max_attempts {
            attempts += 1;
            if let Some(next_track) = self.queue.next().cloned() {
                match self.play_track(&next_track.id) {
                    Ok(()) => return,
                    Err(e) => {
                        eprintln!(
                            "[PlaybackService] Skipped failed track during auto-advance: {e}"
                        );
                    }
                }
            } else {
                // End of sequential queue reached
                let _ = self.backend.stop();
                self.sync_media_playback_state();
                self.emit_state();
                return;
            }
        }

        // Exhausted all items without success: halt to prevent livelock
        let _ = self.backend.stop();
        self.last_error =
            Some("Playback halted: all upcoming tracks in queue failed to load".into());
        self.emit_state();
    }

    fn tick(&mut self) {
        if self.backend.status() == PlaybackStatus::Playing {
            if self.backend.is_ended() {
                self.auto_advance();
            } else {
                self.emit_position();
            }
        }
    }

    fn emit_state(&self) {
        if let Some(emitter) = &self.state_emitter {
            emitter(self.snapshot());
        }
    }

    fn emit_position(&self) {
        if let Some(emitter) = &self.position_emitter {
            let pos = PositionUpdate {
                generation: self.generation,
                track_id: self.queue.current_track().map(|t| t.id.clone()),
                position_ms: self.backend.position_ms() as i64,
                duration_ms: self.backend.duration_ms() as i64,
            };
            emitter(pos);
        }
    }

    fn update_media_metadata(&mut self, track: &Track) {
        if let Some(controls) = &mut self.media_controls {
            let _ = controls.set_metadata(souvlaki::MediaMetadata {
                title: Some(&track.title),
                artist: Some(&track.artist),
                album: Some(&track.album),
                duration: Some(Duration::from_millis(track.duration_ms as u64)),
                cover_url: None,
            });
            let _ = controls.set_playback(souvlaki::MediaPlayback::Playing {
                progress: Some(souvlaki::MediaPosition(Duration::from_millis(0))),
            });
        }
    }

    fn sync_media_playback_state(&mut self) {
        if let Some(controls) = &mut self.media_controls {
            let current_pos = Duration::from_millis(self.backend.position_ms());
            match self.backend.status() {
                PlaybackStatus::Playing => {
                    let _ = controls.set_playback(souvlaki::MediaPlayback::Playing {
                        progress: Some(souvlaki::MediaPosition(current_pos)),
                    });
                }
                PlaybackStatus::Paused => {
                    let _ = controls.set_playback(souvlaki::MediaPlayback::Paused {
                        progress: Some(souvlaki::MediaPosition(current_pos)),
                    });
                }
                _ => {
                    let _ = controls.set_playback(souvlaki::MediaPlayback::Stopped);
                }
            }
        }
    }
}

impl Drop for PlaybackService {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = self.backend.stop();
        self.media_controls = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioCapabilities, PlaybackError};
    use sqlx::sqlite::SqlitePoolOptions;

    struct TestMockBackend {
        status: PlaybackStatus,
        duration_ms: u64,
        position_ms: u64,
        should_fail_load: bool,
    }

    impl AudioBackend for TestMockBackend {
        fn name(&self) -> &'static str {
            "test_mock"
        }
        fn load(&mut self, _path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
            if self.should_fail_load {
                self.status = PlaybackStatus::Error;
                Err(PlaybackError::FileNotFound("test_missing.flac".into()))
            } else {
                self.status = PlaybackStatus::Playing;
                Ok(())
            }
        }
        fn play(&mut self) -> Result<(), PlaybackError> {
            self.status = PlaybackStatus::Playing;
            Ok(())
        }
        fn pause(&mut self) -> Result<(), PlaybackError> {
            self.status = PlaybackStatus::Paused;
            Ok(())
        }
        fn resume(&mut self) -> Result<(), PlaybackError> {
            self.status = PlaybackStatus::Playing;
            Ok(())
        }
        fn stop(&mut self) -> Result<(), PlaybackError> {
            self.status = PlaybackStatus::Stopped;
            Ok(())
        }
        fn seek(&mut self, pos: u64) -> Result<(), PlaybackError> {
            self.position_ms = pos;
            Ok(())
        }
        fn set_volume(&mut self, _vol: f32) -> Result<(), PlaybackError> {
            Ok(())
        }
        fn set_speed(&mut self, _spd: f32) -> Result<(), PlaybackError> {
            Ok(())
        }
        fn position_ms(&self) -> u64 {
            self.position_ms
        }
        fn duration_ms(&self) -> u64 {
            self.duration_ms
        }
        fn status(&self) -> PlaybackStatus {
            self.status
        }
        fn is_ended(&self) -> bool {
            false
        }
        fn capabilities(&self) -> AudioCapabilities {
            AudioCapabilities::unavailable()
        }
    }

    fn dummy_track(id: &str, title: &str, file_uri: Option<&str>) -> Track {
        Track {
            id: id.to_string(),
            title: title.to_string(),
            artist: "Cavia Test Artist".to_string(),
            album: "Cavia Test Album".to_string(),
            album_artist: None,
            duration_ms: 1000,
            artwork_uri: None,
            source_kind: "local".to_string(),
            provider_id: None,
            file_uri: file_uri.map(|s| s.to_string()),
            release_year: Some(2025),
            track_number: Some(1),
            disc_number: Some(1),
            genre: Some("Classical".to_string()),
            is_liked: false,
            play_count: 0,
            date_added: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_failed_track_does_not_mutate_queue_selection() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: true,
        });
        let mut svc = PlaybackService::new_for_testing(backend, pool);

        let track1 = dummy_track("1", "Missing 1", Some("C:/non_existent_file_cavia_1.flac"));
        let track2 = dummy_track("2", "Missing 2", Some("C:/non_existent_file_cavia_2.flac"));

        svc.queue
            .set_queue(vec![track1.clone(), track2.clone()], Some("1"));
        assert_eq!(svc.queue.current_index(), Some(0));

        let res = svc.play_track("2");
        assert!(res.is_err());

        // Queue selection must remain at track 1
        assert_eq!(svc.queue.current_index(), Some(0));
        assert_eq!(svc.queue.current_track().unwrap().id, "1");
        assert!(svc.last_error.is_some());
    }

    #[tokio::test]
    async fn test_repeat_livelock_terminates_cleanly_on_all_failed_tracks() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: true,
        });
        let mut svc = PlaybackService::new_for_testing(backend, pool);

        let track1 = dummy_track("1", "Broken 1", Some("C:/broken_1.flac"));
        let track2 = dummy_track("2", "Broken 2", Some("C:/broken_2.flac"));
        svc.queue.set_queue(vec![track1, track2], None);
        svc.queue.set_repeat_mode(RepeatMode::All);

        // Auto advance terminates without infinite retry loop
        svc.auto_advance();

        assert_eq!(svc.backend.status(), PlaybackStatus::Stopped);
        assert!(
            svc.last_error
                .as_ref()
                .unwrap()
                .contains("all upcoming tracks in queue failed to load")
        );
    }
}
