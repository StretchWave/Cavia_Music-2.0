use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use crate::audio::{AudioBackend, PlaybackStatus, create_platform_backend};
use crate::domain::Track;
use crate::services::connect::ConnectService;
use crate::services::library::base_song_title;
use crate::services::offline::{OfflineFallbackService, probe_internet_connectivity};
use crate::services::preload::{PreloadCoordinator, is_connectivity_error};
use crate::services::queue::{QueueManager, RepeatMode, SavedQueueSnapshot};
use crate::services::ytmusic::{YtMusicService, YtMusicTrack};

fn is_playable_source_available(file_path: &str) -> bool {
    if file_path.starts_with("content://") {
        return true;
    }
    if let Ok(parsed) = url::Url::parse(file_path)
        && parsed.scheme() == "file"
        && let Ok(local_path) = parsed.to_file_path()
    {
        return local_path.exists();
    }
    Path::new(file_path).exists()
}

fn ytmusic_video_id(track: &Track) -> Option<&str> {
    if track.source_kind != "provider" {
        return None;
    }
    let is_ytmusic =
        track.provider_id.as_deref() == Some("youtube_music") || track.id.starts_with("ytmusic:");
    if !is_ytmusic {
        return None;
    }
    let raw = track.id.strip_prefix("ytmusic:")?;
    let valid = raw.len() == 11
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valid.then_some(raw)
}

fn ytmusic_track_to_domain(item: YtMusicTrack) -> Track {
    let artist_name = if item.artist.trim().is_empty() {
        "Unknown artist".to_string()
    } else {
        item.artist
    };
    Track {
        id: format!("ytmusic:{}", item.video_id),
        title: item.title,
        artist: artist_name.clone(),
        album: item.album,
        album_artist: Some(artist_name),
        duration_ms: item.duration_ms.max(0),
        artwork_uri: item.artwork_url,
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
    }
}

