use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

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
    pub backend_health: String,
    pub system_media_controls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionUpdate {
    pub generation: u64,
    pub track_id: Option<String>,
    pub position_ms: i64,
    pub duration_ms: i64,
}

#[derive(Debug, Clone)]
pub struct PersistencePayload {
    queue_json: String,
    current_track_id: Option<String>,
    position_ms: i64,
    repeat_mode: String,
    shuffle_enabled: i64,
    playback_speed: f64,
    volume: i64,
    is_muted: i64,
}

/// Messages consumed by the dedicated persistence thread.
pub enum PersistenceMsg {
    State(PersistencePayload),
    Flush(std::sync::mpsc::Sender<()>),
}

type PersistenceSender = std::sync::mpsc::Sender<PersistenceMsg>;

async fn write_playback_state(pool: &SqlitePool, payload: &PersistencePayload) {
    let _ = sqlx::query(
        "INSERT INTO playback_state (id, queue_json, current_track_id, position_ms, repeat_mode, shuffle_enabled, playback_speed, volume, is_muted, updated_at) \
         VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP) \
         ON CONFLICT(id) DO UPDATE SET \
         queue_json = excluded.queue_json, \
         current_track_id = excluded.current_track_id, \
         position_ms = excluded.position_ms, \
         repeat_mode = excluded.repeat_mode, \
         shuffle_enabled = excluded.shuffle_enabled, \
         playback_speed = excluded.playback_speed, \
         volume = excluded.volume, \
         is_muted = excluded.is_muted, \
         updated_at = CURRENT_TIMESTAMP"
    )
    .bind(payload.queue_json.as_str())
    .bind(payload.current_track_id.as_deref())
    .bind(payload.position_ms)
    .bind(payload.repeat_mode.as_str())
    .bind(payload.shuffle_enabled)
    .bind(payload.playback_speed)
    .bind(payload.volume)
    .bind(payload.is_muted)
    .execute(pool)
    .await;
}

type StateEmitter = Box<dyn Fn(PlaybackSnapshot) + Send + Sync>;
type PositionEmitter = Box<dyn Fn(PositionUpdate) + Send + Sync>;

