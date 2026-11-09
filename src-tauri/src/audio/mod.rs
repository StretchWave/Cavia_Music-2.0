//! Audio backend boundary and contract for Cavia Music.
//!
//! UI and services communicate through this common interface.
//! Backends are target-specific:
//! - Desktop (Windows/macOS): Zero-dependency native Rodio (CPAL WASAPI/CoreAudio + Symphonia decoders).
//! - Desktop MPV (optional): rsmpv bindings when packaged.
//! - Android: Media3 ExoPlayer with MediaSessionService.
//! - iOS: AVPlayer with MPNowPlayingInfoCenter.

pub mod mobile_stubs;
pub mod rodio_backend;

#[cfg(feature = "desktop-mpv")]
pub mod mpv_backend;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStatus {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackError {
    FileNotFound(String),
    DecoderError(String),
    DeviceError(String),
    UnsupportedFormat(String),
    SeekError(String),
    BackendUnavailable(String),
}

impl std::fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileNotFound(path) => write!(f, "Audio source file not found: {path}"),
            Self::DecoderError(err) => write!(f, "Audio decoding failed: {err}"),
            Self::DeviceError(err) => write!(f, "Audio output device error: {err}"),
            Self::UnsupportedFormat(fmt) => write!(f, "Unsupported audio format: {fmt}"),
            Self::SeekError(err) => write!(f, "Audio seek failed: {err}"),
            Self::BackendUnavailable(err) => write!(f, "Audio backend unavailable: {err}"),
        }
    }
}

impl std::error::Error for PlaybackError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioCapabilities {
    pub local_files: bool,
    pub progressive_http: bool,
    pub hls: bool,
    pub dash: bool,
    pub background_playback: bool,
    pub system_media_controls: bool,
    pub gapless: bool,
    pub crossfade: bool,
}

impl AudioCapabilities {
    pub const fn unavailable() -> Self {
        Self {
            local_files: false,
            progressive_http: false,
            hls: false,
            dash: false,
            background_playback: false,
            system_media_controls: false,
            gapless: false,
            crossfade: false,
        }
    }
}

/// The common Cavia playback contract implemented by all platform audio engines.
pub trait AudioBackend: Send {
    fn name(&self) -> &'static str;
    fn load(&mut self, file_path: &str, start_ms: u64) -> Result<(), PlaybackError>;
    fn play(&mut self) -> Result<(), PlaybackError>;
    fn pause(&mut self) -> Result<(), PlaybackError>;
    fn resume(&mut self) -> Result<(), PlaybackError>;
    fn stop(&mut self) -> Result<(), PlaybackError>;
    fn seek(&mut self, position_ms: u64) -> Result<(), PlaybackError>;
    fn set_volume(&mut self, volume: f32) -> Result<(), PlaybackError>;
    fn set_speed(&mut self, speed: f32) -> Result<(), PlaybackError>;
    fn position_ms(&self) -> u64;
    fn duration_ms(&self) -> u64;
    fn status(&self) -> PlaybackStatus;
    fn is_ended(&self) -> bool;
    fn capabilities(&self) -> AudioCapabilities;
    /// Cheap decoder probe used before committing a new queue. Backends that
    /// cannot validate a source without opening an output device may return
    /// `Ok(())` and rely on `load`; the Rodio backend performs a real decode
    /// probe here so a corrupt file cannot replace a working queue.
    fn validate_source(&self, _path: &str) -> Result<(), PlaybackError> {
        Ok(())
    }
    fn is_device_ready(&self) -> bool {
        true
    }
    fn try_init_device(&mut self) -> Result<(), PlaybackError> {
        Ok(())
    }
}

/// Factory initializing the active platform audio backend.
pub fn create_platform_backend() -> Result<Box<dyn AudioBackend>, PlaybackError> {
    #[cfg(target_os = "android")]
    {
        return Ok(Box::new(mobile_stubs::AndroidMedia3Backend::new()));
    }

    #[cfg(target_os = "ios")]
    {
        return Ok(Box::new(mobile_stubs::IosAvPlayerBackend::new()));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // Desktop default: zero-dependency Rodio native backend
        match rodio_backend::RodioBackend::new() {
            Ok(backend) => Ok(Box::new(backend)),
            Err(e) => {
                eprintln!("[AudioBackend] Failed to initialize native Rodio backend: {e}");
                Err(e)
            }
        }
    }
}