fn local_cover_url(artwork_uri: Option<&str>) -> Option<String> {
    let raw = artwork_uri?.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.starts_with("file://") {
        return Some(raw.to_string());
    }
    let path = Path::new(raw);
    if path.exists() {
        url::Url::from_file_path(path).ok().map(String::from)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub generation: u64,
    pub status: String,
    pub current_track: Option<Track>,
    pub current_index: Option<usize>,
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
    pub smtc_status: String,
    pub smtc_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionUpdate {
    pub generation: u64,
    pub track_id: Option<String>,
    pub position_ms: i64,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceHealthStatus {
    Healthy,
    WritePending,
    WriteFailed,
    WorkerUnavailable,
    ShuttingDown,
}

impl PersistenceHealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::WritePending => "write_pending",
            Self::WriteFailed => "write_failed",
            Self::WorkerUnavailable => "worker_unavailable",
            Self::ShuttingDown => "shutting_down",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceHealthInfo {
    pub status: PersistenceHealthStatus,
    pub last_error: Option<String>,
}

impl Default for PersistenceHealthInfo {
    fn default() -> Self {
        Self {
            status: PersistenceHealthStatus::Healthy,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmtcStatus {
    Ready,
    Unavailable,
    Unsupported,
    Error,
}

impl SmtcStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmtcHealthInfo {
    pub status: SmtcStatus,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaMetadataDTO {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    pub cover_url: Option<String>,
}

pub trait MediaControlsHost: Send {
    fn attach(
        &mut self,
        event_handler: Box<dyn Fn(souvlaki::MediaControlEvent) + Send + 'static>,
    ) -> Result<(), String>;
    fn detach(&mut self) -> Result<(), String>;
    fn set_playback(&mut self, playback: souvlaki::MediaPlayback) -> Result<(), String>;
    fn set_metadata(&mut self, metadata: MediaMetadataDTO) -> Result<(), String>;
}

pub struct SouvlakiMediaControlsHost {
    controls: souvlaki::MediaControls,
}

impl SouvlakiMediaControlsHost {
    pub fn new(controls: souvlaki::MediaControls) -> Self {
        Self { controls }
    }
}

impl MediaControlsHost for SouvlakiMediaControlsHost {
    fn attach(
        &mut self,
        event_handler: Box<dyn Fn(souvlaki::MediaControlEvent) + Send + 'static>,
    ) -> Result<(), String> {
        self.controls
            .attach(move |event| event_handler(event))
            .map_err(|e| format!("Windows SMTC event subscription failed: {e:?}"))
    }

    fn detach(&mut self) -> Result<(), String> {
        self.controls
            .detach()
            .map_err(|e| format!("Windows SMTC detach failed: {e:?}"))
    }

    fn set_playback(&mut self, playback: souvlaki::MediaPlayback) -> Result<(), String> {
        self.controls
            .set_playback(playback)
            .map_err(|e| format!("SMTC set_playback error: {e:?}"))
    }

    fn set_metadata(&mut self, metadata: MediaMetadataDTO) -> Result<(), String> {
        self.controls
            .set_metadata(souvlaki::MediaMetadata {
                title: metadata.title.as_deref(),
                artist: metadata.artist.as_deref(),
                album: metadata.album.as_deref(),
                duration: metadata.duration,
                cover_url: metadata.cover_url.as_deref(),
            })
            .map_err(|e| format!("SMTC set_metadata error: {e:?}"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistencePayload {
    pub queue_json: String,
    pub current_track_id: Option<String>,
    pub position_ms: i64,
    pub repeat_mode: String,
    pub shuffle_enabled: i64,
    pub playback_speed: f64,
    pub volume: i64,
    pub is_muted: i64,
}

/// Messages consumed by the dedicated persistence thread.
pub enum PersistenceMsg {
    State(PersistencePayload),
    Flush(std::sync::mpsc::Sender<Result<(), String>>),
    Shutdown(std::sync::mpsc::Sender<Result<(), String>>),
}

type PersistenceSender = std::sync::mpsc::Sender<PersistenceMsg>;

pub async fn write_playback_state(
    pool: &SqlitePool,
    payload: &PersistencePayload,
) -> Result<(), sqlx::Error> {
    sqlx::query(
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
    .await?;
    Ok(())
}

type StateEmitter = Box<dyn Fn(PlaybackSnapshot) + Send + Sync>;
type PositionEmitter = Box<dyn Fn(PositionUpdate) + Send + Sync>;

pub enum ProviderWork {
    ResolveTrack {
        generation: u64,
        index: usize,
        track: Track,
    },
    FetchRadio {
        generation: u64,
        video_id: String,
        seen_ids: HashSet<String>,
        seen_titles: HashSet<(String, String)>,
    },
}

#[derive(Debug, Clone)]
pub enum CoordinatorEvent {
    QueueSnapshotChanged {
        current: Option<Track>,
        upcoming: Vec<Track>,
    },
    TrackStarted {
        track: Track,
    },
    TrackResolutionFinished {
        error: Option<String>,
    },
    PlaybackAdvanced,
    FallbackExhausted,
    RequestRestoreOriginalQueue,
}

pub enum PlaybackCommand {
    PlayTrack {
        track_id: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    PlayQueueIndex {
        index: usize,
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
    SetMixedQueue {
        tracks: Vec<Track>,
        start_index: Option<usize>,
        enable_radio: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    AddToQueue {
        track: Track,
        reply: oneshot::Sender<Result<(), String>>,
    },
    PlayNext {
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
    SetResolverTx {
        tx: tokio::sync::mpsc::UnboundedSender<ProviderWork>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    FinishTrackResolution {
        generation: u64,
        index: usize,
        track_id: String,
        result: Result<String, String>,
    },
    FinishRadioFetch {
        generation: u64,
        result: Result<Vec<Track>, String>,
    },
    InitMediaControls {
        hwnd: Option<isize>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetMediaControlsHost {
        host: Box<dyn MediaControlsHost>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RetryDevice {
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetCoordinatorTx {
        tx: tokio::sync::mpsc::UnboundedSender<CoordinatorEvent>,
        fallback_restore_armed: Arc<AtomicBool>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    QueueSnapshot {
        reply: oneshot::Sender<Result<SavedQueueSnapshot, String>>,
    },
    ApplyCollaborativeSequence {
        current: Option<Track>,
        upcoming: Vec<Track>,
        start_if_idle: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    EnterOfflineFallback {
        fallback_tracks: Vec<Track>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ExitOfflineFallback {
        saved: SavedQueueSnapshot,
        played_entry_ids: HashSet<String>,
        played_track_ids: HashSet<String>,
        auto_play: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SetConnectMetadataTag {
        tag: Option<String>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Shutdown {
        reply: oneshot::Sender<Result<(), String>>,
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
    loading_generation: Option<u64>,
    radio_enabled: bool,
    radio_seed_video_id: Option<String>,
    radio_seen_ids: HashSet<String>,
    radio_seen_titles: HashSet<(String, String)>,
    resolver_tx: Option<tokio::sync::mpsc::UnboundedSender<ProviderWork>>,
    media_controls_host: Option<Box<dyn MediaControlsHost>>,
    media_controls_ready: bool,
    smtc_status: SmtcStatus,
    smtc_error: Option<String>,
    device_error: Option<String>,
    last_device_retry: Instant,
    last_persist: Instant,
    last_media_sync: Instant,
    last_backend_status: PlaybackStatus,
    persistence_tx: Option<PersistenceSender>,
    snapshot_cache: Arc<RwLock<PlaybackSnapshot>>,
    cmd_tx: Option<std::sync::mpsc::Sender<PlaybackCommand>>,
    restored_position_ms: Option<i64>,
    coordinator_tx: Option<tokio::sync::mpsc::UnboundedSender<CoordinatorEvent>>,
    fallback_active: bool,
    fallback_restore_armed: Arc<AtomicBool>,
    last_window_signature: Mutex<Vec<String>>,
    connect_metadata_tag: Option<String>,
}

fn format_album_for_os_metadata(base_album: &str, connect_tag: Option<&str>) -> String {
    match connect_tag.map(str::trim).filter(|s| !s.is_empty()) {
        Some(id) => {
            if base_album.trim().is_empty() {
                format!("Cavia Connect · {id}")
            } else {
                format!("{} · Connect: {id}", base_album.trim())
            }
        }
        None => base_album.to_string(),
    }
}

impl PlaybackCore {
    pub fn new(
        backend: Box<dyn AudioBackend>,
        snapshot_cache: Arc<RwLock<PlaybackSnapshot>>,
        persistence_tx: Option<PersistenceSender>,
    ) -> Self {
        let initial_smtc_status = if cfg!(target_os = "windows") {
            SmtcStatus::Unavailable
        } else {
            SmtcStatus::Unsupported
        };
        let last_backend_status = backend.status();
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
            loading_generation: None,
            radio_enabled: false,
            radio_seed_video_id: None,
            radio_seen_ids: HashSet::new(),
            radio_seen_titles: HashSet::new(),
            resolver_tx: None,
            media_controls_host: None,
            media_controls_ready: false,
            smtc_status: initial_smtc_status,
            smtc_error: None,
            device_error: None,
            last_device_retry: Instant::now(),
            last_persist: Instant::now(),
            last_media_sync: Instant::now(),
            last_backend_status,
            persistence_tx,
            snapshot_cache,
            cmd_tx: None,
            restored_position_ms: None,
            coordinator_tx: None,
            fallback_active: false,
            fallback_restore_armed: Arc::new(AtomicBool::new(false)),
            last_window_signature: Mutex::new(Vec::new()),
            connect_metadata_tag: None,
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

    pub fn set_media_controls_host(
        &mut self,
        mut host: Box<dyn MediaControlsHost>,
    ) -> Result<(), String> {
        let tx_opt = self.cmd_tx.clone();
        if let Err(e) = host.attach(Box::new(move |event| {
            if let Some(tx) = &tx_opt {
                let (dummy_tx, _) = oneshot::channel();
                let cmd = match event {
                    souvlaki::MediaControlEvent::Play => PlaybackCommand::Play { reply: dummy_tx },
                    souvlaki::MediaControlEvent::Pause => {
                        PlaybackCommand::Pause { reply: dummy_tx }
                    }
                    souvlaki::MediaControlEvent::Toggle => {
                        PlaybackCommand::PlayPause { reply: dummy_tx }
                    }
                    souvlaki::MediaControlEvent::Next => PlaybackCommand::Next { reply: dummy_tx },
                    souvlaki::MediaControlEvent::Previous => {
                        PlaybackCommand::Previous { reply: dummy_tx }
                    }
                    souvlaki::MediaControlEvent::Stop => PlaybackCommand::Stop { reply: dummy_tx },
                    souvlaki::MediaControlEvent::SetPosition(pos) => PlaybackCommand::Seek {
                        position_ms: pos.0.as_millis() as u64,
                        generation: None,
                        reply: dummy_tx,
                    },
                    _ => return,
                };
                eprintln!("[PlaybackCore] SMTC command received");
                let _ = tx.send(cmd);
            }
        })) {
            let err = format!("Windows SMTC event subscription failed: {e}");
            eprintln!("[PlaybackCore] {err}");
            self.media_controls_ready = false;
            self.smtc_status = SmtcStatus::Unavailable;
            self.smtc_error = Some(err.clone());
            self.update_snapshot_cache();
            self.emit_state();
            return Err(err);
        }
        self.media_controls_host = Some(host);
        self.media_controls_ready = true;
        self.smtc_status = SmtcStatus::Ready;
        self.smtc_error = None;
        if let Some(track) = self.queue.current_track().cloned() {
            let _ = self.update_media_metadata(&track);
        }
        let _ = self.sync_media_playback_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
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
                Ok(controls) => {
                    let host = Box::new(SouvlakiMediaControlsHost::new(controls));
                    self.set_media_controls_host(host)
                }
                Err(e) => {
                    let err = format!("Windows SMTC media controls initialization failed: {e:?}");
                    eprintln!("[PlaybackCore] {err}");
                    self.media_controls_ready = false;
                    self.smtc_status = SmtcStatus::Unavailable;
                    self.smtc_error = Some(err.clone());
                    self.update_snapshot_cache();
                    self.emit_state();
                    Err(err)
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = hwnd;
            self.media_controls_ready = false;
            self.smtc_status = SmtcStatus::Unsupported;
            self.smtc_error = None;
            self.update_snapshot_cache();
            self.emit_state();
            // Native Now Playing on macOS/Linux not claimed until verified on hardware
            Ok(())
        }
    }

    pub fn set_resolver_tx(&mut self, tx: tokio::sync::mpsc::UnboundedSender<ProviderWork>) {
        self.resolver_tx = Some(tx);
    }

    fn effective_status(&self) -> PlaybackStatus {
        if self.loading_generation == Some(self.generation) {
            PlaybackStatus::Loading
        } else {
            self.backend.status()
        }
    }

    fn record_radio_seed(&mut self, track: &Track) {
        if let Some(vid) = ytmusic_video_id(track) {
            self.radio_seed_video_id = Some(vid.to_string());
        }
        self.radio_seen_ids.insert(track.id.clone());
        let base = base_song_title(&track.title);
        if !base.is_empty() {
            self.radio_seen_titles
                .insert((track.artist.trim().to_lowercase(), base));
        }
    }

    pub fn play_track(&mut self, track_id: &str) -> Result<(), String> {
        let index = self
            .queue
            .tracks()
            .iter()
            .position(|t| t.id == track_id)
            .ok_or_else(|| format!("Track not found in active queue: {track_id}"))?;
        self.play_index(index)
    }

    pub fn play_index(&mut self, index: usize) -> Result<(), String> {
        let track = self
            .queue
            .tracks()
            .get(index)
            .cloned()
            .ok_or_else(|| format!("Queue index out of bounds: {index}"))?;

        let has_ready_file = track
            .file_uri
            .as_deref()
            .is_some_and(is_playable_source_available);

        if !has_ready_file
            && ytmusic_video_id(&track).is_some()
            && let Some(resolver_tx) = self.resolver_tx.clone()
        {
            let _ = self.backend.stop();
            self.queue.set_current_index(index);
            self.last_error = None;
            self.restored_position_ms = None;
            self.generation += 1;
            let generation_id = self.generation;
            self.loading_generation = Some(generation_id);
            self.record_radio_seed(&track);
            let _ = self.update_media_metadata(&track);
            let _ = self.persist_state();
            self.update_snapshot_cache();
            self.emit_state();

            let _ = resolver_tx.send(ProviderWork::ResolveTrack {
                generation: generation_id,
                index,
                track,
            });
            return Ok(());
        }

        let file_path = track
            .file_uri
            .clone()
            .ok_or_else(|| "Track has no associated file path".to_string())?;

        if !is_playable_source_available(&file_path) {
            let err = format!("Audio file does not exist at path: {file_path}");
            self.last_error = Some(err.clone());
            self.loading_generation = None;
            self.update_snapshot_cache();
            self.emit_state();
            return Err(err);
        }

        // Attempt decoder loading on backend without stopping active playback first.
        // If loading fails, backend keeps previous player and session intact.
        if let Err(e) = self.backend.load(&file_path, 0) {
            let err = format!("Failed to load audio: {e}");
            self.last_error = Some(err.clone());
            self.loading_generation = None;
            self.update_snapshot_cache();
            self.emit_state();
            return Err(err);
        }

        let os_album =
            format_album_for_os_metadata(&track.album, self.connect_metadata_tag.as_deref());
        if let Err(error) = self
            .backend
            .set_metadata(&track.title, &track.artist, &os_album)
        {
            self.last_error = Some(format!(
                "Track loaded, but system media metadata could not be updated: {error}"
            ));
        } else {
            self.last_error = None;
        }

        // Loading succeeded: commit queue selection
        self.queue.set_current_index(index);
        self.loading_generation = None;
        self.restored_position_ms = None;
        self.generation += 1;
        self.record_radio_seed(&track);

        let _ = self.backend.set_volume(if self.is_muted {
            0.0
        } else {
            self.volume as f32 / 100.0
        });
        let _ = self.backend.set_speed(self.playback_speed);

        let _ = self.update_media_metadata(&track);
        if let Some(tx) = &self.coordinator_tx {
            let _ = tx.send(CoordinatorEvent::TrackStarted { track });
        }
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn finish_track_resolution(
        &mut self,
        generation: u64,
        index: usize,
        track_id: &str,
        result: Result<String, String>,
    ) {
        if generation != self.generation || self.loading_generation != Some(generation) {
            return;
        }
        self.loading_generation = None;

        match result {
            Ok(file_uri) => {
                if let Some(tx) = &self.coordinator_tx {
                    let _ = tx.send(CoordinatorEvent::TrackResolutionFinished { error: None });
                }
                if !self.queue.update_track_file_uri(index, track_id, file_uri) {
                    self.last_error = Some("Queued track moved before resolution completed".into());
                    self.update_snapshot_cache();
                    self.emit_state();
                    return;
                }
                let _ = self.play_index(index);
            }
            Err(err) => {
                if let Some(tx) = &self.coordinator_tx {
                    let _ = tx.send(CoordinatorEvent::TrackResolutionFinished {
                        error: Some(err.clone()),
                    });
                }
                self.last_error = Some(err);
                let _ = self.backend.stop();
                let _ = self.sync_media_playback_state();
                self.update_snapshot_cache();
                self.emit_state();
            }
        }
    }

    pub fn finish_radio_fetch(&mut self, generation: u64, result: Result<Vec<Track>, String>) {
        if generation != self.generation || self.loading_generation != Some(generation) {
            return;
        }
        self.loading_generation = None;

        match result {
            Ok(tracks) if !tracks.is_empty() => {
                let first_new_index = self.queue.len();
                for track in &tracks {
                    self.record_radio_seed(track);
                }
                self.queue.add_tracks(tracks);
                let _ = self.play_index(first_new_index);
            }
            Ok(_) => {
                let _ = self.backend.stop();
                let _ = self.sync_media_playback_state();
                self.update_snapshot_cache();
                self.emit_state();
            }
            Err(err) => {
                let _ = self.backend.stop();
                let _ = self.sync_media_playback_state();
                self.last_error = Some(err);
                self.update_snapshot_cache();
                self.emit_state();
            }
        }
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

        if self.effective_status() == PlaybackStatus::Loading {
            return Ok(());
        }

        if self.backend.status() == PlaybackStatus::Paused {
            self.backend.play().map_err(|e| e.to_string())?;
            let _ = self.sync_media_playback_state();
            self.update_snapshot_cache();
            self.emit_state();
            return Ok(());
        }

        if let Some(idx) = self.queue.current_index() {
            self.play_index(idx)
        } else if !self.queue.is_empty() {
            self.play_index(0)
        } else {
            Err("Cannot play: queue is empty".into())
        }
    }

    pub fn pause(&mut self) -> Result<(), String> {
        self.loading_generation = None;
        self.backend.pause().map_err(|e| e.to_string())?;
        let _ = self.sync_media_playback_state();
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn play_pause(&mut self) -> Result<(), String> {
        match self.effective_status() {
            PlaybackStatus::Playing => self.pause(),
            PlaybackStatus::Paused => self.play(),
            PlaybackStatus::Idle | PlaybackStatus::Stopped => self.play(),
            PlaybackStatus::Loading => Ok(()),
            PlaybackStatus::Error => {
                if let Some(idx) = self.queue.current_index() {
                    self.play_index(idx)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.loading_generation = None;
        self.backend.stop().map_err(|e| e.to_string())?;
        let _ = self.sync_media_playback_state();
        self.generation += 1;
        let _ = self.persist_state();
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
        self.restored_position_ms = None;
        let _ = self.sync_media_playback_state();
        let _ = self.persist_state();
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
        let _ = self.persist_state();
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
        let _ = self.persist_state();
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
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn set_repeat_mode(&mut self, mode: &str) -> Result<(), String> {
        self.queue.set_repeat_mode(RepeatMode::from(mode));
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn set_shuffle(&mut self, enabled: bool) -> Result<(), String> {
        self.queue.set_shuffle(enabled);
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    fn trigger_radio_fetch_if_enabled(&mut self) -> bool {
        if !self.radio_enabled || self.queue.repeat_mode() != RepeatMode::Off {
            return false;
        }
        let Some(resolver_tx) = self.resolver_tx.clone() else {
            return false;
        };
        let seed = self.radio_seed_video_id.clone().or_else(|| {
            self.queue
                .current_track()
                .and_then(ytmusic_video_id)
                .map(str::to_string)
        });
        let Some(video_id) = seed else {
            return false;
        };

        let _ = self.backend.stop();
        self.last_error = None;
        self.generation += 1;
        let generation_id = self.generation;
        self.loading_generation = Some(generation_id);
        self.update_snapshot_cache();
        self.emit_state();

        let _ = resolver_tx.send(ProviderWork::FetchRadio {
            generation: generation_id,
            video_id,
            seen_ids: self.radio_seen_ids.clone(),
            seen_titles: self.radio_seen_titles.clone(),
        });
        true
    }

    pub fn next(&mut self) -> Result<(), String> {
        if self.fallback_active && self.fallback_restore_armed.swap(false, Ordering::SeqCst) {
            if let Some(tx) = &self.coordinator_tx {
                let _ = tx.send(CoordinatorEvent::RequestRestoreOriginalQueue);
            }
            return Ok(());
        }
        let cand_idx = match self.queue.peek_next() {
            Some((idx, _track)) => idx,
            None => {
                if self.fallback_active {
                    self.loading_generation = None;
                    let _ = self.backend.stop();
                    let _ = self.sync_media_playback_state();
                    self.last_error = Some(
                        "Offline fallback exhausted — all verified cached songs have been played."
                            .into(),
                    );
                    if let Some(tx) = &self.coordinator_tx {
                        let _ = tx.send(CoordinatorEvent::FallbackExhausted);
                    }
                    self.update_snapshot_cache();
                    self.emit_state();
                    return Ok(());
                }
                if self.trigger_radio_fetch_if_enabled() {
                    return Ok(());
                }
                self.loading_generation = None;
                let _ = self.backend.stop();
                let _ = self.sync_media_playback_state();
                self.update_snapshot_cache();
                self.emit_state();
                return Ok(());
            }
        };

        if !self.fallback_active
            && let Some(tx) = &self.coordinator_tx
        {
            let _ = tx.send(CoordinatorEvent::PlaybackAdvanced);
        }
        self.play_index(cand_idx)
    }

    pub fn previous(&mut self) -> Result<(), String> {
        let pos = self.backend.position_ms();
        let cand_idx = match self.queue.peek_previous(pos) {
            Some((idx, _track)) => idx,
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

        self.play_index(cand_idx)
    }

    pub fn set_queue(
        &mut self,
        tracks: Vec<Track>,
        start_track_id: Option<&str>,
    ) -> Result<(), String> {
        let start_index = if let Some(id) = start_track_id {
            match tracks.iter().position(|t| t.id == id) {
                Some(idx) => Some(idx),
                None => {
                    let err = format!("Requested starting track not found in queue: {id}");
                    self.last_error = Some(err.clone());
                    self.update_snapshot_cache();
                    self.emit_state();
                    return Err(err);
                }
            }
        } else {
            None
        };
        self.set_mixed_queue(tracks, start_index, false)
    }

    pub fn set_mixed_queue(
        &mut self,
        tracks: Vec<Track>,
        start_index: Option<usize>,
        enable_radio: bool,
    ) -> Result<(), String> {
        if tracks.is_empty() {
            return self.clear_queue();
        }

        let target_idx = start_index.unwrap_or(0).min(tracks.len() - 1);
        let target_track = tracks[target_idx].clone();

        let is_unready_ytmusic = !target_track
            .file_uri
            .as_deref()
            .is_some_and(is_playable_source_available)
            && ytmusic_video_id(&target_track).is_some()
            && self.resolver_tx.is_some();

        if is_unready_ytmusic {
            self.queue.set_queue_at_index(tracks, Some(target_idx));
            self.radio_enabled = enable_radio;
            self.radio_seen_ids.clear();
            self.radio_seen_titles.clear();
            self.radio_seed_video_id = None;
            return self.play_index(target_idx);
        }

        let file_path = target_track.file_uri.clone().ok_or_else(|| {
            let err = format!("Track {} has no associated file path", target_track.id);
            self.last_error = Some(err.clone());
            self.update_snapshot_cache();
            self.emit_state();
            err
        })?;

        if !is_playable_source_available(&file_path) {
            let err = format!("Audio file does not exist at path: {file_path}");
            self.last_error = Some(err.clone());
            self.update_snapshot_cache();
            self.emit_state();
            return Err(err);
        }

        // Transactional validation: probe candidate decoder without mutating player or queue
        if let Err(e) = self.backend.validate_source(&file_path) {
            let err = format!("Audio could not be decoded: {file_path} ({e})");
            self.last_error = Some(err.clone());
            self.update_snapshot_cache();
            self.emit_state();
            return Err(err);
        }

        // Attempt candidate loading without stopping active playback first.
        // If candidate loading fails, backend keeps previous player and session intact.
        if let Err(e) = self.backend.load(&file_path, 0) {
            let err = format!("Failed to load audio: {e}");
            self.last_error = Some(err.clone());
            self.update_snapshot_cache();
            self.emit_state();
            return Err(err);
        }

        // Candidate successfully prepared: commit new queue and selection atomically
        self.queue.set_queue_at_index(tracks, Some(target_idx));
        self.loading_generation = None;
        self.radio_enabled = enable_radio;
        self.radio_seen_ids.clear();
        self.radio_seen_titles.clear();
        self.radio_seed_video_id = None;
        self.record_radio_seed(&target_track);
        self.last_error = None;
        self.restored_position_ms = None;
        self.generation += 1;

        let _ = self.backend.set_volume(if self.is_muted {
            0.0
        } else {
            self.volume as f32 / 100.0
        });
        let _ = self.backend.set_speed(self.playback_speed);

        let _ = self.update_media_metadata(&target_track);
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn add_to_queue(&mut self, track: Track) -> Result<(), String> {
        self.queue.add_track(track);
        self.generation += 1;
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn play_next(&mut self, track: Track) -> Result<(), String> {
        self.queue.insert_next(track);
        self.generation += 1;
        let _ = self.persist_state();
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
            self.loading_generation = None;
            let _ = self.backend.stop();
        }

        self.queue.remove_track(track_id);
        self.generation += 1;
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn remove_from_queue_by_index(&mut self, index: usize) -> Result<(), String> {
        let is_current = self.queue.current_index() == Some(index);

        if is_current {
            self.loading_generation = None;
            let _ = self.backend.stop();
        }

        self.queue.remove_at(index);
        self.generation += 1;
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn reorder_queue(&mut self, from: usize, to: usize) -> Result<(), String> {
        if self.queue.reorder(from, to) {
            self.generation += 1;
            let _ = self.persist_state();
            self.update_snapshot_cache();
            self.emit_state();
            Ok(())
        } else {
            Err("Invalid queue reorder indices".into())
        }
    }

    pub fn clear_queue(&mut self) -> Result<(), String> {
        self.loading_generation = None;
        self.radio_enabled = false;
        self.radio_seed_video_id = None;
        self.radio_seen_ids.clear();
        self.radio_seen_titles.clear();
        let _ = self.backend.stop();
        self.queue.clear();
        self.restored_position_ms = None;
        self.last_error = None;
        self.generation += 1;
        let _ = self.sync_media_playback_state();
        let _ = self.persist_state();
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

        self.loading_generation = None;
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

        self.restored_position_ms = if saved_position_ms > 0 {
            Some(saved_position_ms)
        } else {
            None
        };

        // Preload track at saved position in Paused state (NEVER auto-resumes playback)
        if let Some(track) = self.queue.current_track().cloned() {
            let should_report_missing = saved_position_ms > 0 || track.source_kind != "provider";
            if let Some(ref uri) = track.file_uri {
                let path = Path::new(uri);
                if !is_playable_source_available(uri) {
                    if should_report_missing {
                        let is_volume_unavailable = {
                            #[cfg(windows)]
                            {
                                let prefix = path.components().next();
                                matches!(prefix, Some(std::path::Component::Prefix(_)))
                                    && !path.parent().map(|p| p.exists()).unwrap_or(false)
                            }
                            #[cfg(not(windows))]
                            {
                                false
                            }
                        };
                        let err = if is_volume_unavailable {
                            format!("External media volume unavailable during restore: {uri}")
                        } else {
                            format!("Media file not found during restore: {uri}")
                        };
                        self.last_error = Some(err);
                    }
                } else {
                    match self
                        .backend
                        .load_paused(uri, saved_position_ms.max(0) as u64)
                    {
                        Ok(()) => {
                            self.restored_position_ms = None;
                            let _ = self.update_media_metadata(&track);
                            let _ = self.sync_media_playback_state();
                        }
                        Err(e) => {
                            if saved_position_ms > 0 {
                                self.last_error =
                                    Some(format!("Failed to load audio during restore: {e}"));
                            }
                        }
                    }
                }
            } else if saved_position_ms > 0 {
                self.last_error = Some("Restored track has no file URI".to_string());
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
        let status_str = match self.effective_status() {
            PlaybackStatus::Idle => "idle",
            PlaybackStatus::Loading => "loading",
            PlaybackStatus::Playing => "playing",
            PlaybackStatus::Paused => "paused",
            PlaybackStatus::Stopped => "stopped",
            PlaybackStatus::Error => "error",
        };

        let current_track = self.queue.current_track().cloned();
        let current_index = self.queue.current_index();
        let duration_ms = if self.backend.duration_ms() > 0 {
            self.backend.duration_ms() as i64
        } else {
            current_track.as_ref().map(|t| t.duration_ms).unwrap_or(0)
        };

        let position_ms = if current_track.is_none() {
            0
        } else if let Some(pos) = self.restored_position_ms {
            pos
        } else {
            self.backend.position_ms() as i64
        };

        PlaybackSnapshot {
            generation: self.generation,
            status: status_str.to_string(),
            current_track,
            current_index,
            position_ms,
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
            system_media_controls: self.media_controls_ready
                || (self.backend.is_device_ready()
                    && self.backend.capabilities().system_media_controls),
            smtc_status: self.smtc_status.as_str().to_string(),
            smtc_error: self.smtc_error.clone(),
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
        if self.fallback_active && self.fallback_restore_armed.swap(false, Ordering::SeqCst) {
            if let Some(tx) = &self.coordinator_tx {
                let _ = tx.send(CoordinatorEvent::RequestRestoreOriginalQueue);
            }
            return;
        }
        let max_attempts = self.queue.len();
        if max_attempts == 0 {
            let _ = self.backend.stop();
            self.update_snapshot_cache();
            self.emit_state();
            return;
        }

        if self.queue.repeat_mode() == RepeatMode::One && !self.fallback_active {
            if let Some(curr_idx) = self.queue.current_index() {
                if let Err(e) = self.play_index(curr_idx) {
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
            if self.queue.next().is_some()
                && let Some(next_idx) = self.queue.current_index()
            {
                if !self.fallback_active
                    && let Some(tx) = &self.coordinator_tx
                {
                    let _ = tx.send(CoordinatorEvent::PlaybackAdvanced);
                }
                match self.play_index(next_idx) {
                    Ok(()) => return,
                    Err(e) => {
                        eprintln!("[PlaybackCore] Skipped failed track during auto-advance: {e}");
                    }
                }
            } else {
                if self.fallback_active {
                    let _ = self.backend.stop();
                    let _ = self.sync_media_playback_state();
                    self.last_error = Some(
                        "Offline fallback exhausted — all verified cached songs have been played."
                            .into(),
                    );
                    if let Some(tx) = &self.coordinator_tx {
                        let _ = tx.send(CoordinatorEvent::FallbackExhausted);
                    }
                    self.update_snapshot_cache();
                    self.emit_state();
                    return;
                }
                if self.trigger_radio_fetch_if_enabled() {
                    return;
                }
                let _ = self.backend.stop();
                let _ = self.sync_media_playback_state();
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
        self.backend.poll();
        let backend_status = self.backend.status();
        if backend_status != self.last_backend_status {
            self.last_backend_status = backend_status;
            self.update_snapshot_cache();
            self.emit_state();
            if matches!(
                backend_status,
                PlaybackStatus::Paused | PlaybackStatus::Stopped
            ) && let Err(error) = self.persist_state()
            {
                self.last_error = Some(error);
                self.update_snapshot_cache();
            }
        }
        if !self.backend.is_device_ready() {
            if self.last_device_retry.elapsed() >= Duration::from_secs(2) {
                self.last_device_retry = Instant::now();
                if let Err(error) = self.retry_device() {
                    self.device_error = Some(error);
                }
            }
            return;
        }

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
                    let _ = self.persist_state();
                }
                // Windows SMTC progress is refreshed once per second; the
                // system overlay interpolates between updates.
                if self.media_controls_ready
                    && self.last_media_sync.elapsed() >= Duration::from_secs(1)
                {
                    let _ = self.sync_media_playback_state();
                    self.last_media_sync = Instant::now();
                }
            }
        }
    }

    fn update_snapshot_cache(&self) {
        if let Ok(mut cache) = self.snapshot_cache.write() {
            *cache = self.snapshot();
        }
        if let Some(tx) = &self.coordinator_tx {
            let current = self.queue.current_track().cloned();
            let upcoming: Vec<Track> = self
                .queue
                .upcoming_tracks_and_indices()
                .into_iter()
                .map(|(_, t)| t)
                .collect();
            let mut sig = Vec::with_capacity(1 + upcoming.len());
            if let Some(cur) = &current {
                sig.push(format!(
                    "{}:{}:{}",
                    cur.id,
                    cur.queue_entry_id.as_deref().unwrap_or(""),
                    cur.file_uri.as_deref().unwrap_or("")
                ));
            }
            for item in upcoming.iter().take(25) {
                sig.push(format!(
                    "{}:{}:{}",
                    item.id,
                    item.queue_entry_id.as_deref().unwrap_or(""),
                    item.file_uri.as_deref().unwrap_or("")
                ));
            }
            if let Ok(mut last_sig) = self.last_window_signature.lock()
                && *last_sig != sig
            {
                *last_sig = sig;
                let _ = tx.send(CoordinatorEvent::QueueSnapshotChanged { current, upcoming });
            }
        }
    }

    pub fn set_coordinator_tx(
        &mut self,
        tx: tokio::sync::mpsc::UnboundedSender<CoordinatorEvent>,
        fallback_restore_armed: Arc<AtomicBool>,
    ) {
        self.coordinator_tx = Some(tx);
        self.fallback_restore_armed = fallback_restore_armed;
        let current = self.queue.current_track().cloned();
        let upcoming: Vec<Track> = self
            .queue
            .upcoming_tracks_and_indices()
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        if let Some(sender) = &self.coordinator_tx {
            let _ = sender.send(CoordinatorEvent::QueueSnapshotChanged { current, upcoming });
        }
    }

    pub fn apply_collaborative_sequence(
        &mut self,
        current: Option<Track>,
        upcoming: Vec<Track>,
        start_if_idle: bool,
    ) -> Result<(), String> {
        let was_idle = self.queue.current_track().is_none()
            || matches!(
                self.effective_status(),
                PlaybackStatus::Idle | PlaybackStatus::Stopped
            );
        self.queue.apply_collaborative_upcoming(current, upcoming);
        if start_if_idle && was_idle && !self.queue.is_empty() {
            return self.play_index(0);
        }
        let _ = self.persist_state();
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
    }

    pub fn enter_offline_fallback(&mut self, fallback_tracks: Vec<Track>) -> Result<(), String> {
        if fallback_tracks.is_empty() {
            return Err("No verified offline tracks available".into());
        }
        let has_active_audio = matches!(
            self.backend.status(),
            PlaybackStatus::Playing | PlaybackStatus::Paused
        ) && self.queue.current_track().is_some();

        self.fallback_active = true;
        self.fallback_restore_armed.store(false, Ordering::SeqCst);
        self.queue.set_repeat_mode(RepeatMode::Off);
        self.last_error = None;

        if has_active_audio {
            // Keep currently playing/loaded song undisturbed; swap only upcoming tracks.
            let current = self.queue.current_track().cloned();
            self.queue
                .apply_collaborative_upcoming(current, fallback_tracks);
            self.update_snapshot_cache();
            self.emit_state();
            Ok(())
        } else {
            self.queue.set_queue_at_index(fallback_tracks, Some(0));
            self.play_index(0)
        }
    }

    pub fn exit_offline_fallback(
        &mut self,
        saved: SavedQueueSnapshot,
        played_entry_ids: HashSet<String>,
        played_track_ids: HashSet<String>,
        auto_play: bool,
    ) -> Result<(), String> {
        self.fallback_active = false;
        self.fallback_restore_armed.store(false, Ordering::SeqCst);
        self.queue.restore_snapshot(saved);
        self.queue
            .advance_past_played_during_fallback(&played_entry_ids, &played_track_ids);
        self.last_error = None;
        let _ = self.persist_state();
        if auto_play && let Some(idx) = self.queue.current_index() {
            return self.play_index(idx);
        }
        self.update_snapshot_cache();
        self.emit_state();
        Ok(())
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

    fn persist_state(&mut self) -> Result<(), String> {
        if self.fallback_active {
            return Ok(());
        }
        self.last_persist = Instant::now();
        if let Some(tx) = &self.persistence_tx {
            let current_track_id = self.queue.current_track().map(|track| track.id.clone());
            let position_ms = if current_track_id.is_none()
                && matches!(
                    self.backend.status(),
                    PlaybackStatus::Idle | PlaybackStatus::Stopped
                ) {
                0
            } else if let Some(pos) = self.restored_position_ms {
                pos
            } else {
                self.backend.position_ms() as i64
            };
            let payload = PersistencePayload {
                queue_json: serde_json::to_string(self.queue.tracks())
                    .unwrap_or_else(|_| "[]".into()),
                current_track_id,
                position_ms,
                repeat_mode: self.queue.repeat_mode().as_str().to_string(),
                shuffle_enabled: if self.queue.is_shuffle() { 1 } else { 0 },
                playback_speed: self.playback_speed as f64,
                volume: self.volume as i64,
                is_muted: if self.is_muted { 1 } else { 0 },
            };
            tx.send(PersistenceMsg::State(payload))
                .map_err(|e| format!("Persistence channel error: {e}"))
        } else {
            Err("Persistence worker offline".into())
        }
    }

    fn record_smtc_failure(&mut self, err: String) {
        self.media_controls_ready = false;
        self.smtc_status = SmtcStatus::Error;
        self.smtc_error = Some(err);
        self.update_snapshot_cache();
        self.emit_state();
    }

    fn record_smtc_success(&mut self) {
        if self.media_controls_host.is_some() {
            if self.smtc_status != SmtcStatus::Ready
                || self.smtc_error.is_some()
                || !self.media_controls_ready
            {
                self.media_controls_ready = true;
                self.smtc_status = SmtcStatus::Ready;
                self.smtc_error = None;
                self.update_snapshot_cache();
                self.emit_state();
            }
        }
    }

    pub fn set_connect_metadata_tag(&mut self, tag: Option<String>) -> Result<(), String> {
        self.connect_metadata_tag = tag.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        if let Some(track) = self.queue.current_track().cloned() {
            let os_album =
                format_album_for_os_metadata(&track.album, self.connect_metadata_tag.as_deref());
            let _ = self
                .backend
                .set_metadata(&track.title, &track.artist, &os_album);
            let _ = self.update_media_metadata(&track);
        }
        Ok(())
    }

    fn update_media_metadata(&mut self, track: &Track) -> Result<(), String> {
        if self.media_controls_host.is_some() {
            let cover_url = local_cover_url(track.artwork_uri.as_deref());
            let os_album =
                format_album_for_os_metadata(&track.album, self.connect_metadata_tag.as_deref());
            let metadata = MediaMetadataDTO {
                title: Some(track.title.clone()),
                artist: Some(track.artist.clone()),
                album: Some(os_album),
                duration: Some(Duration::from_millis(track.duration_ms.max(0) as u64)),
                cover_url,
            };
            let meta_res = self
                .media_controls_host
                .as_mut()
                .expect("checked is_some")
                .set_metadata(metadata);
            if let Err(e) = meta_res {
                let err = format!("SMTC set_metadata error: {e:?}");
                eprintln!("[PlaybackCore] {err}");
                self.record_smtc_failure(err.clone());
                return Err(err);
            }
            self.sync_media_playback_state()?;
        }
        Ok(())
    }

    fn sync_media_playback_state(&mut self) -> Result<(), String> {
        if let Some(controls) = &mut self.media_controls_host {
            let current_pos = Duration::from_millis(self.backend.position_ms());
            let res = match self.backend.status() {
                PlaybackStatus::Playing => {
                    controls.set_playback(souvlaki::MediaPlayback::Playing {
                        progress: Some(souvlaki::MediaPosition(current_pos)),
                    })
                }
                PlaybackStatus::Paused => controls.set_playback(souvlaki::MediaPlayback::Paused {
                    progress: Some(souvlaki::MediaPosition(current_pos)),
                }),
                _ => controls.set_playback(souvlaki::MediaPlayback::Stopped),
            };
            if let Err(e) = res {
                let err = format!("SMTC set_playback error: {e:?}");
                eprintln!("[PlaybackCore] {err}");
                self.record_smtc_failure(err.clone());
                return Err(err);
            }
            self.record_smtc_success();
        }
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), String> {
        // Phase A: Prepare shutdown. Pause playback so position cannot advance.
        if self.backend.status() == PlaybackStatus::Playing {
            let _ = self.backend.pause();
        }
        let persist_res = self.persist_state();
        let mut flush_res = Ok(());
        // Phase B: Commit shutdown. Flush and await truthful acknowledgement.
        if let Some(tx) = &self.persistence_tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::channel();
            match tx.send(PersistenceMsg::Shutdown(ack_tx)) {
                Ok(()) => match ack_rx.recv_timeout(Duration::from_secs(3)) {
                    Ok(res) => flush_res = res,
                    Err(e) => {
                        flush_res = Err(format!(
                            "Persistence shutdown flush timed out or failed: {e}"
                        ));
                    }
                },
                Err(_) => {
                    flush_res =
                        Err("Persistence worker channel disconnected during shutdown".to_string());
                }
            }
        }
        let final_res = persist_res.and(flush_res);
        if final_res.is_ok() {
            // Only stop the backend and tear down media controls after persistence succeeds
            let _ = self.backend.stop();
            if let Some(mut controls) = self.media_controls_host.take() {
                let _ = controls.detach();
            }
            self.media_controls_ready = false;
            self.smtc_status = SmtcStatus::Unavailable;
            self.smtc_error = None;
        }
        final_res
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownProgress {
    Idle,
    InProgress,
    Failed(String),
    Completed,
}

/// Thread-safe, non-blocking handle for playback operations.
/// All synchronous audio work and decoder creation execute sequentially on the dedicated audio worker thread.
#[derive(Clone)]
pub struct PlaybackService {
    cmd_tx: std::sync::mpsc::Sender<PlaybackCommand>,
    snapshot_cache: Arc<RwLock<PlaybackSnapshot>>,
    database: SqlitePool,
    backend_engine_id: &'static str,
    shutdown_flag: Arc<AtomicBool>,
    shutdown_state: Arc<Mutex<ShutdownProgress>>,
    pub shutdown_watch: Arc<tokio::sync::watch::Sender<ShutdownProgress>>,
    command_gate: Arc<std::sync::RwLock<()>>,
    persistence_health: Arc<RwLock<PersistenceHealthInfo>>,
    preload_coordinator: Arc<Mutex<Option<Arc<PreloadCoordinator>>>>,
    _worker_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    _persist_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

pub type SharedPlaybackService = PlaybackService;

pub fn run_persistence_worker(
    persist_rx: std::sync::mpsc::Receiver<PersistenceMsg>,
    pool_for_persist: SqlitePool,
    init_tx: Option<std::sync::mpsc::Sender<Result<(), String>>>,
    health: Arc<RwLock<PersistenceHealthInfo>>,
    hook: Option<Arc<dyn Fn(Result<(), &str>) + Send + Sync>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => {
            if let Some(tx) = init_tx {
                let _ = tx.send(Ok(()));
            }
            runtime
        }
        Err(e) => {
            let err = format!("Persistence runtime failed to build: {e}");
            eprintln!("[PlaybackService] {err}");
            if let Ok(mut h) = health.write() {
                h.status = PersistenceHealthStatus::WorkerUnavailable;
                h.last_error = Some(err.clone());
            }
            if let Some(tx) = init_tx {
                let _ = tx.send(Err(err));
            }
            return;
        }
    };

    let mut pending_payload: Option<PersistencePayload> = None;
    let mut last_error: Option<String> = None;

    while let Ok(initial_msg) = persist_rx.recv() {
        let mut acks: Vec<(std::sync::mpsc::Sender<Result<(), String>>, bool)> = Vec::new();
        match initial_msg {
            PersistenceMsg::State(payload) => {
                pending_payload = Some(payload);
            }
            PersistenceMsg::Flush(ack) => {
                acks.push((ack, false));
            }
            PersistenceMsg::Shutdown(ack) => {
                acks.push((ack, true));
            }
        }

        // Drain any pending messages in channel to debounce
        loop {
            match persist_rx.try_recv() {
                Ok(PersistenceMsg::State(newer)) => {
                    pending_payload = Some(newer);
                }
                Ok(PersistenceMsg::Flush(ack)) => {
                    acks.push((ack, false));
                }
                Ok(PersistenceMsg::Shutdown(ack)) => {
                    acks.push((ack, true));
                }
                Err(_) => break,
            }
        }

        let has_shutdown = acks.iter().any(|(_, is_shut)| *is_shut);

        if let Some(payload) = pending_payload.take() {
            if let Ok(mut h) = health.write() {
                h.status = PersistenceHealthStatus::WritePending;
            }
            let write_res = runtime
                .block_on(write_playback_state(&pool_for_persist, &payload))
                .map_err(|e| format!("Persistence write failed: {e}"));

            match write_res {
                Ok(()) => {
                    last_error = None;
                    if let Ok(mut h) = health.write() {
                        h.status = if has_shutdown {
                            PersistenceHealthStatus::ShuttingDown
                        } else {
                            PersistenceHealthStatus::Healthy
                        };
                        h.last_error = None;
                    }
                    if let Some(ref h) = hook {
                        h(Ok(()));
                    }
                    for (ack, _) in acks {
                        let _ = ack.send(Ok(()));
                    }
                    if has_shutdown {
                        break;
                    }
                }
                Err(err_msg) => {
                    // Retain failed payload and error
                    pending_payload = Some(payload);
                    last_error = Some(err_msg.clone());
                    if let Ok(mut h) = health.write() {
                        h.status = PersistenceHealthStatus::WriteFailed;
                        h.last_error = Some(err_msg.clone());
                    }
                    if let Some(ref h) = hook {
                        h(Err(&err_msg));
                    }
                    for (ack, _) in acks {
                        let _ = ack.send(Err(err_msg.clone()));
                    }
                    // DO NOT BREAK on shutdown failure to preserve worker for retries
                }
            }
        } else {
            // No pending payload: all prior writes succeeded or nothing was queued
            if let Ok(mut h) = health.write() {
                if has_shutdown {
                    h.status = PersistenceHealthStatus::ShuttingDown;
                } else if last_error.is_none() {
                    h.status = PersistenceHealthStatus::Healthy;
                }
            }
            for (ack, _) in acks {
                let _ = ack.send(Ok(()));
            }
            if has_shutdown {
                break;
            }
        }
    }

    // Channel disconnected: mark worker unavailable if still has pending payload
    if let Ok(mut h) = health.write() {
        if pending_payload.is_some() {
            h.status = PersistenceHealthStatus::WorkerUnavailable;
        }
    }
}

impl PlaybackService {
    pub fn new(database: SqlitePool, app_handle: Option<AppHandle>) -> Result<Self, String> {
        let backend = create_platform_backend(app_handle.clone())
            .map_err(|e| format!("Failed to create audio backend: {e}"))?;
        let backend_engine_id = backend.engine_id();
        let backend_device_ready = backend.is_device_ready();
        let backend_capabilities = backend.capabilities();

        let initial_snapshot = PlaybackSnapshot {
            generation: 1,
            status: "idle".to_string(),
            current_track: None,
            current_index: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 85,
            is_muted: false,
            repeat_mode: "off".to_string(),
            is_shuffle: false,
            playback_speed: 1.0,
            queue: Vec::new(),
            error_message: None,
            device_ready: backend_device_ready,
            backend_health: if backend.name().contains("unintegrated") {
                "not_integrated".to_string()
            } else if backend_device_ready {
                "ready".to_string()
            } else {
                "device_unavailable".to_string()
            },
            system_media_controls: backend_device_ready
                && backend_capabilities.system_media_controls,
            smtc_status: if cfg!(target_os = "windows") {
                "unavailable".to_string()
            } else {
                "unsupported".to_string()
            },
            smtc_error: None,
        };

        let snapshot_cache = Arc::new(RwLock::new(initial_snapshot));
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_state = Arc::new(Mutex::new(ShutdownProgress::Idle));
        let (shutdown_watch_tx, _) = tokio::sync::watch::channel(ShutdownProgress::Idle);
        let shutdown_watch = Arc::new(shutdown_watch_tx);
        let command_gate = Arc::new(std::sync::RwLock::new(()));
        let persistence_health = Arc::new(RwLock::new(PersistenceHealthInfo::default()));

        // Dedicated persistence thread with its own current-thread runtime.
        // This avoids depending on an ambient Tokio context during Tauri's
        // synchronous setup and gives shutdown a joinable owner.
        let (persist_tx, persist_rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let pool_for_persist = database.clone();
        let persist_health_worker = Arc::clone(&persistence_health);
        let persist_thread = std::thread::Builder::new()
            .name("cavia-persist".into())
            .spawn(move || {
                run_persistence_worker(
                    persist_rx,
                    pool_for_persist,
                    Some(init_tx),
                    persist_health_worker,
                    None,
                );
            })
            .map_err(|e| format!("Failed to spawn persistence worker thread: {e}"))?;

        // Verify persistence worker initialization succeeded
        match init_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(format!("Persistence worker initialization timed out: {e}")),
        }

        let mut core = PlaybackCore::new(backend, Arc::clone(&snapshot_cache), Some(persist_tx));

        if let Some(app) = app_handle {
            core.set_app_handle(app);
        }

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<PlaybackCommand>();
        core.set_cmd_tx(cmd_tx.clone());

        let worker_join = std::thread::Builder::new()
            .name("cavia-audio-control".into())
            .spawn(move || {
                let tick_interval = Duration::from_millis(250);
                loop {
                    match cmd_rx.recv_timeout(tick_interval) {
                        Ok(cmd) => match cmd {
                            PlaybackCommand::PlayTrack { track_id, reply } => {
                                let _ = reply.send(core.play_track(&track_id));
                            }
                            PlaybackCommand::PlayQueueIndex { index, reply } => {
                                let _ = reply.send(core.play_index(index));
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
                            PlaybackCommand::SetMixedQueue {
                                tracks,
                                start_index,
                                enable_radio,
                                reply,
                            } => {
                                let _ = reply.send(core.set_mixed_queue(
                                    tracks,
                                    start_index,
                                    enable_radio,
                                ));
                            }
                            PlaybackCommand::AddToQueue { track, reply } => {
                                let _ = reply.send(core.add_to_queue(track));
                            }
                            PlaybackCommand::PlayNext { track, reply } => {
                                let _ = reply.send(core.play_next(track));
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
                            PlaybackCommand::SetResolverTx { tx, reply } => {
                                core.set_resolver_tx(tx);
                                let _ = reply.send(Ok(()));
                            }
                            PlaybackCommand::FinishTrackResolution {
                                generation,
                                index,
                                track_id,
                                result,
                            } => {
                                core.finish_track_resolution(generation, index, &track_id, result);
                            }
                            PlaybackCommand::FinishRadioFetch { generation, result } => {
                                core.finish_radio_fetch(generation, result);
                            }
                            PlaybackCommand::InitMediaControls { hwnd, reply } => {
                                let _ = reply.send(core.init_media_controls(hwnd));
                            }
                            PlaybackCommand::SetMediaControlsHost { host, reply } => {
                                let _ = reply.send(core.set_media_controls_host(host));
                            }
                            PlaybackCommand::RetryDevice { reply } => {
                                let _ = reply.send(core.retry_device());
                            }
                            PlaybackCommand::SetCoordinatorTx {
                                tx,
                                fallback_restore_armed,
                                reply,
                            } => {
                                core.set_coordinator_tx(tx, fallback_restore_armed);
                                let _ = reply.send(Ok(()));
                            }
                            PlaybackCommand::QueueSnapshot { reply } => {
                                let _ = reply.send(Ok(core.queue.snapshot_state()));
                            }
                            PlaybackCommand::ApplyCollaborativeSequence {
                                current,
                                upcoming,
                                start_if_idle,
                                reply,
                            } => {
                                let _ = reply.send(core.apply_collaborative_sequence(
                                    current,
                                    upcoming,
                                    start_if_idle,
                                ));
                            }
                            PlaybackCommand::EnterOfflineFallback {
                                fallback_tracks,
                                reply,
                            } => {
                                let _ = reply.send(core.enter_offline_fallback(fallback_tracks));
                            }
                            PlaybackCommand::ExitOfflineFallback {
                                saved,
                                played_entry_ids,
                                played_track_ids,
                                auto_play,
                                reply,
                            } => {
                                let _ = reply.send(core.exit_offline_fallback(
                                    saved,
                                    played_entry_ids,
                                    played_track_ids,
                                    auto_play,
                                ));
                            }
                            PlaybackCommand::SetConnectMetadataTag { tag, reply } => {
                                let _ = reply.send(core.set_connect_metadata_tag(tag));
                            }
                            PlaybackCommand::Shutdown { reply } => {
                                let res = core.shutdown();
                                let is_ok = res.is_ok();
                                let _ = reply.send(res);
                                if is_ok {
                                    break;
                                }
                            }
                        },
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            core.tick();
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            let _ = core.shutdown();
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
            backend_engine_id,
            shutdown_flag,
            shutdown_state,
            shutdown_watch,
            command_gate,
            persistence_health,
            preload_coordinator: Arc::new(Mutex::new(None)),
            _worker_thread: Arc::new(Mutex::new(Some(worker_join))),
            _persist_thread: Arc::new(Mutex::new(Some(persist_thread))),
        })
    }

    pub async fn attach_coordinators(
        &self,
        preload: Arc<PreloadCoordinator>,
        connect: Arc<ConnectService>,
        offline: Arc<OfflineFallbackService>,
    ) -> Result<(), String> {
        if let Ok(mut guard) = self.preload_coordinator.lock() {
            *guard = Some(Arc::clone(&preload));
        }

        preload.start_worker();

        let (coord_tx, mut coord_rx) = tokio::sync::mpsc::unbounded_channel::<CoordinatorEvent>();
        let fallback_restore_armed = Arc::new(AtomicBool::new(false));

        let rx = self.send_command(|reply| PlaybackCommand::SetCoordinatorTx {
            tx: coord_tx,
            fallback_restore_armed: Arc::clone(&fallback_restore_armed),
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())??;

        let service = self.clone();
        let preload_ev = Arc::clone(&preload);
        let connect_ev = Arc::clone(&connect);
        let offline_ev = Arc::clone(&offline);
        let restore_armed_ev = Arc::clone(&fallback_restore_armed);
        let shutdown_ev = Arc::clone(&self.shutdown_flag);

        tokio::spawn(async move {
            while let Some(ev) = coord_rx.recv().await {
                if shutdown_ev.load(Ordering::SeqCst) {
                    break;
                }
                match ev {
                    CoordinatorEvent::QueueSnapshotChanged { current, upcoming } => {
                        if !connect_ev.is_active() {
                            preload_ev.update_window(current, upcoming, Vec::new());
                        }
                    }
                    CoordinatorEvent::TrackStarted { track } => {
                        offline_ev.record_track_played(&track);
                    }
                    CoordinatorEvent::TrackResolutionFinished { error } => {
                        if let Some(err) = error {
                            if is_connectivity_error(&err) {
                                preload_ev.set_offline_paused(true);
                                let queue_tracks = service.snapshot().queue;
                                offline_ev.notify_connectivity_lost(&queue_tracks).await;
                            }
                        } else {
                            handle_connectivity_restored(
                                &service,
                                &preload_ev,
                                &connect_ev,
                                &offline_ev,
                                &restore_armed_ev,
                            )
                            .await;
                        }
                    }
                    CoordinatorEvent::PlaybackAdvanced => {
                        if connect_ev.is_active() && !offline_ev.is_fallback_active() {
                            let _ = connect_ev.advance_playback().await;
                        }
                    }
                    CoordinatorEvent::FallbackExhausted => {
                        offline_ev.mark_fallback_exhausted();
                    }
                    CoordinatorEvent::RequestRestoreOriginalQueue => {
                        restore_original_queue_now(&service, &connect_ev, &offline_ev, true).await;
                    }
                }
            }
        });

        // Background connectivity recovery probe that sleeps cleanly and only probes the network
        // while an outage / offline_paused state is active.
        let service_probe = self.clone();
        let preload_probe = Arc::clone(&preload);
        let connect_probe = Arc::clone(&connect);
        let offline_probe = Arc::clone(&offline);
        let armed_probe = Arc::clone(&fallback_restore_armed);
        let shutdown_probe = Arc::clone(&self.shutdown_flag);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(6)).await;
                if shutdown_probe.load(Ordering::SeqCst) {
                    break;
                }
                let needs_probe =
                    offline_probe.snapshot().is_offline || preload_probe.snapshot().offline_paused;
                if needs_probe && probe_internet_connectivity().await {
                    handle_connectivity_restored(
                        &service_probe,
                        &preload_probe,
                        &connect_probe,
                        &offline_probe,
                        &armed_probe,
                    )
                    .await;
                }
            }
        });

        Ok(())
    }

    pub async fn queue_snapshot(&self) -> Result<SavedQueueSnapshot, String> {
        let rx = {
            let _gate = self
                .command_gate
                .read()
                .map_err(|_| "Command gate poisoned".to_string())?;
            if self.shutdown_flag.load(Ordering::SeqCst) {
                return Err("Playback service is shutting down".to_string());
            }
            let (tx, rx) = oneshot::channel();
            self.cmd_tx
                .send(PlaybackCommand::QueueSnapshot { reply: tx })
                .map_err(|_| "Audio worker thread is offline".to_string())?;
            rx
        };
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn apply_collaborative_sequence(
        &self,
        current: Option<Track>,
        upcoming: Vec<Track>,
        start_if_idle: bool,
    ) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::ApplyCollaborativeSequence {
            current,
            upcoming,
            start_if_idle,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn enter_offline_fallback(&self, fallback_tracks: Vec<Track>) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::EnterOfflineFallback {
            fallback_tracks,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn exit_offline_fallback(
        &self,
        saved: SavedQueueSnapshot,
        played_entry_ids: HashSet<String>,
        played_track_ids: HashSet<String>,
        auto_play: bool,
    ) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::ExitOfflineFallback {
            saved,
            played_entry_ids,
            played_track_ids,
            auto_play,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_connect_metadata_tag(&self, tag: Option<String>) -> Result<(), String> {
        let rx =
            self.send_command(|reply| PlaybackCommand::SetConnectMetadataTag { tag, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    /// Attaches `YtMusicService` so `PlaybackCore` can resolve uncached YouTube Music
    /// tracks and fetch radio continuations natively in Rust without blocking the audio thread.
    pub async fn attach_ytmusic(&self, ytmusic: Arc<YtMusicService>) -> Result<(), String> {
        let (work_tx, mut work_rx) = tokio::sync::mpsc::unbounded_channel::<ProviderWork>();
        let cmd_tx = self.cmd_tx.clone();
        let shutdown_flag = Arc::clone(&self.shutdown_flag);
        let preload_holder = Arc::clone(&self.preload_coordinator);

        tokio::spawn(async move {
            while let Some(work) = work_rx.recv().await {
                if shutdown_flag.load(Ordering::SeqCst) {
                    break;
                }
                match work {
                    ProviderWork::ResolveTrack {
                        generation,
                        index,
                        track,
                    } => {
                        let preload_opt = preload_holder.lock().ok().and_then(|g| g.clone());
                        let res = match ytmusic_video_id(&track) {
                            Some(vid) => {
                                if let Some(cached) = ytmusic.cached_playable_path(vid) {
                                    Ok(cached.to_string_lossy().to_string())
                                } else {
                                    if let Some(p) = &preload_opt {
                                        p.begin_active_playback_resolution();
                                    }
                                    let resolved_res = match ytmusic.resolve_audio(vid).await {
                                        Ok(resolved) => ytmusic
                                            .ensure_cached_with_mime(
                                                vid,
                                                &resolved.url,
                                                &resolved.mime_type,
                                            )
                                            .await
                                            .map(|p| p.to_string_lossy().to_string()),
                                        Err(err) => Err(err),
                                    };
                                    if let Some(p) = &preload_opt {
                                        p.end_active_playback_resolution();
                                    }
                                    resolved_res
                                }
                            }
                            None => Err("Unsupported provider track identifier".to_string()),
                        };
                        let _ = cmd_tx.send(PlaybackCommand::FinishTrackResolution {
                            generation,
                            index,
                            track_id: track.id,
                            result: res,
                        });
                    }
                    ProviderWork::FetchRadio {
                        generation,
                        video_id,
                        seen_ids,
                        seen_titles,
                    } => {
                        let res = ytmusic.related(&video_id).await.map(|items| {
                            items
                                .into_iter()
                                .filter(|item| {
                                    let tid = format!("ytmusic:{}", item.video_id);
                                    if seen_ids.contains(&tid) {
                                        return false;
                                    }
                                    let base = base_song_title(&item.title);
                                    let key = (item.artist.trim().to_lowercase(), base);
                                    !key.1.is_empty() && !seen_titles.contains(&key)
                                })
                                .take(10)
                                .map(ytmusic_track_to_domain)
                                .collect::<Vec<_>>()
                        });
                        let _ = cmd_tx.send(PlaybackCommand::FinishRadioFetch {
                            generation,
                            result: res,
                        });
                    }
                }
            }
        });

        let rx =
            self.send_command(|reply| PlaybackCommand::SetResolverTx { tx: work_tx, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub fn persistence_health(&self) -> PersistenceHealthInfo {
        self.persistence_health
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn send_command<F>(&self, make_cmd: F) -> Result<oneshot::Receiver<Result<(), String>>, String>
    where
        F: FnOnce(oneshot::Sender<Result<(), String>>) -> PlaybackCommand,
    {
        let _gate = self
            .command_gate
            .read()
            .map_err(|_| "Command gate poisoned".to_string())?;
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return Err("Playback service is shutting down".to_string());
        }
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(make_cmd(tx))
            .map_err(|_| "Audio worker thread is offline".to_string())?;
        Ok(rx)
    }

    pub fn set_app_handle(&self, app_handle: AppHandle) {
        let _gate = match self.command_gate.read() {
            Ok(g) => g,
            Err(_) => return,
        };
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
        let rx = self.send_command(|reply| PlaybackCommand::InitMediaControls { hwnd, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_media_controls_host(
        &self,
        host: Box<dyn MediaControlsHost>,
    ) -> Result<(), String> {
        let rx =
            self.send_command(|reply| PlaybackCommand::SetMediaControlsHost { host, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub fn smtc_status(&self) -> String {
        self.snapshot().smtc_status
    }

    pub fn smtc_error(&self) -> Option<String> {
        self.snapshot().smtc_error
    }

    pub async fn play_track(&self, track_id: &str) -> Result<(), String> {
        let track_id_owned = track_id.to_string();
        let rx = self.send_command(|reply| PlaybackCommand::PlayTrack {
            track_id: track_id_owned,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn play_queue_index(&self, index: usize) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::PlayQueueIndex { index, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_mixed_queue(
        &self,
        tracks: Vec<Track>,
        start_index: Option<usize>,
        enable_radio: bool,
    ) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::SetMixedQueue {
            tracks,
            start_index,
            enable_radio,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn play(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::Play { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn pause(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::Pause { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn play_pause(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::PlayPause { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn stop(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::Stop { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn seek(&self, position_ms: u64, generation: Option<u64>) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::Seek {
            position_ms,
            generation,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_volume(&self, volume: u32) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::SetVolume { volume, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn toggle_mute(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::ToggleMute { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_speed(&self, speed: f32) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::SetSpeed { speed, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_repeat_mode(&self, mode: &str) -> Result<(), String> {
        let mode_owned = mode.to_string();
        let rx = self.send_command(|reply| PlaybackCommand::SetRepeatMode {
            mode: mode_owned,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_shuffle(&self, enabled: bool) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::SetShuffle { enabled, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn next(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::Next { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn previous(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::Previous { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn set_queue(
        &self,
        tracks: Vec<Track>,
        start_track_id: Option<&str>,
    ) -> Result<(), String> {
        let start_track_id_owned = start_track_id.map(|s| s.to_string());
        let rx = self.send_command(|reply| PlaybackCommand::SetQueue {
            tracks,
            start_track_id: start_track_id_owned,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn add_to_queue(&self, track: Track) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::AddToQueue { track, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn play_next(&self, track: Track) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::PlayNext { track, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn remove_from_queue(&self, track_id: &str) -> Result<(), String> {
        let track_id_owned = track_id.to_string();
        let rx = self.send_command(|reply| PlaybackCommand::RemoveFromQueue {
            track_id: track_id_owned,
            reply,
        })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn remove_from_queue_by_index(&self, index: usize) -> Result<(), String> {
        let rx =
            self.send_command(|reply| PlaybackCommand::RemoveFromQueueByIndex { index, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn reorder_queue(&self, from: usize, to: usize) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::ReorderQueue { from, to, reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn clear_queue(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::ClearQueue { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub async fn restore_saved_state(&self) -> Result<(), String> {
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

            let mut all_tracks = Vec::new();
            if let Ok(tracks) = serde_json::from_str::<Vec<Track>>(&queue_json) {
                all_tracks = tracks;
            }

            let rx = self.send_command(|reply| PlaybackCommand::ApplyRestoredState {
                tracks: all_tracks,
                current_track_id,
                saved_position_ms,
                repeat_mode,
                shuffle_enabled: shuffle_enabled == 1,
                playback_speed: playback_speed as f32,
                volume: volume as u32,
                is_muted: is_muted == 1,
                reply,
            })?;
            rx.await
                .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())??;
        }

        Ok(())
    }

    pub async fn retry_device(&self) -> Result<(), String> {
        let rx = self.send_command(|reply| PlaybackCommand::RetryDevice { reply })?;
        rx.await
            .map_err(|_| "Audio worker thread terminated unexpectedly".to_string())?
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot_cache
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn backend_health_status(&self) -> String {
        self.snapshot().backend_health
    }

    pub fn backend_name(&self) -> String {
        self.backend_engine_id.to_string()
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }

    fn join_workers(&self) -> Result<(), String> {
        let mut err = None;
        if let Ok(mut slot) = self._worker_thread.lock() {
            if let Some(handle) = slot.take() {
                if let Err(_) = handle.join() {
                    err = Some("Audio worker thread panicked during shutdown".to_string());
                }
            }
        }
        if let Ok(mut slot) = self._persist_thread.lock() {
            if let Some(handle) = slot.take() {
                if let Err(_) = handle.join() {
                    let msg = "Persistence worker thread panicked during shutdown".to_string();
                    err = Some(err.map(|e| format!("{e}; {msg}")).unwrap_or(msg));
                }
            }
        }
        if let Some(e) = err { Err(e) } else { Ok(()) }
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        loop {
            let mut rx = self.shutdown_watch.subscribe();
            let is_in_progress = {
                let mut state = self
                    .shutdown_state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                match &*state {
                    ShutdownProgress::Completed => return Ok(()),
                    ShutdownProgress::InProgress => true,
                    ShutdownProgress::Idle | ShutdownProgress::Failed(_) => {
                        *state = ShutdownProgress::InProgress;
                        let _ = self.shutdown_watch.send(ShutdownProgress::InProgress);
                        false
                    }
                }
            };

            if is_in_progress {
                // Wait for the in-progress shutdown to reach a terminal state (Completed or Failed).
                // Because watch channel retains the latest state, wait_for returns immediately
                // without lost wakeups if the transition completed before this line is reached.
                let wait_res = rx
                    .wait_for(|val| {
                        matches!(
                            val,
                            ShutdownProgress::Completed | ShutdownProgress::Failed(_)
                        )
                    })
                    .await;
                match wait_res {
                    Ok(val) => match &*val {
                        ShutdownProgress::Completed => return Ok(()),
                        ShutdownProgress::Failed(err) => return Err(err.clone()),
                        _ => continue,
                    },
                    Err(_) => return Err("Shutdown watch channel closed".into()),
                }
            }

            let (tx, rx_reply) = oneshot::channel();
            let send_res = {
                let _gate = self
                    .command_gate
                    .write()
                    .map_err(|_| "Command gate poisoned".to_string())?;
                self.shutdown_flag.store(true, Ordering::SeqCst);
                self.cmd_tx.send(PlaybackCommand::Shutdown { reply: tx })
            };

            let res = match send_res {
                Ok(()) => match tokio::time::timeout(Duration::from_secs(5), rx_reply).await {
                    Ok(Ok(inner_res)) => inner_res,
                    Ok(Err(_)) => Err("Audio worker thread dropped shutdown reply".into()),
                    Err(_) => Err("Audio worker shutdown timed out".into()),
                },
                Err(_) => Err("Audio worker thread channel disconnected".into()),
            };

            match res {
                Ok(()) => {
                    let join_res = self.join_workers();
                    let mut state = self
                        .shutdown_state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if let Err(e) = join_res {
                        *state = ShutdownProgress::Failed(e.clone());
                        let _ = self
                            .shutdown_watch
                            .send(ShutdownProgress::Failed(e.clone()));
                        return Err(e);
                    } else {
                        *state = ShutdownProgress::Completed;
                        let _ = self.shutdown_watch.send(ShutdownProgress::Completed);
                        return Ok(());
                    }
                }
                Err(err_msg) => {
                    self.shutdown_flag.store(false, Ordering::SeqCst);
                    let mut state = self
                        .shutdown_state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *state = ShutdownProgress::Failed(err_msg.clone());
                    let _ = self
                        .shutdown_watch
                        .send(ShutdownProgress::Failed(err_msg.clone()));
                    return Err(err_msg);
                }
            }
        }
    }
}

pub async fn handle_connectivity_restored(
    playback: &PlaybackService,
    preload: &PreloadCoordinator,
    connect: &ConnectService,
    offline: &OfflineFallbackService,
    fallback_restore_armed: &AtomicBool,
) {
    preload.set_offline_paused(false);
    if connect.is_active() {
        let _ = connect.sync_pending_events().await;
    }
    let snap = playback.snapshot();
    let actively_playing = matches!(snap.status.as_str(), "playing" | "paused");
    let status = offline.mark_reconnected(actively_playing);
    if status.fallback_active {
        if actively_playing {
            fallback_restore_armed.store(true, Ordering::SeqCst);
        } else {
            restore_original_queue_now(playback, connect, offline, false).await;
        }
    }
}

pub async fn restore_original_queue_now(
    playback: &PlaybackService,
    connect: &ConnectService,
    offline: &OfflineFallbackService,
    auto_play: bool,
) {
    if let Some((saved, played_entries, played_tracks)) = offline.take_restore_bundle() {
        let _ = playback
            .exit_offline_fallback(saved, played_entries, played_tracks, auto_play)
            .await;
    }
    if connect.is_active()
        && let Some((current, upcoming)) = connect.upcoming_playback_tracks()
    {
        let _ = playback
            .apply_collaborative_sequence(current, upcoming, false)
            .await;
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
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        }
    }

    pub fn new_test_core(backend: Box<dyn AudioBackend>) -> PlaybackCore {
        let initial_snapshot = PlaybackSnapshot {
            generation: 1,
            status: "idle".to_string(),
            current_track: None,
            current_index: None,
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
            smtc_status: if cfg!(target_os = "windows") {
                "unavailable".to_string()
            } else {
                "unsupported".to_string()
            },
            smtc_error: None,
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
    async fn test_clear_queue_resets_restored_and_backend_positions() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Playing,
            duration_ms: 1000,
            position_ms: 700,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        core.queue.set_queue(
            vec![dummy_track("1", "Temporary", Some("C:/temporary.wav"))],
            Some("1"),
        );
        core.restored_position_ms = Some(900);
        let (tx, rx) = std::sync::mpsc::channel();
        core.persistence_tx = Some(tx);

        core.clear_queue().unwrap();

        let snapshot = core.snapshot();
        assert!(snapshot.queue.is_empty());
        assert!(snapshot.current_track.is_none());
        assert_eq!(snapshot.position_ms, 0);
        assert_eq!(core.restored_position_ms, None);
        match rx.recv_timeout(Duration::from_secs(1)).unwrap() {
            PersistenceMsg::State(payload) => assert_eq!(payload.position_ms, 0),
            PersistenceMsg::Flush(_) | PersistenceMsg::Shutdown(_) => {
                panic!("unexpected persistence message")
            }
        }
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
    async fn test_set_queue_fails_atomically_on_missing_track_preserving_previous_queue() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_sq_miss_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 1.0);

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let old1 = dummy_track("old1", "Old 1", Some(good_wav.to_str().unwrap()));
        core.set_queue(vec![old1.clone()], Some("old1")).unwrap();
        assert_eq!(core.queue.current_track().unwrap().id, "old1");

        let missing = dummy_track("missing", "Missing", Some("C:/non_existent_cavia_sq.flac"));
        let candidate2 = dummy_track("cand2", "Cand 2", Some(good_wav.to_str().unwrap()));

        // set_queue requesting missing track MUST fail atomically without skipping to candidate2
        let res = core.set_queue(vec![missing, candidate2], Some("missing"));
        assert!(res.is_err());

        // Previous queue and selection must remain 100% intact
        assert_eq!(core.queue.len(), 1);
        assert_eq!(core.queue.current_track().unwrap().id, "old1");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_set_queue_fails_atomically_on_corrupt_header_preserving_previous_queue() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_sq_hdr_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 1.0);

        let corrupt_wav = temp_dir.join("corrupt_header.wav");
        std::fs::write(&corrupt_wav, b"NOT_A_VALID_WAV_HEADER_12345").unwrap();

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let old1 = dummy_track("old1", "Old 1", Some(good_wav.to_str().unwrap()));
        core.set_queue(vec![old1.clone()], Some("old1")).unwrap();
        assert_eq!(core.queue.current_track().unwrap().id, "old1");

        let bad_track = dummy_track("bad", "Bad", Some(corrupt_wav.to_str().unwrap()));
        let res = core.set_queue(vec![bad_track], Some("bad"));
        assert!(res.is_err());

        // Previous queue and selection must remain intact
        assert_eq!(core.queue.len(), 1);
        assert_eq!(core.queue.current_track().unwrap().id, "old1");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_set_queue_fails_atomically_on_valid_header_with_truncated_audio_preserving_previous_queue()
     {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_sq_trunc_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 1.0);

        // Create a WAV with valid 44-byte PCM header claiming 50000 bytes of data, but with 0 data bytes
        let trunc_wav = temp_dir.join("truncated.wav");
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&trunc_wav).unwrap();
            file.write_all(b"RIFF").unwrap();
            file.write_all(&(36u32 + 50000).to_le_bytes()).unwrap();
            file.write_all(b"WAVEfmt ").unwrap();
            file.write_all(&16u32.to_le_bytes()).unwrap();
            file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
            file.write_all(&1u16.to_le_bytes()).unwrap(); // 1 channel
            file.write_all(&44100u32.to_le_bytes()).unwrap(); // 44.1kHz
            file.write_all(&(44100u32 * 2).to_le_bytes()).unwrap(); // byte rate
            file.write_all(&2u16.to_le_bytes()).unwrap(); // block align
            file.write_all(&16u16.to_le_bytes()).unwrap(); // 16 bits
            file.write_all(b"data").unwrap();
            file.write_all(&50000u32.to_le_bytes()).unwrap(); // claims 50000 bytes
            // Write ZERO sample bytes (truncated payload)
        }

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let old1 = dummy_track("old1", "Old 1", Some(good_wav.to_str().unwrap()));
        core.set_queue(vec![old1.clone()], Some("old1")).unwrap();
        assert_eq!(core.queue.current_track().unwrap().id, "old1");

        let trunc_track = dummy_track("trunc", "Truncated", Some(trunc_wav.to_str().unwrap()));
        let res = core.set_queue(vec![trunc_track], Some("trunc"));
        assert!(
            res.is_err(),
            "set_queue should fail on truncated audio data"
        );

        // Previous queue and selection must remain intact
        assert_eq!(core.queue.len(), 1);
        assert_eq!(core.queue.current_track().unwrap().id, "old1");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_set_queue_fails_atomically_on_unsupported_codec_preserving_previous_queue() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_sq_codec_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 1.0);

        let unsupported = temp_dir.join("unsupported.wma");
        std::fs::write(
            &unsupported,
            b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA\x00\x62\xCE\x6C",
        )
        .unwrap();

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let old1 = dummy_track("old1", "Old 1", Some(good_wav.to_str().unwrap()));
        core.set_queue(vec![old1.clone()], Some("old1")).unwrap();
        assert_eq!(core.queue.current_track().unwrap().id, "old1");

        let unsupp_track =
            dummy_track("unsupp", "Unsupported", Some(unsupported.to_str().unwrap()));
        let res = core.set_queue(vec![unsupp_track], Some("unsupp"));
        assert!(res.is_err(), "set_queue should fail on unsupported codec");

        assert_eq!(core.queue.len(), 1);
        assert_eq!(core.queue.current_track().unwrap().id, "old1");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_set_queue_succeeds_on_valid_track_and_supports_normal_navigation() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_sq_ok_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav1 = temp_dir.join("track1.wav");
        let wav2 = temp_dir.join("track2.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&wav1, 2.0);
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&wav2, 2.0);

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let t1 = dummy_track("t1", "Track 1", Some(wav1.to_str().unwrap()));
        let t2 = dummy_track("t2", "Track 2", Some(wav2.to_str().unwrap()));

        let res = core.set_queue(vec![t1.clone(), t2.clone()], Some("t1"));
        assert!(res.is_ok());
        assert_eq!(core.queue.current_track().unwrap().id, "t1");
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);

        // Next navigates to t2
        let next_res = core.next();
        assert!(next_res.is_ok());
        assert_eq!(core.queue.current_track().unwrap().id, "t2");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_persistence_round_trip_integration() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_persist_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let pool = temp_pool(&temp_dir).await;

        let service = PlaybackService::new(pool.clone(), None).unwrap();
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

        service.shutdown().await.unwrap();
        service2.shutdown().await.unwrap();
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
            PersistenceMsg::Shutdown(_) => panic!("unexpected shutdown message"),
        }
    }

    #[tokio::test]
    async fn test_write_playback_state_propagates_sql_error() {
        // Connect to an in-memory SQLite pool without running migrations (table does not exist)
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();

        let payload = PersistencePayload {
            queue_json: "[]".into(),
            current_track_id: None,
            position_ms: 1000,
            repeat_mode: "off".into(),
            shuffle_enabled: 0,
            playback_speed: 1.0,
            volume: 85,
            is_muted: 0,
        };

        let res = write_playback_state(&pool, &payload).await;
        assert!(
            res.is_err(),
            "write_playback_state must propagate SQLx error"
        );
    }

    // --- Issue 2 Shutdown Tests A-H ---

    #[tokio::test]
    async fn test_shutdown_test_a_normal_shutdown_persists_state_and_terminates_workers() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_a_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        service.set_volume(78).await.unwrap();
        service.set_repeat_mode("one").await.unwrap();

        let shut_res = service.shutdown().await;
        assert!(shut_res.is_ok(), "Shutdown should succeed");
        assert!(service.is_shutting_down());

        // Verify SQLite contains persisted state
        let row: (i64, String) =
            sqlx::query_as("SELECT volume, repeat_mode FROM playback_state WHERE id = 1")
                .fetch_one(&pool)
                .await
                .expect("Row must exist in SQLite");
        assert_eq!(row.0, 78);
        assert_eq!(row.1, "one");

        // Verify worker thread handles are joined/consumed
        assert!(service._worker_thread.lock().unwrap().is_none());
        assert!(service._persist_thread.lock().unwrap().is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_test_b_shutdown_after_persistence_failure_returns_error() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_b_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        // Inject SQLite trigger to force persistence failure
        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_shut_b \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected DB failure for shutdown B'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        service.set_volume(45).await.unwrap();
        let shut_res = service.shutdown().await;
        assert!(
            shut_res.is_err(),
            "Shutdown must return error on persistence failure"
        );
        let err_msg = shut_res.unwrap_err();
        assert_eq!(
            *service.shutdown_state.lock().unwrap(),
            ShutdownProgress::Failed(err_msg)
        );

        // Service must NOT be permanently shut down; threads should still be present for retry
        assert!(service._worker_thread.lock().unwrap().is_some());
        assert!(service._persist_thread.lock().unwrap().is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_lost_wakeup_deterministic() {
        let dir = std::env::temp_dir().join(format!("cavia_lost_wakeup_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        // 1. Caller B subscribes when shutdown is InProgress:
        let mut rx = service.shutdown_watch.subscribe();
        {
            let mut state = service.shutdown_state.lock().unwrap();
            *state = ShutdownProgress::InProgress;
            let _ = service.shutdown_watch.send(ShutdownProgress::InProgress);
        }

        // 2. Caller A finishes shutdown completely before Caller B awaits the watcher:
        {
            let mut state = service.shutdown_state.lock().unwrap();
            *state = ShutdownProgress::Completed;
            let _ = service.shutdown_watch.send(ShutdownProgress::Completed);
        }

        // 3. Caller B now awaits terminal state:
        // With watch channels, wait_for observes the latest value immediately without losing the wakeup.
        let wait_res = tokio::time::timeout(
            Duration::from_millis(500),
            rx.wait_for(|val| {
                matches!(
                    val,
                    ShutdownProgress::Completed | ShutdownProgress::Failed(_)
                )
            }),
        )
        .await;

        assert!(
            wait_res.is_ok(),
            "Watch channel must wake immediately on existing terminal state without timing out"
        );
        assert_eq!(*wait_res.unwrap().unwrap(), ShutdownProgress::Completed);

        // A call to shutdown() by Caller B also returns Ok(()) immediately:
        let shut_res = tokio::time::timeout(Duration::from_millis(500), service.shutdown()).await;
        assert!(shut_res.is_ok());
        assert!(shut_res.unwrap().is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_concurrent_failed_callers() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_fail_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        // Inject SQL failure trigger on playback_state
        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_concurrent \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected concurrent failure'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        let _ = service.set_volume(42).await;

        // Spawn 10 concurrent callers attempting shutdown simultaneously
        let mut handles = Vec::new();
        for _ in 0..10 {
            let s = service.clone();
            handles.push(tokio::spawn(async move { s.shutdown().await }));
        }

        let mut errors = Vec::new();
        for h in handles {
            let res = h.await.unwrap();
            assert!(
                res.is_err(),
                "All concurrent callers must observe Err during failed shutdown"
            );
            errors.push(res.unwrap_err());
        }

        // All 10 callers must have received the same consistent error message
        let first_err = &errors[0];
        assert!(first_err.contains("Injected concurrent failure"));
        for err in &errors {
            assert_eq!(
                err, first_err,
                "All concurrent callers must receive identical error"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_repeated_after_completion() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_rep_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        // Perform initial shutdown
        let initial_res = service.shutdown().await;
        assert!(initial_res.is_ok());

        // Repeated subsequent calls must return Ok immediately without blocking
        for _ in 0..5 {
            let res = tokio::time::timeout(Duration::from_millis(200), service.shutdown()).await;
            assert!(res.is_ok(), "Repeated shutdown must not hang");
            assert!(res.unwrap().is_ok());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_test_c_retry_shutdown() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_retry_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        // 1. Inject trigger to force persistence failure
        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_retry \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected failure retry'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        // 2. Set volume to 62 (which queues state to persist)
        let _ = service.set_volume(62).await;

        // 3. First shutdown attempt: must fail because persistence write fails
        let res1 = service.shutdown().await;
        assert!(res1.is_err(), "First shutdown must fail due to DB trigger");

        // 4. Restore DB access
        sqlx::query("DROP TRIGGER force_playback_state_failure_retry")
            .execute(&pool)
            .await
            .unwrap();

        // 5. Retry shutdown
        let res2 = service.shutdown().await;
        assert!(res2.is_ok(), "Retried shutdown must succeed");

        // 6. Verify SQLite actually contains the persisted volume 62!
        let row: (i64,) = sqlx::query_as("SELECT volume FROM playback_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("SQLite must have persisted row on successful shutdown retry");
        assert_eq!(
            row.0, 62,
            "Retried shutdown must have committed state to SQLite"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_test_d_concurrent_shutdown_callers() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_d_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        service.set_volume(81).await.unwrap();

        let mut handles = Vec::new();
        for _ in 0..10 {
            let s = service.clone();
            handles.push(tokio::spawn(async move { s.shutdown().await }));
        }

        for h in handles {
            let res = h.await.unwrap();
            assert!(
                res.is_ok(),
                "All concurrent shutdown callers must observe Ok(())"
            );
        }

        assert_eq!(
            *service.shutdown_state.lock().unwrap(),
            ShutdownProgress::Completed
        );
        let row: (i64,) = sqlx::query_as("SELECT volume FROM playback_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, 81);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_test_e_command_acceptance_during_shutdown() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_e_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool, None).unwrap();

        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let barrier_clone = Arc::clone(&barrier);
        let s_clone = service.clone();

        let h_cmd = tokio::spawn(async move {
            barrier_clone.wait().await;
            s_clone.set_volume(55).await
        });

        let s_shut = service.clone();
        let h_shut = tokio::spawn(async move {
            barrier.wait().await;
            s_shut.shutdown().await
        });

        let (res_cmd, res_shut) = tokio::join!(h_cmd, h_shut);
        assert!(res_shut.unwrap().is_ok());
        let cmd_res = res_cmd.unwrap();
        match cmd_res {
            Ok(()) => {}
            Err(e) => assert_eq!(e, "Playback service is shutting down"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_test_f_worker_failure_reports_error() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_f_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let mut service = PlaybackService::new(pool, None).unwrap();

        // Disconnect cmd_tx by replacing it with a closed sender
        let (closed_tx, _) = std::sync::mpsc::channel();
        service.cmd_tx = closed_tx;

        let res = service.shutdown().await;
        assert!(
            res.is_err(),
            "Shutdown must report failure when actor is terminated"
        );
        assert!(res.unwrap_err().contains("disconnected"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_test_g_deterministic_shutdown_ordering() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_g_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        let h1 = {
            let s = service.clone();
            tokio::spawn(async move { s.set_volume(91).await })
        };
        let h2 = {
            let s = service.clone();
            tokio::spawn(async move { s.shutdown().await })
        };

        let (res1, res2) = tokio::join!(h1, h2);
        assert!(res2.unwrap().is_ok(), "Shutdown must succeed");
        match res1.unwrap() {
            Ok(()) => {
                let row: (i64,) = sqlx::query_as("SELECT volume FROM playback_state WHERE id = 1")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                assert_eq!(row.0, 91);
            }
            Err(e) => assert_eq!(e, "Playback service is shutting down"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_test_h_application_exit_decision() {
        let dir = std::env::temp_dir().join(format!("cavia_shut_h_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        // 1. Successful shutdown permits exit
        let dec1 = crate::handle_exit_request(&service, false).await;
        assert_eq!(dec1, crate::ExitDecision::ExitSuccess);

        // 2. Failed flush prevents exit
        let dir2 = std::env::temp_dir().join(format!("cavia_shut_h2_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir2).unwrap();
        let pool2 = temp_pool(&dir2).await;
        let service2 = PlaybackService::new(pool2.clone(), None).unwrap();

        sqlx::query(
            "CREATE TRIGGER force_fail_exit \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Persistence failed on exit'); \
             END;",
        )
        .execute(&pool2)
        .await
        .unwrap();

        let dec2 = crate::handle_exit_request(&service2, false).await;
        match dec2 {
            crate::ExitDecision::BlockExitAndReport(err) => {
                assert!(err.contains("Persistence failed on exit") || err.contains("Persistence"));
            }
            crate::ExitDecision::ExitSuccess => panic!("Failed flush must NOT permit ExitSuccess!"),
        }

        // 3. Force exit explicitly requested permits exit
        let dec3 = crate::handle_exit_request(&service2, true).await;
        assert_eq!(dec3, crate::ExitDecision::ExitSuccess);

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[tokio::test]
    async fn test_persistence_test_a_successful_state_write() {
        let dir = std::env::temp_dir().join(format!("cavia_p_test_a_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        let (tx, rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let hook_sync = Arc::new(move |res: Result<(), &str>| {
            let _ = sync_tx.send(res.map_err(|s| s.to_string()));
        });

        let health = Arc::new(RwLock::new(PersistenceHealthInfo::default()));
        let health_clone = Arc::clone(&health);
        let pool_worker = pool.clone();
        let worker_handle = std::thread::spawn(move || {
            run_persistence_worker(rx, pool_worker, None, health_clone, Some(hook_sync));
        });

        let payload = PersistencePayload {
            queue_json: "[{\"id\":\"t1\",\"title\":\"Song\"}]".into(),
            current_track_id: Some("t1".into()),
            position_ms: 3500,
            repeat_mode: "all".into(),
            shuffle_enabled: 1,
            playback_speed: 1.25,
            volume: 75,
            is_muted: 0,
        };

        // Submit state
        tx.send(PersistenceMsg::State(payload)).unwrap();
        let hook_res = sync_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("worker hook timeout");
        assert!(hook_res.is_ok(), "Write should succeed");

        // Verify SQLite contains expected values
        let row: (String, Option<String>, i64, String, i64, f64, i64, i64) = sqlx::query_as(
            "SELECT queue_json, current_track_id, position_ms, repeat_mode, shuffle_enabled, playback_speed, volume, is_muted \
             FROM playback_state WHERE id = 1"
        )
        .fetch_one(&pool)
        .await
        .expect("Row must exist in SQLite");

        assert_eq!(row.0, "[{\"id\":\"t1\",\"title\":\"Song\"}]");
        assert_eq!(row.1.as_deref(), Some("t1"));
        assert_eq!(row.2, 3500);
        assert_eq!(row.3, "all");
        assert_eq!(row.4, 1);
        assert!((row.5 - 1.25).abs() < f64::EPSILON);
        assert_eq!(row.6, 75);
        assert_eq!(row.7, 0);

        // Verify worker reports healthy
        assert_eq!(
            health.read().unwrap().status,
            PersistenceHealthStatus::Healthy
        );
        assert!(health.read().unwrap().last_error.is_none());

        drop(tx);
        let _ = worker_handle.join();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_persistence_test_b_failed_async_write_retains_failure() {
        let dir = std::env::temp_dir().join(format!("cavia_p_test_b_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        // Force production writer to fail with trigger
        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_b \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected SQLite persistence failure B'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (tx, rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let hook_sync = Arc::new(move |res: Result<(), &str>| {
            let _ = sync_tx.send(res.map_err(|s| s.to_string()));
        });

        let health = Arc::new(RwLock::new(PersistenceHealthInfo::default()));
        let health_clone = Arc::clone(&health);
        let pool_worker = pool.clone();
        let worker_handle = std::thread::spawn(move || {
            run_persistence_worker(rx, pool_worker, None, health_clone, Some(hook_sync));
        });

        let payload = PersistencePayload {
            queue_json: "[]".into(),
            current_track_id: Some("tx_b".into()),
            position_ms: 1234,
            repeat_mode: "off".into(),
            shuffle_enabled: 0,
            playback_speed: 1.0,
            volume: 80,
            is_muted: 0,
        };

        tx.send(PersistenceMsg::State(payload)).unwrap();
        let hook_res = sync_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("worker hook timeout");
        assert!(hook_res.is_err(), "Hook should report failed write");

        // Failure must be retained in health
        let snap_health = health.read().unwrap().clone();
        assert_eq!(snap_health.status, PersistenceHealthStatus::WriteFailed);
        assert!(
            snap_health
                .last_error
                .unwrap()
                .contains("Injected SQLite persistence failure B")
        );

        drop(tx);
        let _ = worker_handle.join();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_persistence_test_c_flush_after_failed_state_returns_error() {
        let dir = std::env::temp_dir().join(format!("cavia_p_test_c_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_c \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected SQLite persistence failure C'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (tx, rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let hook_sync = Arc::new(move |res: Result<(), &str>| {
            let _ = sync_tx.send(res.map_err(|s| s.to_string()));
        });

        let health = Arc::new(RwLock::new(PersistenceHealthInfo::default()));
        let pool_worker = pool.clone();
        let worker_handle = std::thread::spawn(move || {
            run_persistence_worker(rx, pool_worker, None, health, Some(hook_sync));
        });

        let payload = PersistencePayload {
            queue_json: "[]".into(),
            current_track_id: Some("tx_c".into()),
            position_ms: 4444,
            repeat_mode: "off".into(),
            shuffle_enabled: 0,
            playback_speed: 1.0,
            volume: 80,
            is_muted: 0,
        };

        // Submit State and wait until failure occurs
        tx.send(PersistenceMsg::State(payload)).unwrap();
        let hook_res = sync_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("worker hook timeout");
        assert!(hook_res.is_err());

        // Send Flush: must return error because write was never committed
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        tx.send(PersistenceMsg::Flush(ack_tx)).unwrap();
        let flush_res = ack_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("flush timeout");
        assert!(
            flush_res.is_err(),
            "Flush must NOT return Ok(()) after failed state write"
        );

        drop(tx);
        let _ = worker_handle.join();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_persistence_test_d_retry_after_recovering_database() {
        let dir = std::env::temp_dir().join(format!("cavia_p_test_d_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_d \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected SQLite persistence failure D'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (tx, rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let hook_sync = Arc::new(move |res: Result<(), &str>| {
            let _ = sync_tx.send(res.map_err(|s| s.to_string()));
        });

        let health = Arc::new(RwLock::new(PersistenceHealthInfo::default()));
        let health_clone = Arc::clone(&health);
        let pool_worker = pool.clone();
        let worker_handle = std::thread::spawn(move || {
            run_persistence_worker(rx, pool_worker, None, health_clone, Some(hook_sync));
        });

        let payload = PersistencePayload {
            queue_json: "[]".into(),
            current_track_id: Some("tx_d".into()),
            position_ms: 7777,
            repeat_mode: "off".into(),
            shuffle_enabled: 0,
            playback_speed: 1.0,
            volume: 80,
            is_muted: 0,
        };

        // Submit state and fail
        tx.send(PersistenceMsg::State(payload)).unwrap();
        let hook_res = sync_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("hook timeout");
        assert!(hook_res.is_err());

        // First flush fails
        let (ack_tx1, ack_rx1) = std::sync::mpsc::channel();
        tx.send(PersistenceMsg::Flush(ack_tx1)).unwrap();
        assert!(
            ack_rx1
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );

        // Restore database access
        sqlx::query("DROP TRIGGER force_playback_state_failure_d")
            .execute(&pool)
            .await
            .unwrap();

        // Retry flush
        let (ack_tx2, ack_rx2) = std::sync::mpsc::channel();
        tx.send(PersistenceMsg::Flush(ack_tx2)).unwrap();
        let retry_res = ack_rx2.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(retry_res.is_ok(), "Flush after recovery must succeed");

        // Verify row now exists with 7777 ms in SQLite
        let pos: (i64, Option<String>) =
            sqlx::query_as("SELECT position_ms, current_track_id FROM playback_state WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pos.0, 7777);
        assert_eq!(pos.1.as_deref(), Some("tx_d"));
        assert_eq!(
            health.read().unwrap().status,
            PersistenceHealthStatus::Healthy
        );

        drop(tx);
        let _ = worker_handle.join();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_persistence_test_e_newer_state_replaces_failed_pending_state() {
        let dir = std::env::temp_dir().join(format!("cavia_p_test_e_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_e \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected SQLite persistence failure E'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (tx, rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let hook_sync = Arc::new(move |res: Result<(), &str>| {
            let _ = sync_tx.send(res.map_err(|s| s.to_string()));
        });

        let health = Arc::new(RwLock::new(PersistenceHealthInfo::default()));
        let pool_worker = pool.clone();
        let worker_handle = std::thread::spawn(move || {
            run_persistence_worker(rx, pool_worker, None, health, Some(hook_sync));
        });

        let payload1 = PersistencePayload {
            queue_json: "[]".into(),
            current_track_id: Some("old_track".into()),
            position_ms: 1000,
            repeat_mode: "off".into(),
            shuffle_enabled: 0,
            playback_speed: 1.0,
            volume: 80,
            is_muted: 0,
        };

        let payload2 = PersistencePayload {
            queue_json: "[]".into(),
            current_track_id: Some("new_authoritative_track".into()),
            position_ms: 9999,
            repeat_mode: "all".into(),
            shuffle_enabled: 1,
            playback_speed: 1.5,
            volume: 90,
            is_muted: 0,
        };

        // Send older payload; fails
        tx.send(PersistenceMsg::State(payload1)).unwrap();
        assert!(
            sync_rx
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );

        // Send newer payload; replaces older pending; fails
        tx.send(PersistenceMsg::State(payload2)).unwrap();
        assert!(
            sync_rx
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );

        // Restore database
        sqlx::query("DROP TRIGGER force_playback_state_failure_e")
            .execute(&pool)
            .await
            .unwrap();

        // Flush
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        tx.send(PersistenceMsg::Flush(ack_tx)).unwrap();
        assert!(ack_rx.recv_timeout(Duration::from_secs(3)).unwrap().is_ok());

        // Verify newest authoritative payload is written, not the older one
        let row: (i64, Option<String>, String) = sqlx::query_as(
            "SELECT position_ms, current_track_id, repeat_mode FROM playback_state WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, 9999);
        assert_eq!(row.1.as_deref(), Some("new_authoritative_track"));
        assert_eq!(row.2, "all");

        drop(tx);
        let _ = worker_handle.join();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_persistence_test_f_shutdown_after_failed_write_returns_error_and_allows_retry() {
        let dir = std::env::temp_dir().join(format!("cavia_p_test_f_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_f \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected SQLite persistence failure F'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (tx, rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let hook_sync = Arc::new(move |res: Result<(), &str>| {
            let _ = sync_tx.send(res.map_err(|s| s.to_string()));
        });

        let health = Arc::new(RwLock::new(PersistenceHealthInfo::default()));
        let health_clone = Arc::clone(&health);
        let pool_worker = pool.clone();
        let worker_handle = std::thread::spawn(move || {
            run_persistence_worker(rx, pool_worker, None, health_clone, Some(hook_sync));
        });

        let payload = PersistencePayload {
            queue_json: "[]".into(),
            current_track_id: Some("shutdown_track".into()),
            position_ms: 8888,
            repeat_mode: "off".into(),
            shuffle_enabled: 0,
            playback_speed: 1.0,
            volume: 80,
            is_muted: 0,
        };

        // Submit state; fails
        tx.send(PersistenceMsg::State(payload)).unwrap();
        assert!(
            sync_rx
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );

        // Send Shutdown: must report error and NOT terminate worker
        let (ack_tx1, ack_rx1) = std::sync::mpsc::channel();
        tx.send(PersistenceMsg::Shutdown(ack_tx1)).unwrap();
        let shut_res1 = ack_rx1.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(
            shut_res1.is_err(),
            "Shutdown must report error when persistence fails"
        );

        // Worker must still be alive and responsive!
        // Restore database
        sqlx::query("DROP TRIGGER force_playback_state_failure_f")
            .execute(&pool)
            .await
            .unwrap();

        // Retry shutdown
        let (ack_tx2, ack_rx2) = std::sync::mpsc::channel();
        tx.send(PersistenceMsg::Shutdown(ack_tx2)).unwrap();
        let shut_res2 = ack_rx2.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(
            shut_res2.is_ok(),
            "Retried shutdown must succeed after DB recovery"
        );

        // Now worker terminates cleanly
        worker_handle
            .join()
            .expect("Worker thread should terminate after successful shutdown");

        // Verify SQLite row exists with 8888ms
        let pos: (i64, Option<String>) =
            sqlx::query_as("SELECT position_ms, current_track_id FROM playback_state WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pos.0, 8888);
        assert_eq!(pos.1.as_deref(), Some("shutdown_track"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_persistence_test_g_worker_unavailable_reported() {
        let dir = std::env::temp_dir().join(format!("cavia_p_test_g_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        let (tx, rx) = std::sync::mpsc::channel::<PersistenceMsg>();
        let health = Arc::new(RwLock::new(PersistenceHealthInfo::default()));
        let health_clone = Arc::clone(&health);
        let pool_worker = pool.clone();

        // Force write to fail so worker has a pending payload
        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_g \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected failure G'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let hook_sync = Arc::new(move |res: Result<(), &str>| {
            let _ = sync_tx.send(res.map_err(|s| s.to_string()));
        });

        let worker_handle = std::thread::spawn(move || {
            run_persistence_worker(rx, pool_worker, None, health_clone, Some(hook_sync));
        });

        let payload = PersistencePayload {
            queue_json: "[]".into(),
            current_track_id: None,
            position_ms: 100,
            repeat_mode: "off".into(),
            shuffle_enabled: 0,
            playback_speed: 1.0,
            volume: 80,
            is_muted: 0,
        };

        tx.send(PersistenceMsg::State(payload)).unwrap();
        assert!(
            sync_rx
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );

        // Drop tx to simulate channel disconnection while write is unpersisted
        drop(tx);
        worker_handle.join().unwrap();

        // Worker health must report WorkerUnavailable
        assert_eq!(
            health.read().unwrap().status,
            PersistenceHealthStatus::WorkerUnavailable
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_position_preservation_after_restore_failure_and_shutdown() {
        let dir = std::env::temp_dir().join(format!("cavia_restore_fail_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        // Populate database with saved state at 4200ms with a missing file
        let track = dummy_track(
            "saved1",
            "Saved Track",
            Some("Z:/ExternalDrive/missing_track.flac"),
        );
        let queue_json = serde_json::to_string(&vec![track.clone()]).unwrap();

        sqlx::query(
            "INSERT INTO playback_state (id, queue_json, current_track_id, position_ms, repeat_mode, shuffle_enabled, playback_speed, volume, is_muted) \
             VALUES (1, ?, 'saved1', 4200, 'off', 0, 1.0, 75, 0)"
        )
        .bind(&queue_json)
        .execute(&pool)
        .await
        .unwrap();

        // Boot service and restore state
        let service = PlaybackService::new(pool.clone(), None).unwrap();
        service.restore_saved_state().await.unwrap();

        // Snapshot must preserve the 4200ms position and report the error honestly
        let snap = service.snapshot();
        assert_eq!(
            snap.position_ms, 4200,
            "Position must NOT be zeroed after restore failure"
        );
        assert_eq!(
            snap.queue.len(),
            1,
            "Queue must NOT be discarded after restore failure"
        );
        assert_eq!(snap.current_track.as_ref().unwrap().id, "saved1");
        assert!(
            snap.error_message.is_some(),
            "Restoration failure must be surfaced"
        );

        // Immediately shut down
        service.shutdown().await.unwrap();

        // Verify SQLite database STILL contains 4200ms and the full queue!
        let row: (i64, String) =
            sqlx::query_as("SELECT position_ms, queue_json FROM playback_state WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            row.0, 4200,
            "Shutdown must preserve the saved position even after restore failure"
        );
        assert!(
            row.1.contains("saved1"),
            "Queue metadata must remain intact in SQLite"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_restore_preserves_queue_when_external_drive_missing() {
        let dir =
            std::env::temp_dir().join(format!("cavia_restore_drive_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;

        let t1 = dummy_track("d1", "Drive Track 1", Some("X:/Music/t1.flac"));
        let t2 = dummy_track("d2", "Drive Track 2", Some("X:/Music/t2.flac"));
        let queue_json = serde_json::to_string(&vec![t1, t2]).unwrap();

        sqlx::query(
            "INSERT INTO playback_state (id, queue_json, current_track_id, position_ms, repeat_mode, shuffle_enabled, playback_speed, volume, is_muted) \
             VALUES (1, ?, 'd1', 8800, 'all', 1, 1.0, 80, 0)"
        )
        .bind(&queue_json)
        .execute(&pool)
        .await
        .unwrap();

        let service = PlaybackService::new(pool.clone(), None).unwrap();
        service.restore_saved_state().await.unwrap();

        let snap = service.snapshot();
        assert_eq!(
            snap.queue.len(),
            2,
            "All queue entries from unmounted volume must be preserved"
        );
        assert_eq!(snap.position_ms, 8800);
        assert!(
            snap.error_message.as_ref().unwrap().contains("unavailable")
                || snap.error_message.as_ref().unwrap().contains("not found")
        );

        service.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_concurrency_race_deterministic() {
        let dir = std::env::temp_dir().join(format!("cavia_race_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool, None).unwrap();

        // Enqueue command and shutdown concurrently
        let h1 = {
            let s = service.clone();
            tokio::spawn(async move { s.set_volume(65).await })
        };
        let h2 = {
            let s = service.clone();
            tokio::spawn(async move { s.shutdown().await })
        };

        let (res1, res2) = tokio::join!(h1, h2);
        // h1 either executed before shutdown or was rejected if shutdown flag won; either way it returns a defined Result, not hanging!
        assert!(res1.is_ok());
        assert!(res2.unwrap().is_ok());

        assert!(service.is_shutting_down());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_multiple_shutdown_calls_idempotent() {
        let dir = std::env::temp_dir().join(format!("cavia_multi_shut_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool, None).unwrap();

        // Concurrently call shutdown 5 times
        let mut handles = Vec::new();
        for _ in 0..5 {
            let s = service.clone();
            handles.push(tokio::spawn(async move { s.shutdown().await }));
        }

        for h in handles {
            let res = h.await.unwrap();
            assert!(
                res.is_ok(),
                "Concurrent shutdown calls must succeed idempotently"
            );
        }

        // Subsequent sequential call
        assert!(service.shutdown().await.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_shutdown_rejects_subsequent_commands() {
        let dir = std::env::temp_dir().join(format!("cavia_reject_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool, None).unwrap();

        service.shutdown().await.unwrap();
        assert!(service.is_shutting_down());

        let res = service.set_volume(50).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Playback service is shutting down");

        let res2 = service.play().await;
        assert!(res2.is_err());

        let _ = std::fs::remove_dir_all(&dir);
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
        let _ = tokio::time::timeout(Duration::from_secs(10), shutdown)
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
        service.shutdown().await.unwrap();

        let row: (i64, String) =
            sqlx::query_as("SELECT volume, repeat_mode FROM playback_state WHERE id = 1")
                .fetch_one(&pool)
                .await
                .expect("final state was not flushed to SQLite");
        assert_eq!(row.0, 73);
        assert_eq!(row.1, "all");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[derive(Debug, Clone, Default)]
    pub struct MockMediaControlsState {
        pub attached: bool,
        pub detached: bool,
        pub last_metadata: Option<MediaMetadataDTO>,
        pub last_playback: Option<souvlaki::MediaPlayback>,
        pub fail_attach: bool,
        pub fail_set_metadata: bool,
        pub fail_set_playback: bool,
    }

    pub struct MockMediaControlsHost {
        pub state: Arc<std::sync::Mutex<MockMediaControlsState>>,
    }

    impl MockMediaControlsHost {
        pub fn new(state: Arc<std::sync::Mutex<MockMediaControlsState>>) -> Self {
            Self { state }
        }
    }

    impl MediaControlsHost for MockMediaControlsHost {
        fn attach(
            &mut self,
            _event_handler: Box<dyn Fn(souvlaki::MediaControlEvent) + Send + 'static>,
        ) -> Result<(), String> {
            let mut st = self.state.lock().unwrap();
            if st.fail_attach {
                return Err("Mock attach error".to_string());
            }
            st.attached = true;
            Ok(())
        }

        fn detach(&mut self) -> Result<(), String> {
            let mut st = self.state.lock().unwrap();
            st.detached = true;
            st.attached = false;
            Ok(())
        }

        fn set_playback(&mut self, playback: souvlaki::MediaPlayback) -> Result<(), String> {
            let mut st = self.state.lock().unwrap();
            if st.fail_set_playback {
                return Err("Mock set_playback error".to_string());
            }
            st.last_playback = Some(playback);
            Ok(())
        }

        fn set_metadata(&mut self, metadata: MediaMetadataDTO) -> Result<(), String> {
            let mut st = self.state.lock().unwrap();
            if st.fail_set_metadata {
                return Err("Mock set_metadata error".to_string());
            }
            st.last_metadata = Some(metadata);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_smtc_test_a_successful_initialization() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        let state = Arc::new(std::sync::Mutex::new(MockMediaControlsState::default()));
        let host = Box::new(MockMediaControlsHost::new(state.clone()));

        let res = core.set_media_controls_host(host);
        assert!(res.is_ok());

        let snap = core.snapshot();
        assert_eq!(snap.smtc_status, "ready");
        assert_eq!(snap.system_media_controls, true);
        assert!(snap.smtc_error.is_none());
        assert!(state.lock().unwrap().attached);
    }

    #[tokio::test]
    async fn test_smtc_test_b_attachment_failure() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        let (cmd_tx, _cmd_rx) = std::sync::mpsc::channel();
        core.set_cmd_tx(cmd_tx);

        let state = Arc::new(std::sync::Mutex::new(MockMediaControlsState {
            fail_attach: true,
            ..Default::default()
        }));
        let host = Box::new(MockMediaControlsHost::new(state));

        let res = core.set_media_controls_host(host);
        assert!(res.is_err(), "Attachment failure must return Err");

        let snap = core.snapshot();
        assert_eq!(snap.smtc_status, "unavailable");
        assert_eq!(snap.system_media_controls, false);
        assert!(snap.smtc_error.is_some());
    }

    #[tokio::test]
    async fn test_smtc_test_c_metadata_update_failure() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 5000,
            position_ms: 0,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        let state = Arc::new(std::sync::Mutex::new(MockMediaControlsState::default()));
        let host = Box::new(MockMediaControlsHost::new(state.clone()));
        assert!(core.set_media_controls_host(host).is_ok());

        // Make set_metadata fail
        state.lock().unwrap().fail_set_metadata = true;

        let track = dummy_track("track-smtc-c", "Metadata Fail Track", None);
        core.queue
            .set_queue(vec![track.clone()], Some("track-smtc-c"));

        // Updating media metadata returns error
        let res = core.update_media_metadata(&track);
        assert!(res.is_err());

        // PROVING THE DEFECT: In the un-remediated implementation, status stayed ready!
        let snap = core.snapshot();
        assert_eq!(
            snap.smtc_status, "error",
            "SMTC status must be error on metadata failure"
        );
        assert_eq!(
            snap.system_media_controls, false,
            "system_media_controls must be false on metadata failure"
        );
        assert!(snap.smtc_error.is_some(), "smtc_error must be recorded");
        // Audio must be unaffected
        assert_eq!(snap.backend_health, "ready");
    }

    #[tokio::test]
    async fn test_smtc_test_d_playback_synchronization_failure() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Playing,
            duration_ms: 5000,
            position_ms: 1000,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        let state = Arc::new(std::sync::Mutex::new(MockMediaControlsState::default()));
        let host = Box::new(MockMediaControlsHost::new(state.clone()));
        assert!(core.set_media_controls_host(host).is_ok());

        // Make set_playback fail
        state.lock().unwrap().fail_set_playback = true;

        // Trigger playback sync
        let res = core.sync_media_playback_state();
        assert!(res.is_err());

        // PROVING THE DEFECT: In the un-remediated implementation, status stayed ready!
        let snap = core.snapshot();
        assert_eq!(
            snap.smtc_status, "error",
            "SMTC status must be error on playback sync failure"
        );
        assert_eq!(
            snap.system_media_controls, false,
            "system_media_controls must be false on playback sync failure"
        );
        assert!(snap.smtc_error.is_some(), "smtc_error must be recorded");
        // Audio playback remains playing and healthy
        assert_eq!(snap.status, "playing");
        assert_eq!(snap.backend_health, "ready");
    }

    #[tokio::test]
    async fn test_smtc_test_e_runtime_status_refresh() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Playing,
            duration_ms: 5000,
            position_ms: 1000,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        let state = Arc::new(std::sync::Mutex::new(MockMediaControlsState::default()));
        let host = Box::new(MockMediaControlsHost::new(state.clone()));
        assert!(core.set_media_controls_host(host).is_ok());

        // Fail playback sync
        state.lock().unwrap().fail_set_playback = true;
        let _ = core.sync_media_playback_state();
        assert_eq!(core.snapshot().smtc_status, "error");

        // Now recover: clear failure flag
        state.lock().unwrap().fail_set_playback = false;
        let res = core.sync_media_playback_state();
        assert!(res.is_ok());

        // Status must refresh to ready
        let snap = core.snapshot();
        assert_eq!(snap.smtc_status, "ready");
        assert_eq!(snap.system_media_controls, true);
        assert!(snap.smtc_error.is_none());
    }

    #[tokio::test]
    async fn test_smtc_test_f_platform_unsupported() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        // Directly simulate unsupported platform path
        core.media_controls_ready = false;
        core.smtc_status = SmtcStatus::Unsupported;
        core.smtc_error = None;
        core.update_snapshot_cache();

        let snap = core.snapshot();
        assert_eq!(snap.smtc_status, "unsupported");
        assert_eq!(snap.system_media_controls, false);
        assert!(snap.smtc_error.is_none());
    }

    #[tokio::test]
    async fn test_smtc_test_g_audio_independence() {
        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Paused,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);
        let state = Arc::new(std::sync::Mutex::new(MockMediaControlsState {
            fail_set_playback: true,
            fail_set_metadata: true,
            ..Default::default()
        }));
        let host = Box::new(MockMediaControlsHost::new(state));
        // Even if set_media_controls_host updates metadata/playback and fails:
        let _ = core.set_media_controls_host(host);

        // Execute audio operations
        assert!(core.play().is_ok());
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);
        assert!(core.pause().is_ok());
        assert_eq!(core.backend.status(), PlaybackStatus::Paused);
        assert!(core.set_volume(50).is_ok());

        let snap = core.snapshot();
        // Audio is completely functional and healthy
        assert_eq!(snap.backend_health, "ready");
        assert_eq!(snap.device_ready, true);
        // SMTC reports failure truthfully
        assert_eq!(snap.smtc_status, "error");
        assert_eq!(snap.system_media_controls, false);
    }

    #[test]
    fn test_smtc_status_audit_no_phantom_initializing() {
        assert_eq!(SmtcStatus::Ready.as_str(), "ready");
        assert_eq!(SmtcStatus::Unavailable.as_str(), "unavailable");
        assert_eq!(SmtcStatus::Unsupported.as_str(), "unsupported");
        assert_eq!(SmtcStatus::Error.as_str(), "error");

        // Phantom 'initializing' state is completely eliminated
        let parse_init: Result<SmtcStatus, _> = serde_json::from_str(r#""initializing""#);
        assert!(
            parse_init.is_err(),
            "Phantom 'initializing' state must not exist in SmtcStatus"
        );
    }

    pub struct TransactionalTestBackend {
        pub status: PlaybackStatus,
        pub pass_validate: bool,
        pub fail_load: bool,
        pub fail_device: bool,
        pub fail_seek: bool,
        pub position_ms: u64,
        pub duration_ms: u64,
    }

    impl AudioBackend for TransactionalTestBackend {
        fn name(&self) -> &'static str {
            "transactional_mock"
        }
        fn load(&mut self, _path: &str, start_ms: u64) -> Result<(), PlaybackError> {
            if self.fail_device {
                return Err(PlaybackError::DeviceError("Device error on load".into()));
            }
            if self.fail_load {
                return Err(PlaybackError::DecoderError("Decoder error on load".into()));
            }
            if self.fail_seek && start_ms > 0 {
                return Err(PlaybackError::SeekError("Initial seek failed".into()));
            }
            self.status = PlaybackStatus::Playing;
            Ok(())
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
        fn validate_source(&self, _path: &str) -> Result<(), PlaybackError> {
            if self.pass_validate {
                Ok(())
            } else {
                Err(PlaybackError::DecoderError(
                    "Validation probe failed".into(),
                ))
            }
        }
        fn is_device_ready(&self) -> bool {
            !self.fail_device
        }
    }

    #[tokio::test]
    async fn test_queue_test_a_missing_candidate() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_q_a_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 2.0);

        let backend = Box::new(TransactionalTestBackend {
            status: PlaybackStatus::Playing,
            pass_validate: true,
            fail_load: false,
            fail_device: false,
            fail_seek: false,
            position_ms: 500,
            duration_ms: 2000,
        });
        let mut core = new_test_core(backend);

        let t1 = dummy_track("t1", "Track 1", Some(good_wav.to_str().unwrap()));
        core.queue.set_queue(vec![t1.clone()], Some("t1"));

        let missing = dummy_track("missing", "Missing", Some("C:/non_existent_cavia_q_a.flac"));
        let res = core.set_queue(vec![missing], Some("missing"));
        assert!(res.is_err());

        // Previous playback session must remain intact
        assert_eq!(core.queue.current_track().unwrap().id, "t1");
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);
        assert_eq!(core.snapshot().status, "playing");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_queue_test_b_corrupt_header() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_q_b_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 2.0);

        let corrupt_wav = temp_dir.join("corrupt.wav");
        std::fs::write(&corrupt_wav, b"GARBAGE_HEADER_123456789").unwrap();

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let t1 = dummy_track("t1", "Track 1", Some(good_wav.to_str().unwrap()));
        core.set_queue(vec![t1.clone()], Some("t1")).unwrap();
        assert!(core.play().is_ok());
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);

        let bad_track = dummy_track("bad", "Bad Track", Some(corrupt_wav.to_str().unwrap()));
        let res = core.set_queue(vec![bad_track], Some("bad"));
        assert!(res.is_err());

        assert_eq!(core.queue.current_track().unwrap().id, "t1");
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);
        assert_eq!(core.snapshot().status, "playing");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_queue_test_c_truncated_payload() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_q_c_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 2.0);

        let trunc_wav = temp_dir.join("truncated.wav");
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&trunc_wav).unwrap();
            file.write_all(b"RIFF").unwrap();
            file.write_all(&(36u32 + 50000).to_le_bytes()).unwrap();
            file.write_all(b"WAVEfmt ").unwrap();
            file.write_all(&16u32.to_le_bytes()).unwrap();
            file.write_all(&1u16.to_le_bytes()).unwrap();
            file.write_all(&1u16.to_le_bytes()).unwrap();
            file.write_all(&44100u32.to_le_bytes()).unwrap();
            file.write_all(&(44100u32 * 2).to_le_bytes()).unwrap();
            file.write_all(&2u16.to_le_bytes()).unwrap();
            file.write_all(&16u16.to_le_bytes()).unwrap();
            file.write_all(b"data").unwrap();
            file.write_all(&50000u32.to_le_bytes()).unwrap();
        }

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let t1 = dummy_track("t1", "Track 1", Some(good_wav.to_str().unwrap()));
        core.set_queue(vec![t1.clone()], Some("t1")).unwrap();
        assert!(core.play().is_ok());
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);

        let trunc_track = dummy_track("trunc", "Truncated", Some(trunc_wav.to_str().unwrap()));
        let res = core.set_queue(vec![trunc_track], Some("trunc"));
        assert!(res.is_err());

        assert_eq!(core.queue.current_track().unwrap().id, "t1");
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);
        assert_eq!(core.snapshot().status, "playing");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_queue_test_d_validation_succeeds_actual_loading_fails() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_q_d_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 2.0);
        let cand_wav = temp_dir.join("cand.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&cand_wav, 2.0);

        let backend = Box::new(TransactionalTestBackend {
            status: PlaybackStatus::Playing,
            pass_validate: true,
            fail_load: false,
            fail_device: false,
            fail_seek: false,
            position_ms: 1000,
            duration_ms: 2000,
        });
        let mut core = new_test_core(backend);

        let t1 = dummy_track("t1", "Track 1", Some(good_wav.to_str().unwrap()));
        core.queue.set_queue(vec![t1.clone()], Some("t1"));
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);

        // Simulate: validation passes, but subsequent backend load fails!
        // We configure fail_load = true via unsafe or direct struct access:
        // Since core.backend is Box<dyn AudioBackend>, let's test directly with TransactionalTestBackend
        // Or configure a backend whose second load fails:
        struct FailSecondLoadBackend {
            loads_attempted: usize,
            status: PlaybackStatus,
        }
        impl AudioBackend for FailSecondLoadBackend {
            fn name(&self) -> &'static str {
                "fail_second"
            }
            fn load(&mut self, _path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
                self.loads_attempted += 1;
                if self.loads_attempted > 0 {
                    // Second load fails, but does NOT stop previous player!
                    Err(PlaybackError::DecoderError(
                        "Candidate decoder failed during load".into(),
                    ))
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
            fn seek(&mut self, _pos: u64) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn set_volume(&mut self, _vol: f32) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn set_speed(&mut self, _spd: f32) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn position_ms(&self) -> u64 {
                1000
            }
            fn duration_ms(&self) -> u64 {
                2000
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
            fn validate_source(&self, _path: &str) -> Result<(), PlaybackError> {
                // Validation explicitly SUCCEEDS!
                Ok(())
            }
        }

        let backend = Box::new(FailSecondLoadBackend {
            loads_attempted: 0,
            status: PlaybackStatus::Playing,
        });
        let mut core = new_test_core(backend);
        core.queue.set_queue(vec![t1.clone()], Some("t1"));
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);

        let cand_track = dummy_track("cand", "Candidate", Some(cand_wav.to_str().unwrap()));
        let res = core.set_queue(vec![cand_track], Some("cand"));
        assert!(
            res.is_err(),
            "set_queue must return Err when backend load fails"
        );

        // PROVING THE DEFECT: In un-remediated code, stop() was called before load,
        // so core.backend.status() is Stopped instead of Playing!
        assert_eq!(
            core.backend.status(),
            PlaybackStatus::Playing,
            "Active playback session must remain Playing when candidate load fails"
        );
        assert_eq!(core.snapshot().status, "playing");
        assert_eq!(core.queue.current_track().unwrap().id, "t1");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_queue_test_e_initial_seek_failure() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_q_e_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 2.0);

        struct FailSeekBackend {
            status: PlaybackStatus,
        }
        impl AudioBackend for FailSeekBackend {
            fn name(&self) -> &'static str {
                "fail_seek"
            }
            fn load(&mut self, _path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
                // Seek during load fails!
                Err(PlaybackError::SeekError("Initial seek failed".into()))
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
            fn seek(&mut self, _pos: u64) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn set_volume(&mut self, _vol: f32) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn set_speed(&mut self, _spd: f32) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn position_ms(&self) -> u64 {
                500
            }
            fn duration_ms(&self) -> u64 {
                2000
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
            fn validate_source(&self, _path: &str) -> Result<(), PlaybackError> {
                Ok(())
            }
        }

        let backend = Box::new(FailSeekBackend {
            status: PlaybackStatus::Playing,
        });
        let mut core = new_test_core(backend);
        let t1 = dummy_track("t1", "Track 1", Some(good_wav.to_str().unwrap()));
        core.queue.set_queue(vec![t1.clone()], Some("t1"));

        let cand = dummy_track("cand", "Cand", Some(good_wav.to_str().unwrap()));
        let res = core.set_queue(vec![cand], Some("cand"));
        assert!(res.is_err());

        assert_eq!(core.backend.status(), PlaybackStatus::Playing);
        assert_eq!(core.queue.current_track().unwrap().id, "t1");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_queue_test_f_device_unavailable() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_q_f_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 2.0);

        struct DeviceFailBackend {
            status: PlaybackStatus,
        }
        impl AudioBackend for DeviceFailBackend {
            fn name(&self) -> &'static str {
                "device_fail"
            }
            fn load(&mut self, _path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
                Err(PlaybackError::DeviceError(
                    "No audio device available".into(),
                ))
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
            fn seek(&mut self, _pos: u64) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn set_volume(&mut self, _vol: f32) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn set_speed(&mut self, _spd: f32) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn position_ms(&self) -> u64 {
                500
            }
            fn duration_ms(&self) -> u64 {
                2000
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
            fn validate_source(&self, _path: &str) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn is_device_ready(&self) -> bool {
                false
            }
        }

        let backend = Box::new(DeviceFailBackend {
            status: PlaybackStatus::Playing,
        });
        let mut core = new_test_core(backend);
        let t1 = dummy_track("t1", "Track 1", Some(good_wav.to_str().unwrap()));
        core.queue.set_queue(vec![t1.clone()], Some("t1"));

        let cand = dummy_track("cand", "Cand", Some(good_wav.to_str().unwrap()));
        let res = core.set_queue(vec![cand], Some("cand"));
        assert!(res.is_err());

        assert_eq!(core.queue.current_track().unwrap().id, "t1");
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_queue_test_g_successful_replacement() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_q_g_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav1 = temp_dir.join("good1.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav1, 2.0);
        let good_wav2 = temp_dir.join("good2.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav2, 3.0);

        let backend = Box::new(crate::audio::rodio_backend::RodioBackend::new().unwrap());
        let mut core = new_test_core(backend);

        let t1 = dummy_track("t1", "Track 1", Some(good_wav1.to_str().unwrap()));
        core.set_queue(vec![t1.clone()], Some("t1")).unwrap();
        assert!(core.play().is_ok());

        let t2 = dummy_track("t2", "Track 2", Some(good_wav2.to_str().unwrap()));
        let res = core.set_queue(vec![t2.clone()], Some("t2"));
        assert!(res.is_ok());

        assert_eq!(core.queue.current_track().unwrap().id, "t2");
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);
        assert_eq!(core.snapshot().current_track.unwrap().id, "t2");
        assert_eq!(core.snapshot().status, "playing");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_queue_test_h_play_track_failure_preserves_session() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_test_q_h_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let good_wav = temp_dir.join("good.wav");
        crate::audio::rodio_backend::tests::create_test_wav_with_duration(&good_wav, 2.0);

        struct FailOnSecondTrackBackend {
            status: PlaybackStatus,
        }
        impl AudioBackend for FailOnSecondTrackBackend {
            fn name(&self) -> &'static str {
                "fail_t2"
            }
            fn load(&mut self, path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
                if path.contains("bad") {
                    Err(PlaybackError::DecoderError(
                        "Track 2 failed to decode".into(),
                    ))
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
            fn seek(&mut self, _pos: u64) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn set_volume(&mut self, _vol: f32) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn set_speed(&mut self, _spd: f32) -> Result<(), PlaybackError> {
                Ok(())
            }
            fn position_ms(&self) -> u64 {
                750
            }
            fn duration_ms(&self) -> u64 {
                2000
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
            fn validate_source(&self, _path: &str) -> Result<(), PlaybackError> {
                Ok(())
            }
        }

        let backend = Box::new(FailOnSecondTrackBackend {
            status: PlaybackStatus::Playing,
        });
        let mut core = new_test_core(backend);

        let t1 = dummy_track("t1", "Track 1", Some(good_wav.to_str().unwrap()));
        let t2 = dummy_track("t2", "Track 2", Some("C:/non_existent_bad.flac"));
        core.queue
            .set_queue(vec![t1.clone(), t2.clone()], Some("t1"));
        assert_eq!(core.backend.status(), PlaybackStatus::Playing);

        // Attempting to play t2 fails
        let res = core.play_track("t2");
        assert!(res.is_err());

        // PROVING THE DEFECT: In un-remediated code, stop() was called before load(),
        // so core.backend.status() was set to Stopped!
        assert_eq!(
            core.backend.status(),
            PlaybackStatus::Playing,
            "Current track playback must not be stopped when next track fails to load"
        );
        assert_eq!(core.queue.current_track().unwrap().id, "t1");
        assert_eq!(core.snapshot().status, "playing");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn offline_fallback_preserves_original_queue_stops_when_exhausted_and_restores_cleanly() {
        let temp_dir =
            std::env::temp_dir().join(format!("cavia_offline_core_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wav_a = temp_dir.join("cached_a.wav");
        let wav_b = temp_dir.join("cached_b.wav");
        std::fs::write(&wav_a, minimal_wav()).unwrap();
        std::fs::write(&wav_b, minimal_wav()).unwrap();

        let backend = Box::new(TestMockBackend {
            status: PlaybackStatus::Idle,
            duration_ms: 1000,
            position_ms: 0,
            should_fail_load: false,
        });
        let mut core = new_test_core(backend);

        let (coord_tx, mut coord_rx) = tokio::sync::mpsc::unbounded_channel();
        let armed = Arc::new(AtomicBool::new(false));
        core.set_coordinator_tx(coord_tx, Arc::clone(&armed));

        let t1 = dummy_track("orig1", "Orig 1", Some(wav_a.to_str().unwrap()));
        let t2 = dummy_track("orig2", "Orig 2 Uncached", None);
        let t3 = dummy_track("orig3", "Orig 3", Some(wav_b.to_str().unwrap()));
        core.queue
            .set_queue(vec![t1, t2, t3.clone()], Some("orig1"));
        core.play_index(0).unwrap();

        let saved = core.queue.snapshot_state();
        let mut fallback_track = t3.clone();
        fallback_track.queue_entry_id = t3.queue_entry_id.clone();

        // Enter offline fallback while track 0 is actively playing -> preserves current track and
        // swaps only upcoming tail without losing original queue snapshot.
        core.enter_offline_fallback(vec![fallback_track.clone()])
            .unwrap();
        assert!(core.fallback_active);
        assert_eq!(core.queue.current_track().unwrap().id, "orig1");

        // Advance to fallback track, then exhaust pool -> must stop instead of looping.
        core.next().unwrap();
        assert_eq!(core.queue.current_track().unwrap().id, "orig3");
        core.next().unwrap();
        let mut saw_exhausted = false;
        while let Ok(ev) = coord_rx.try_recv() {
            if matches!(ev, CoordinatorEvent::FallbackExhausted) {
                saw_exhausted = true;
            }
        }
        assert!(
            saw_exhausted,
            "Offline fallback exhaustion must notify coordinator rather than looping"
        );

        // Simulate reconnection armed and exiting fallback -> restores original queue advancing
        // past tracks already heard during fallback.
        let mut played_ids = HashSet::new();
        played_ids.insert("orig3".to_string());
        core.exit_offline_fallback(saved, HashSet::new(), played_ids, false)
            .unwrap();
        assert!(!core.fallback_active);
        assert_eq!(core.queue.len(), 3, "Original queue items must stay intact");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn show_connect_id_controls_os_and_bluetooth_metadata_visibility() {
        assert_eq!(
            format_album_for_os_metadata("Random Access Memories", Some("H4K7MN")),
            "Random Access Memories · Connect: H4K7MN"
        );
        assert_eq!(
            format_album_for_os_metadata("", Some("H4K7MN")),
            "Cavia Connect · H4K7MN"
        );
        assert_eq!(
            format_album_for_os_metadata("Random Access Memories", None),
            "Random Access Memories"
        );
    }

    fn minimal_wav() -> Vec<u8> {
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
