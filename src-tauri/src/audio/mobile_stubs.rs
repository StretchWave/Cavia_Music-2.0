//! Mobile platform audio backend contracts and staging.
//!
//! Per architecture guidelines and AGENTS.md:
//! Desktop engines must not be pretended to support mobile targets.
//! Android requires Media3 ExoPlayer with MediaSessionService.
//! iOS requires AVPlayer with MPNowPlayingInfoCenter.
//!
//! UNTIL NATIVE MOBILE AUDIO OUTPUT IS AUDIBLY IMPLEMENTED AND VERIFIED ON REAL HARDWARE,
//! THIS ADAPTER HONESTLY REPORTS UNAVAILABLE AND REJECTS PLAYBACK REQUESTS.

use super::{AudioBackend, AudioCapabilities, PlaybackError, PlaybackStatus};

pub struct AndroidMedia3Backend {
    status: PlaybackStatus,
}

impl AndroidMedia3Backend {
    pub fn new() -> Self {
        Self {
            status: PlaybackStatus::Idle,
        }
    }
}

impl Default for AndroidMedia3Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for AndroidMedia3Backend {
    fn name(&self) -> &'static str {
        "android_media3_unintegrated"
    }

    fn load(&mut self, _file_path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Error;
        Err(PlaybackError::BackendUnavailable(
            "Android Media3 native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn play(&mut self) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Error;
        Err(PlaybackError::BackendUnavailable(
            "Android Media3 native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn pause(&mut self) -> Result<(), PlaybackError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Error;
        Err(PlaybackError::BackendUnavailable(
            "Android Media3 native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn stop(&mut self) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Stopped;
        Ok(())
    }

    fn seek(&mut self, _position_ms: u64) -> Result<(), PlaybackError> {
        Err(PlaybackError::BackendUnavailable(
            "Android Media3 native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn set_volume(&mut self, _volume: f32) -> Result<(), PlaybackError> {
        Ok(())
    }

    fn set_speed(&mut self, _speed: f32) -> Result<(), PlaybackError> {
        Ok(())
    }

    fn position_ms(&self) -> u64 {
        0
    }

    fn duration_ms(&self) -> u64 {
        0
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

    fn validate_source(&self, _file_path: &str) -> Result<(), PlaybackError> {
        Err(PlaybackError::BackendUnavailable(
            "Android Media3 native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn is_device_ready(&self) -> bool {
        false
    }

    fn try_init_device(&mut self) -> Result<(), PlaybackError> {
        Err(PlaybackError::BackendUnavailable(
            "Android Media3 native playback engine is not yet integrated into this build.".into(),
        ))
    }
}

pub struct IosAvPlayerBackend {
    status: PlaybackStatus,
}

impl IosAvPlayerBackend {
    pub fn new() -> Self {
        Self {
            status: PlaybackStatus::Idle,
        }
    }
}

impl Default for IosAvPlayerBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for IosAvPlayerBackend {
    fn name(&self) -> &'static str {
        "ios_avplayer_unintegrated"
    }

    fn load(&mut self, _file_path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Error;
        Err(PlaybackError::BackendUnavailable(
            "iOS AVPlayer native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn play(&mut self) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Error;
        Err(PlaybackError::BackendUnavailable(
            "iOS AVPlayer native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn pause(&mut self) -> Result<(), PlaybackError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Error;
        Err(PlaybackError::BackendUnavailable(
            "iOS AVPlayer native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn stop(&mut self) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Stopped;
        Ok(())
    }

    fn seek(&mut self, _position_ms: u64) -> Result<(), PlaybackError> {
        Err(PlaybackError::BackendUnavailable(
            "iOS AVPlayer native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn set_volume(&mut self, _volume: f32) -> Result<(), PlaybackError> {
        Ok(())
    }

    fn set_speed(&mut self, _speed: f32) -> Result<(), PlaybackError> {
        Ok(())
    }

    fn position_ms(&self) -> u64 {
        0
    }

    fn duration_ms(&self) -> u64 {
        0
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

    fn validate_source(&self, _file_path: &str) -> Result<(), PlaybackError> {
        Err(PlaybackError::BackendUnavailable(
            "iOS AVPlayer native playback engine is not yet integrated into this build.".into(),
        ))
    }

    fn is_device_ready(&self) -> bool {
        false
    }

    fn try_init_device(&mut self) -> Result<(), PlaybackError> {
        Err(PlaybackError::BackendUnavailable(
            "iOS AVPlayer native playback engine is not yet integrated into this build.".into(),
        ))
    }
}