pub enum PlaybackCommand {
    PlayTrack {
        track_id: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Play {
        reply: oneshot::Sender<Result<(), String>>,
    },
    Pause {
        reply: oneshot::Sender<Result<(), String>>,
    },
    PlayPause {
        reply: oneshot::Sender<Result<(), String>>,
    },
    Stop {
        reply: oneshot::Sender<Result<(), String>>,
    },
    Seek {
        position_ms: u64,
        generation: Option<u64>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetVolume {
        volume: u32,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ToggleMute {
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetSpeed {
        speed: f32,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetRepeatMode {
        mode: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetShuffle {
        enabled: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Next {
        reply: oneshot::Sender<Result<(), String>>,
    },
    Previous {
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetQueue {
        tracks: Vec<Track>,
        start_track_id: Option<String>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    AddToQueue {
        track: Track,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RemoveFromQueue {
        track_id: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RemoveFromQueueByIndex {
        index: usize,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ReorderQueue {
        from: usize,
        to: usize,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ClearQueue {
        reply: oneshot::Sender<Result<(), String>>,
    },
    ApplyRestoredState {
        tracks: Vec<Track>,
        current_track_id: Option<String>,
        saved_position_ms: i64,
        repeat_mode: String,
        shuffle_enabled: bool,
        playback_speed: f32,
        volume: u32,
        is_muted: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetAppHandle {
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    InitMediaControls {
        hwnd: Option<isize>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RetryDevice {
        reply: oneshot::Sender<Result<(), String>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

pub struct PlaybackCore {
    backend: Box<dyn AudioBackend>,
    queue: QueueManager,
    volume: u32,
    is_muted: bool,
    previous_volume: u32,
    playback_speed: f32,
    state_emitter: Option<StateEmitter>,
    position_emitter: Option<PositionEmitter>,
    last_error: Option<String>,
    generation: u64,
    media_controls: Option<souvlaki::MediaControls>,
    media_controls_ready: bool,
    device_error: Option<String>,
    last_persist: Instant,
    last_media_sync: Instant,
    persistence_tx: Option<PersistenceSender>,
    snapshot_cache: Arc<RwLock<PlaybackSnapshot>>,
    cmd_tx: Option<std::sync::mpsc::Sender<PlaybackCommand>>,
}

impl PlaybackCore {
    pub fn new(
        backend: Box<dyn AudioBackend>,
        snapshot_cache: Arc<RwLock<PlaybackSnapshot>>,
        persistence_tx: Option<PersistenceSender>,
    ) -> Self {
        let core = Self {
            backend,
            queue: QueueManager::new(),
            volume: 85,
            is_muted: false,
            previous_volume: 85,
            playback_speed: 1.0,
            state_emitter: None,
            position_emitter: None,
            last_error: None,
            generation: 1,
            media_controls: None,
            media_controls_ready: false,
            device_error: None,
            last_persist: Instant::now(),
            last_media_sync: Instant::now(),
            persistence_tx,
            snapshot_cache,
            cmd_tx: None,
        };
        core.update_snapshot_cache();
        core
    }

    pub fn set_cmd_tx(&mut self, tx: std::sync::mpsc::Sender<PlaybackCommand>) {
        self.cmd_tx = Some(tx);
    }

    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        let app1 = app_handle.clone();
        self.state_emitter = Some(Box::new(move |snap| {
            let _ = app1.emit("playback:state-changed", snap);
        }));

        let app2 = app_handle;
        self.position_emitter = Some(Box::new(move |pos| {
            let _ = app2.emit("playback:position", pos);
        }));
    }

    pub fn init_media_controls(&mut self, hwnd: Option<isize>) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let config = souvlaki::PlatformConfig {
                dbus_name: "cavia_music",
                display_name: "Cavia Music",
                hwnd: hwnd.map(|h| h as *mut std::ffi::c_void),
            };

            match souvlaki::MediaControls::new(config) {
                Ok(mut controls) => {
                    if let Some(cmd_tx) = &self.cmd_tx {
                        let tx = cmd_tx.clone();
                        if let Err(e) = controls.attach(move |event| {
                            let (dummy_tx, _) = oneshot::channel();
                            let cmd = match event {
                                souvlaki::MediaControlEvent::Play => {
                                    PlaybackCommand::Play { reply: dummy_tx }
                                }
                                souvlaki::MediaControlEvent::Pause => {
                                    PlaybackCommand::Pause { reply: dummy_tx }
                                }
                                souvlaki::MediaControlEvent::Toggle => {
                                    PlaybackCommand::PlayPause { reply: dummy_tx }
                                }
                                souvlaki::MediaControlEvent::Next => {
                                    PlaybackCommand::Next { reply: dummy_tx }
                                }
                                souvlaki::MediaControlEvent::Previous => {
                                    PlaybackCommand::Previous { reply: dummy_tx }
                                }
                                souvlaki::MediaControlEvent::Stop => {
                                    PlaybackCommand::Stop { reply: dummy_tx }
                                }
                                souvlaki::MediaControlEvent::SetPosition(pos) => {
                                    PlaybackCommand::Seek {
                                        position_ms: pos.0.as_millis() as u64,
                                        generation: None,
                                        reply: dummy_tx,
                                    }
                                }
                                _ => return,
                            };
                            eprintln!("[PlaybackCore] SMTC command received");
                            let _ = tx.send(cmd);
                        }) {
                            let err = format!("Windows SMTC event subscription failed: {e:?}");
                            eprintln!("[PlaybackCore] {err}");
                            self.media_controls_ready = false;
                            return Err(err);
                        }
                    }
                    self.media_controls = Some(controls);
                    self.media_controls_ready = true;
                    // Publish the restored track and its paused state so
                    // Windows has an active session for media keys instead
                    // of an empty metadata/playback state.
                    if let Some(track) = self.queue.current_track().cloned() {
                        self.update_media_metadata(&track);
                    }
                    self.sync_media_playback_state();
                    Ok(())
                }
                Err(e) => {
                    let err = format!("Windows SMTC media controls initialization failed: {e:?}");
                    eprintln!("[PlaybackCore] {err}");
                    Err(err)
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = hwnd;
            self.media_controls_ready = false;
            // Native Now Playing on macOS/Linux not claimed until verified on hardware
            Ok(())
        }
    }

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
            self.update_snapshot_cache();
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
            self.update_snapshot_cache();
            self.emit_state();
            return Err(err);
        }

        // Loading succeeded: commit queue selection
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
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), String> {
        if !self.backend.is_device_ready() {
            if let Err(e) = self.backend.try_init_device() {
                let err = format!("No audio output device available: {e}");
                self.device_error = Some(err.clone());
                self.last_error = Some(err.clone());
                self.update_snapshot_cache();
                self.emit_state();
                return Err(err);
            }
            self.device_error = None;
        }

        if self.backend.status() == PlaybackStatus::Paused {
            self.backend.play().map_err(|e| e.to_string())?;
            self.sync_media_playback_state();
            self.update_snapshot_cache();
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
        self.persist_state();
        self.update_snapshot_cache();
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
        self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn seek(
        &mut self,
        position_ms: u64,
        expected_generation: Option<u64>,
    ) -> Result<(), String> {
        if let Some(exp_gen) = expected_generation {
            if exp_gen != self.generation {
                return Err(format!(
                    "Stale seek command rejected: expected generation {exp_gen}, current is {}",
                    self.generation
                ));
            }
        }

        self.backend.seek(position_ms).map_err(|e| e.to_string())?;
        self.sync_media_playback_state();
        self.persist_state();
        self.update_snapshot_cache();
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
        self.persist_state();
        self.update_snapshot_cache();
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
        self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn set_speed(&mut self, speed: f32) -> Result<(), String> {
        if !speed.is_finite() || speed < 0.25 || speed > 4.0 {
            return Err(
                "Invalid playback speed: must be a finite number between 0.25 and 4.0".into(),
            );
        }
        let clamped = speed.clamp(0.25, 4.0);
        self.playback_speed = clamped;
        self.backend.set_speed(clamped).map_err(|e| e.to_string())?;
        self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn set_repeat_mode(&mut self, mode: &str) -> Result<(), String> {
        self.queue.set_repeat_mode(RepeatMode::from(mode));
        self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn set_shuffle(&mut self, enabled: bool) -> Result<(), String> {
        self.queue.set_shuffle(enabled);
        self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn next(&mut self) -> Result<(), String> {
        let (_cand_idx, cand_track) = match self.queue.peek_next() {
            Some((idx, track)) => (idx, track.clone()),
            None => {
                let _ = self.backend.stop();
                self.sync_media_playback_state();
                self.update_snapshot_cache();
                self.emit_state();
                return Ok(());
            }
        };

        match self.play_track(&cand_track.id) {
            Ok(()) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn previous(&mut self) -> Result<(), String> {
        let pos = self.backend.position_ms();
        let (cand_idx, cand_track) = match self.queue.peek_previous(pos) {
            Some((idx, track)) => (idx, track.clone()),
            None => {
                let _ = self.backend.seek(0);
                self.emit_position();
                return Ok(());
            }
        };

        if self.queue.current_index() == Some(cand_idx) {
            let _ = self.backend.seek(0);
            self.emit_position();
            return Ok(());
        }

        self.play_track(&cand_track.id)
    }

    pub fn set_queue(
        &mut self,
        tracks: Vec<Track>,
        start_track_id: Option<&str>,
    ) -> Result<(), String> {
        if tracks.is_empty() {
            return self.clear_queue();
        }

        let target_id = start_track_id
            .map(|s| s.to_string())
            .or_else(|| tracks.first().map(|t| t.id.clone()));

        let target_idx = target_id
            .as_deref()
            .and_then(|id| tracks.iter().position(|t| t.id == id))
            .unwrap_or(0);

        // Bounded skip policy: validate the decoder, not merely file
        // existence, before replacing the live queue with a new one.
        let mut playable_idx = None;
        let mut first_error = None;
        let scan_limit = tracks.len().min(10);
        for i in 0..scan_limit {
            let idx = (target_idx + i) % tracks.len();
            let track = &tracks[idx];
            if let Some(ref uri) = track.file_uri {
                let path = Path::new(uri);
                if !path.exists() {
                    if first_error.is_none() {
                        first_error = Some(format!("Audio file not found: {uri}"));
                    }
                    continue;
                }
                match self.backend.validate_source(uri) {
                    Ok(()) => {
                        playable_idx = Some(idx);
                        break;
                    }
                    Err(e) => {
                        if first_error.is_none() {
                            first_error = Some(format!("Audio could not be decoded: {uri} ({e})"));
                        }
                    }
                }
            } else if first_error.is_none() {
                first_error = Some("Track has no file path".to_string());
            }
        }

        if let Some(idx) = playable_idx {
            let start_id = tracks[idx].id.clone();
            let previous_tracks = self.queue.tracks().to_vec();
            let previous_id = self.queue.current_track().map(|t| t.id.clone());
            self.queue.set_queue(tracks, Some(&start_id));
            self.generation += 1;
            self.persist_state();
            match self.play_track(&start_id) {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Device/output failure after validation: roll back to the
                    // previous queue and selection so the caller keeps its
                    // working state.
                    self.queue
                        .set_queue(previous_tracks, previous_id.as_deref());
                    self.generation += 1;
                    self.last_error = Some(e.clone());
                    self.update_snapshot_cache();
                    self.emit_state();
                    Err(e)
                }
            }
        } else {
            let _ = self.backend.stop();
            let err = first_error.unwrap_or_else(|| "No playable tracks in queue".to_string());
            self.last_error = Some(err.clone());
            self.generation += 1;
            self.update_snapshot_cache();
            self.emit_state();
            Err(err)
        }
    }

    pub fn add_to_queue(&mut self, track: Track) -> Result<(), String> {
        self.queue.add_track(track);
        self.generation += 1;
        self.persist_state();
        self.update_snapshot_cache();
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
        self.update_snapshot_cache();
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
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn reorder_queue(&mut self, from: usize, to: usize) -> Result<(), String> {
        if self.queue.reorder(from, to) {
            self.generation += 1;
            self.persist_state();
            self.update_snapshot_cache();
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
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn apply_restored_state(
        &mut self,
        tracks: Vec<Track>,
        current_track_id: Option<String>,
        saved_position_ms: i64,
        repeat_mode: String,
        shuffle_enabled: bool,
        playback_speed: f32,
        volume: u32,
        is_muted: bool,
    ) -> Result<(), String> {
        if !tracks.is_empty() {
            let valid_start_id = current_track_id.as_deref().and_then(|id| {
                if tracks.iter().any(|t| t.id == id) {
                    Some(id)
                } else {
                    None
                }
            });
            self.queue.set_queue(tracks, valid_start_id);
        }

        self.queue
            .set_repeat_mode(RepeatMode::from(repeat_mode.as_str()));
        self.queue.set_shuffle(shuffle_enabled);
        self.playback_speed = playback_speed.clamp(0.25, 4.0);
        let _ = self.backend.set_speed(self.playback_speed);

        self.volume = volume.min(100);
        self.is_muted = is_muted;
        let backend_vol = if self.is_muted {
            0.0
        } else {
            self.volume as f32 / 100.0
        };
        let _ = self.backend.set_volume(backend_vol);

        // Preload track at saved position in Paused state (NEVER auto-resumes playback)
        if saved_position_ms > 0 {
            if let Some(track) = self.queue.current_track().cloned() {
                if let Some(ref uri) = track.file_uri {
                    if self.backend.load(uri, saved_position_ms as u64).is_ok() {
                        let _ = self.backend.pause();
                        self.update_media_metadata(&track);
                        self.sync_media_playback_state();
                    }
                }
            }
        }

        self.generation += 1;
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn retry_device(&mut self) -> Result<(), String> {
        match self.backend.try_init_device() {
            Ok(()) => {
                self.device_error = None;
                self.last_error = None;
                self.update_snapshot_cache();
                self.emit_state();
                Ok(())
            }
            Err(e) => {
                let err = format!("Audio device reinitialization failed: {e}");
                self.device_error = Some(err.clone());
                self.last_error = Some(err.clone());
                self.update_snapshot_cache();
                self.emit_state();
                Err(err)
            }
        }
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
            backend_health: self.compute_backend_health(),
            system_media_controls: self.media_controls_ready,
        }
    }

    fn compute_backend_health(&self) -> String {
        if self.backend.name().contains("unintegrated") {
            "not_integrated".to_string()
        } else if self.device_error.is_some() {
            "error".to_string()
        } else if !self.backend.is_device_ready() {
            "device_unavailable".to_string()
        } else {
            "ready".to_string()
        }
    }

    pub fn backend_health_status(&self) -> &'static str {
        if self.backend.name().contains("unintegrated") {
            "not_integrated"
        } else if self.device_error.is_some() {
            "error"
        } else if !self.backend.is_device_ready() {
            "device_unavailable"
        } else {
            "ready"
        }
    }

    pub fn auto_advance(&mut self) {
        let max_attempts = self.queue.len();
        if max_attempts == 0 {
            let _ = self.backend.stop();
            self.update_snapshot_cache();
            self.emit_state();
            return;
        }

        if self.queue.repeat_mode() == RepeatMode::One {
            if let Some(track) = self.queue.current_track().cloned() {
                if let Err(e) = self.play_track(&track.id) {
                    eprintln!("[PlaybackCore] Repeat-one failed to reload: {e}");
                    let _ = self.backend.stop();
                    self.last_error = Some(format!("Repeat-one failed: {e}"));
                    self.update_snapshot_cache();
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
                        eprintln!("[PlaybackCore] Skipped failed track during auto-advance: {e}");
                    }
                }
            } else {
                let _ = self.backend.stop();
                self.sync_media_playback_state();
                self.update_snapshot_cache();
                self.emit_state();
                return;
            }
        }

        let _ = self.backend.stop();
        self.last_error =
            Some("Playback halted: all upcoming tracks in queue failed to load".into());
        self.update_snapshot_cache();
        self.emit_state();
    }

    pub fn tick(&mut self) {
        if self.backend.status() == PlaybackStatus::Playing {
            if self.backend.is_ended() {
                self.auto_advance();
            } else {
                self.update_snapshot_cache();
                self.emit_position();
                // Bounded periodic persistence: at most one write every five
                // seconds while audio is actually playing. Explicit commands
                // reset this timer, so pause/stop/track-change/exit always
                // flush immediately.
                if self.last_persist.elapsed() >= Duration::from_secs(5) {
                    self.persist_state();
                }
                // Windows SMTC progress is refreshed once per second; the
                // system overlay interpolates between updates.
                if self.media_controls_ready
                    && self.last_media_sync.elapsed() >= Duration::from_secs(1)
                {
                    self.sync_media_playback_state();
                    self.last_media_sync = Instant::now();
                }
            }
        }
    }

    fn update_snapshot_cache(&self) {
        if let Ok(mut cache) = self.snapshot_cache.write() {
            *cache = self.snapshot();
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

    fn persist_state(&mut self) {
        self.last_persist = Instant::now();
        if let Some(tx) = &self.persistence_tx {
            let payload = PersistencePayload {
                queue_json: serde_json::to_string(self.queue.tracks())
                    .unwrap_or_else(|_| "[]".into()),
                current_track_id: self.queue.current_track().map(|t| t.id.clone()),
                position_ms: self.backend.position_ms() as i64,
                repeat_mode: self.queue.repeat_mode().as_str().to_string(),
                shuffle_enabled: if self.queue.is_shuffle() { 1 } else { 0 },
                playback_speed: self.playback_speed as f64,
                volume: self.volume as i64,
                is_muted: if self.is_muted { 1 } else { 0 },
            };
            let _ = tx.send(PersistenceMsg::State(payload));
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

    pub fn shutdown(&mut self) {
        // Capture the live position BEFORE stopping the backend. Rodio's
        // stop() tears down the player and zeroes its seek offset, so
        // persisting afterwards would erase the user's resume point.
        self.persist_state();
        let _ = self.backend.stop();
        self.media_controls = None;
        self.media_controls_ready = false;
        // Ask the persistence thread to flush everything queued so far and
        // wait (bounded) for the write to reach SQLite before process exit.
        if let Some(tx) = &self.persistence_tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::channel();
            if tx.send(PersistenceMsg::Flush(ack_tx)).is_ok() {
                let _ = ack_rx.recv_timeout(Duration::from_secs(3));
            }
        }
    }
}

/// Thread-safe, non-blocking handle for playback operations.
/// All synchronous audio work and decoder creation execute sequentially on the dedicated audio worker thread.
#[derive(Clone)]
pub struct PlaybackService {
    cmd_tx: std::sync::mpsc::Sender<PlaybackCommand>,
    snapshot_cache: Arc<RwLock<PlaybackSnapshot>>,
    database: SqlitePool,
    shutdown_flag: Arc<AtomicBool>,
    _worker_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    _persist_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

macro_rules! guard_shutdown {
    ($service:expr) => {
        if $service.shutdown_flag.load(Ordering::SeqCst) {
            return Err("Playback service is shutting down".to_string());
        }
    };
}

pub type SharedPlaybackService = PlaybackService;

impl PlaybackService {
    pub fn new(database: SqlitePool, app_handle: Option<AppHandle>) -> Result<Self, String> {
        let backend = create_platform_backend()
            .map_err(|e| format!("Failed to create audio backend: {e}"))?;

        let initial_snapshot = PlaybackSnapshot {
            generation: 1,
            status: "idle".to_string(),
            current_track: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 85,
            is_muted: false,
            repeat_mode: "off".to_string(),
            is_shuffle: false,
            playback_speed: 1.0,
            queue: Vec::new(),
            error_message: None,
            device_ready: backend.is_device_ready(),
            backend_health: if backend.name().contains("unintegrated") {
                "not_integrated".to_string()
            } else if backend.is_device_ready() {
                "ready".to_string()
            } else {
                "device_unavailable".to_string()
            },
            system_media_controls: false,
        };

        let snapshot_cache = Arc::new(RwLock::new(initial_snapshot));
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        // Dedicated persistence thread with its own current-thread runtime.
        // This avoids depending on an ambient Tokio context during Tauri's
        // synchronous setup and gives shutdown a joinable owner.
        let (persist_tx, persist_rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let pool_for_persist = database.clone();
        let persist_thread = std::thread::Builder::new()
            .name("cavia-persist".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        eprintln!("[PlaybackService] Persistence runtime failed: {e}");
                        return;
                    }
                };

                while let Ok(message) = persist_rx.recv() {
                    match message {
                        PersistenceMsg::State(mut payload) => {
                            // Debounce: collapse all newer states into the
                            // latest one, but preserve an in-flight flush ack.
                            let mut flush_ack = None;
                            loop {
                                match persist_rx.try_recv() {
                                    Ok(PersistenceMsg::State(newer)) => payload = newer,
                                    Ok(PersistenceMsg::Flush(ack)) => {
                                        flush_ack = Some(ack);
                                        break;
                                    }
                                    Err(_) => break,
                                }
                            }
                            runtime.block_on(write_playback_state(&pool_for_persist, &payload));
                            if let Some(ack) = flush_ack {
                                let _ = ack.send(());
                            }
                        }
                        PersistenceMsg::Flush(ack) => {
                            let _ = ack.send(());
                        }
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn persistence worker thread: {e}"))?;

        let mut core = PlaybackCore::new(backend, Arc::clone(&snapshot_cache), Some(persist_tx));

        if let Some(app) = app_handle {
            core.set_app_handle(app);
        }

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<PlaybackCommand>();
        core.set_cmd_tx(cmd_tx.clone());

        let shutdown_worker = Arc::clone(&shutdown_flag);
        let worker_join = std::thread::Builder::new()
            .name("cavia-audio-control".into())
            .spawn(move || {
                let tick_interval = Duration::from_millis(250);
                while !shutdown_worker.load(Ordering::Relaxed) {
                    match cmd_rx.recv_timeout(tick_interval) {
                        Ok(cmd) => match cmd {
                            PlaybackCommand::PlayTrack { track_id, reply } => {
                                let _ = reply.send(core.play_track(&track_id));
                            }
                            PlaybackCommand::Play { reply } => {
                                let _ = reply.send(core.play());
                            }
                            PlaybackCommand::Pause { reply } => {
                                let _ = reply.send(core.pause());
                            }
                            PlaybackCommand::PlayPause { reply } => {
                                let _ = reply.send(core.play_pause());
                            }
                            PlaybackCommand::Stop { reply } => {
                                let _ = reply.send(core.stop());
                            }
                            PlaybackCommand::Seek {
                                position_ms,
                                generation,
                                reply,
                            } => {
                                let _ = reply.send(core.seek(position_ms, generation));
                            }
                            PlaybackCommand::SetVolume { volume, reply } => {
                                let _ = reply.send(core.set_volume(volume));
                            }
                            PlaybackCommand::ToggleMute { reply } => {
                                let _ = reply.send(core.toggle_mute());
                            }
                            PlaybackCommand::SetSpeed { speed, reply } => {
                                let _ = reply.send(core.set_speed(speed));
                            }
                            PlaybackCommand::SetRepeatMode { mode, reply } => {
                                let _ = reply.send(core.set_repeat_mode(&mode));
                            }
                            PlaybackCommand::SetShuffle { enabled, reply } => {
                                let _ = reply.send(core.set_shuffle(enabled));
                            }
                            PlaybackCommand::Next { reply } => {
                                let _ = reply.send(core.next());
                            }
                            PlaybackCommand::Previous { reply } => {
                                let _ = reply.send(core.previous());
                            }
                            PlaybackCommand::SetQueue {
                                tracks,
                                start_track_id,
                                reply,
                            } => {
                                let _ =
                                    reply.send(core.set_queue(tracks, start_track_id.as_deref()));
                            }
                            PlaybackCommand::AddToQueue { track, reply } => {
                                let _ = reply.send(core.add_to_queue(track));
                            }
                            PlaybackCommand::RemoveFromQueue { track_id, reply } => {
                                let _ = reply.send(core.remove_from_queue(&track_id));
                            }
                            PlaybackCommand::RemoveFromQueueByIndex { index, reply } => {
                                let _ = reply.send(core.remove_from_queue_by_index(index));
                            }
                            PlaybackCommand::ReorderQueue { from, to, reply } => {
                                let _ = reply.send(core.reorder_queue(from, to));
                            }
                            PlaybackCommand::ClearQueue { reply } => {
                                let _ = reply.send(core.clear_queue());
                            }
                            PlaybackCommand::ApplyRestoredState {
                                tracks,
                                current_track_id,
                                saved_position_ms,
                                repeat_mode,
                                shuffle_enabled,
                                playback_speed,
                                volume,
                                is_muted,
                                reply,
                            } => {
                                let _ = reply.send(core.apply_restored_state(
                                    tracks,
                                    current_track_id,
                                    saved_position_ms,
                                    repeat_mode,
                                    shuffle_enabled,
                                    playback_speed,
                                    volume,
                                    is_muted,
                                ));
                            }
                            PlaybackCommand::SetAppHandle { app_handle, reply } => {
                                core.set_app_handle(app_handle);
                                let _ = reply.send(Ok(()));
                            }
                            PlaybackCommand::InitMediaControls { hwnd, reply } => {
                                let _ = reply.send(core.init_media_controls(hwnd));
                            }
                            PlaybackCommand::RetryDevice { reply } => {
                                let _ = reply.send(core.retry_device());
                            }
                            PlaybackCommand::Shutdown { reply } => {
                                core.shutdown();
                                let _ = reply.send(());
                                break;
                            }
                        },
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            core.tick();
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            core.shutdown();
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn audio worker thread: {e}"))?;

        Ok(Self {
            cmd_tx,
            snapshot_cache,
            database,
            shutdown_flag,
            _worker_thread: Arc::new(Mutex::new(Some(worker_join))),
            _persist_thread: Arc::new(Mutex::new(Some(persist_thread))),
        })
    }

    pub fn set_app_handle(&self, app_handle: AppHandle) {
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return;
        }
        let (tx, _rx) = oneshot::channel();
        let _ = self.cmd_tx.send(PlaybackCommand::SetAppHandle {
            app_handle,
            reply: tx,
        });
    }

    pub async fn init_media_controls(&self, hwnd: Option<isize>) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::InitMediaControls { hwnd, reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn play_track(&self, track_id: &str) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::PlayTrack {
                track_id: track_id.to_string(),
                reply: tx,
            })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn play(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::Play { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn pause(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::Pause { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn play_pause(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::PlayPause { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn stop(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::Stop { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn seek(&self, position_ms: u64, generation: Option<u64>) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::Seek {
                position_ms,
                generation,
                reply: tx,
            })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_volume(&self, volume: u32) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::SetVolume { volume, reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn toggle_mute(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::ToggleMute { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_speed(&self, speed: f32) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::SetSpeed { speed, reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_repeat_mode(&self, mode: &str) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::SetRepeatMode {
                mode: mode.to_string(),
                reply: tx,
            })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_shuffle(&self, enabled: bool) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::SetShuffle { enabled, reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn next(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::Next { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn previous(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::Previous { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_queue(
        &self,
        tracks: Vec<Track>,
        start_track_id: Option<&str>,
    ) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::SetQueue {
                tracks,
                start_track_id: start_track_id.map(|s| s.to_string()),
                reply: tx,
            })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn add_to_queue(&self, track: Track) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::AddToQueue { track, reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn remove_from_queue(&self, track_id: &str) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::RemoveFromQueue {
                track_id: track_id.to_string(),
                reply: tx,
            })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn remove_from_queue_by_index(&self, index: usize) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::RemoveFromQueueByIndex { index, reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn reorder_queue(&self, from: usize, to: usize) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::ReorderQueue {
                from,
                to,
                reply: tx,
            })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn clear_queue(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::ClearQueue { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn restore_saved_state(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let row_opt = sqlx::query(
            "SELECT queue_json, current_track_id, position_ms, repeat_mode, shuffle_enabled, playback_speed, volume, is_muted \
             FROM playback_state WHERE id = 1",
        )
        .fetch_optional(&self.database)
        .await
        .map_err(|e| format!("Failed to read saved playback state: {e}"))?;

        if let Some(row) = row_opt {
            let queue_json: String = row.try_get("queue_json").unwrap_or_else(|_| "[]".into());
            let current_track_id: Option<String> = row.try_get("current_track_id").ok();
            let saved_position_ms: i64 = row.try_get("position_ms").unwrap_or(0);
            let repeat_mode: String = row.try_get("repeat_mode").unwrap_or_else(|_| "off".into());
            let shuffle_enabled: i64 = row.try_get("shuffle_enabled").unwrap_or(0);
            let playback_speed: f64 = row.try_get("playback_speed").unwrap_or(1.0);
            let volume: i64 = row.try_get("volume").unwrap_or(85);
            let is_muted: i64 = row.try_get("is_muted").unwrap_or(0);

            let mut valid_tracks = Vec::new();
            if let Ok(tracks) = serde_json::from_str::<Vec<Track>>(&queue_json) {
                for t in tracks {
                    if let Some(ref uri) = t.file_uri {
                        if Path::new(uri).exists() {
                            valid_tracks.push(t);
                        }
                    }
                }
            }

            let (tx, rx) = oneshot::channel();
            self.cmd_tx
                .send(PlaybackCommand::ApplyRestoredState {
                    tracks: valid_tracks,
                    current_track_id,
                    saved_position_ms,
                    repeat_mode,
                    shuffle_enabled: shuffle_enabled == 1,
                    playback_speed: playback_speed as f32,
                    volume: volume as u32,
                    is_muted: is_muted == 1,
                    reply: tx,
                })
                .map_err(|_| "Audio worker thread is offline".to_string())?;
            rx.await
                .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())??;
        }

        Ok(())
    }

    pub async fn retry_device(&self) -> Result<(), String> {
        guard_shutdown!(self);
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PlaybackCommand::RetryDevice { reply: tx })
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot_cache.read().unwrap().clone()
    }

    pub fn backend_health_status(&self) -> String {
        self.snapshot().backend_health
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }

    fn join_workers(&self) {
        if let Ok(mut slot) = self._worker_thread.lock() {
            if let Some(handle) = slot.take() {
                let _ = handle.join();
            }
        }
        if let Ok(mut slot) = self._persist_thread.lock() {
            if let Some(handle) = slot.take() {
                let _ = handle.join();
            }
        }
    }

    pub async fn shutdown(&self) {
        let already_shutting_down = self.shutdown_flag.swap(true, Ordering::SeqCst);
        if already_shutting_down {
            self.join_workers();
            return;
        }

        // Commands already in the channel are processed before Shutdown
        // (FIFO), so their replies are defined rather than dropped.
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(PlaybackCommand::Shutdown { reply: tx })
            .is_ok()
        {
            let _ = rx.await;
        }
        self.join_workers();
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::audio::{AudioCapabilities, PlaybackError};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    pub struct TestMockBackend {
        pub status: PlaybackStatus,
        pub duration_ms: u64,
        pub position_ms: u64,
        pub should_fail_load: bool,
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

    pub fn dummy_track(id: &str, title: &str, file_uri: Option<&str>) -> Track {
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

    pub fn new_test_core(backend: Box<dyn AudioBackend>) -> PlaybackCore {
        let initial_snapshot = PlaybackSnapshot {
            generation: 1,
            status: "idle".to_string(),
            current_track: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 85,
            is_muted: false,
            repeat_mode: "off".to_string(),
            is_shuffle: false,
            playback_speed: 1.0,
            queue: Vec::new(),
            error_message: None,
            device_ready: true,
            backend_health: "ready".to_string(),
            system_media_controls: false,
        };
        let snapshot_cache = Arc::new(RwLock::new(initial_snapshot));
        PlaybackCore::new(backend, snapshot_cache, None)
    }

    #[tokio::test]
    async fn test_failed_track_does_not_mutate_queue_selection() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: true,
        });
        let mut core = new_test_core(backend);

        let track1 = dummy_track("1", "Missing 1", Some("C:/non_existent_file_cavia_1.flac"));
        let track2 = dummy_track("2", "Missing 2", Some("C:/non_existent_file_cavia_2.flac"));

        core.queue
            .set_queue(vec![track1.clone(), track2.clone()], Some("1"));
        assert_eq!(core.queue.current_index(), Some(0));

        let res = core.play_track("2");
        assert!(res.is_err());

        // Queue selection must remain at track 1
        assert_eq!(core.queue.current_index(), Some(0));
        assert_eq!(core.queue.current_track().unwrap().id, "1");
        assert!(core.last_error.is_some());
    }

    #[tokio::test]
    async fn test_repeat_livelock_terminates_cleanly_on_all_failed_tracks() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: true,
        });
        let mut core = new_test_core(backend);

        let track1 = dummy_track("1", "Broken 1", Some("C:/broken_1.flac"));
        let track2 = dummy_track("2", "Broken 2", Some("C:/broken_2.flac"));
        core.queue.set_queue(vec![track1, track2], None);
        core.queue.set_repeat_mode(RepeatMode::All);

        // Auto advance terminates without infinite retry loop
        core.auto_advance();

        assert_eq!(core.backend.status(), PlaybackStatus::Stopped);
        assert!(
            core.last_error
                .as_ref()
                .unwrap()
                .contains("all upcoming tracks in queue failed to load")
        );
    }

    #[tokio::test]
    async fn test_next_failure_preserves_previous_selection() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_next_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 1.0);

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let track1 = dummy_track("1", "Good 1", Some(good_wav.to_str().unwrap()));
        let track2 = dummy_track("2", "Missing 2", Some("C:/non_existent_cavia_track_2.flac"));

        core.queue
            .set_queue(vec![track1.clone(), track2.clone()], Some("1"));
        assert_eq!(core.queue.current_index(), Some(0));

        let _ = core.play_track("1");
        assert_eq!(core.queue.current_index(), Some(0));

        // Next attempts to play track 2, which is missing
        let res = core.next();
        assert!(res.is_err());

        // Queue selection MUST remain at track 1 (index 0)
        assert_eq!(core.queue.current_index(), Some(0));
        assert_eq!(core.queue.current_track().unwrap().id, "1");
        assert!(core.last_error.is_some());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_previous_failure_preserves_current_selection() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_prev_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 1.0);

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let track1 = dummy_track("1", "Missing 1", Some("C:/non_existent_cavia_track_1.flac"));
        let track2 = dummy_track("2", "Good 2", Some(good_wav.to_str().unwrap()));

        core.queue
            .set_queue(vec![track1.clone(), track2.clone()], Some("2"));
        assert_eq!(core.queue.current_index(), Some(1));

        let res = core.previous();
        assert!(res.is_err());

        // Queue selection MUST remain at track 2 (index 1)
        assert_eq!(core.queue.current_index(), Some(1));
        assert_eq!(core.queue.current_track().unwrap().id, "2");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_set_queue_bounded_skip_on_broken_first_track() {
        let temp_dir = std::env::temp_dir().join(format!("cavia_test_sq_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 1.0);

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let track1 = dummy_track("1", "Missing 1", Some("C:/non_existent_1.flac"));
        let track2 = dummy_track("2", "Good 2", Some(good_wav.to_str().unwrap()));

        // set_queue with track 1 as start should bounded-skip to track 2
        let res = core.set_queue(vec![track1, track2], Some("1"));
        assert!(res.is_ok());

        assert_eq!(core.queue.current_index(), Some(1));
        assert_eq!(core.queue.current_track().unwrap().id, "2");
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_persistence_round_trip_integration() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let service = PlaybackService::new(pool.clone(), None).unwrap();

        let temp_dir =
            std::env::temp_dir().join(format!("cavia_persist_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_wav = temp_dir.join("persist.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&test_wav, 5.0);

        let track = dummy_track("t1", "Persist Track", Some(test_wav.to_str().unwrap()));

        // Add track, pause, and seek to 1500ms
        service
            .set_queue(vec![track.clone()], Some("t1"))
            .await
            .unwrap();
        service.pause().await.unwrap();
        service.seek(1500, None).await.unwrap();
        service.set_speed(1.25).await.unwrap();
        service.set_volume(72).await.unwrap();
        service.set_repeat_mode("all").await.unwrap();
        service.set_shuffle(true).await.unwrap();

        // Give persistence worker time to commit debounced write
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Restore into a fresh service instance
        let service2 = PlaybackService::new(pool.clone(), None).unwrap();
        service2.restore_saved_state().await.unwrap();

        let snap = service2.snapshot();
        assert_eq!(snap.volume, 72);
        assert_eq!(snap.playback_speed, 1.25);
        assert_eq!(snap.repeat_mode, "all");
        assert!(snap.is_shuffle);
        assert_eq!(snap.current_track.unwrap().id, "t1");
        assert_eq!(snap.position_ms, 1500);
        // Playback must NOT auto-resume; should be paused or idle
        assert_ne!(snap.status, "playing");

        service.shutdown().await;
        service2.shutdown().await;
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_tick_persists_position_while_playing() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Playing,
            duration_ms: 180_000,
            position_ms: 4_200,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        let (tx, rx) = std::sync::mpsc::channel();
        core.persistence_tx = Some(tx);
        core.last_persist = Instant::now() - Duration::from_secs(6);

        core.tick();

        match rx
            .recv_timeout(Duration::from_secs(1))
            .expect("no persistence message")
        {
            PersistenceMsg::State(payload) => assert_eq!(payload.position_ms, 4_200),
            PersistenceMsg::Flush(_) => panic!("unexpected flush message"),
        }
    }

    #[test]
    fn test_set_queue_preserves_previous_queue_on_decoder_failure() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_corrupt_queue_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let corrupt = temp_dir.join("corrupt.wav");
        std::fs::write(&corrupt, b"RIFF-not-a-valid-wave-file").unwrap();

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);
        let old1 = dummy_track("old1", "Old 1", Some("C:/non_existent_old_1.flac"));
        let old2 = dummy_track("old2", "Old 2", Some("C:/non_existent_old_2.flac"));
        core.queue.set_queue(vec![old1, old2], Some("old2"));
        assert_eq!(core.queue.current_track().unwrap().id, "old2");

        let corrupt_track = dummy_track("bad", "Corrupt", Some(corrupt.to_str().unwrap()));
        let res = core.set_queue(vec![corrupt_track], Some("bad"));

        assert!(res.is_err());
        assert_eq!(core.queue.len(), 2);
        assert_eq!(core.queue.current_track().unwrap().id, "old2");
        assert!(
            core.last_error
                .as_ref()
                .unwrap()
                .contains("could not be decoded"),
            "unexpected error: {:?}",
            core.last_error
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    async fn temp_pool(dir: &std::path::Path) -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(dir.join("test.sqlite3"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_actor_concurrent_commands_and_shutdown_are_bounded() {
        let dir = std::env::temp_dir().join(format!("cavia_actor_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool, None).unwrap();

        let mut handles = Vec::new();
        for i in 0..200u32 {
            let handle = service.clone();
            handles.push(tokio::spawn(async move {
                let _ = match i % 5 {
                    0 => handle.set_volume(i % 101).await,
                    1 => handle.pause().await,
                    2 => handle.seek(0, None).await,
                    3 => handle.play_pause().await,
                    _ => handle.set_speed(1.0).await,
                };
            }));
        }

        let shutdown = {
            let handle = service.clone();
            tokio::spawn(async move { handle.shutdown().await })
        };

        for task in handles {
            tokio::time::timeout(Duration::from_secs(10), task)
                .await
                .expect("a playback command hung")
                .unwrap();
        }
        tokio::time::timeout(Duration::from_secs(10), shutdown)
            .await
            .expect("shutdown hung")
            .unwrap();

        assert!(service.is_shutting_down());
        let rejected = service.set_volume(10).await;
        assert!(rejected.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_flushes_final_state_to_sqlite() {
        let dir = std::env::temp_dir().join(format!("cavia_flush_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        service.set_volume(73).await.unwrap();
        service.set_repeat_mode("all").await.unwrap();
        service.shutdown().await;

        let row: (i64, String) =
            sqlx::query_as("SELECT volume, repeat_mode FROM playback_state WHERE id = 1")
                .fetch_one(&pool)
                .await
                .expect("final state was not flushed to SQLite");
        assert_eq!(row.0, 73);
        assert_eq!(row.1, "all");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
