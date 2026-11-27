//! Optional desktop libmpv backend placeholder.
//!
//! `rsmpv` integration is not yet wired to libmpv playback calls in this build;
//! per `AGENTS.md`, this adapter honestly reports `BackendUnavailable` rather
//! than simulating playback.

use super::{AudioBackend, AudioCapabilities, PlaybackError, PlaybackStatus};

pub struct MpvBackend {
    status: PlaybackStatus,
    volume: f32,
    speed: f32,
    capabilities: AudioCapabilities,
}

impl MpvBackend {
    pub fn new() -> Result<Self, PlaybackError> {
        let _ = (PlaybackStatus::Idle, 0.85_f32, 1.0_f32);
        Err(PlaybackError::BackendUnavailable(
            "libmpv (rsmpv) playback backend is not integrated in this build".into(),
        ))
    }
}

impl AudioBackend for MpvBackend {
    fn name(&self) -> &'static str {
        "desktop_mpv_unintegrated"
    }

    fn load(&mut self, _file_path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Error;
        Err(PlaybackError::BackendUnavailable(
            "libmpv (rsmpv) playback backend is not integrated in this build".into(),
        ))
    }

    fn play(&mut self) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Error;
        Err(PlaybackError::BackendUnavailable(
            "libmpv (rsmpv) playback backend is not integrated in this build".into(),
        ))
    }

    fn pause(&mut self) -> Result<(), PlaybackError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), PlaybackError> {
        self.play()
    }

    fn stop(&mut self) -> Result<(), PlaybackError> {
        self.status = PlaybackStatus::Stopped;
        Ok(())
    }

    fn seek(&mut self, _position_ms: u64) -> Result<(), PlaybackError> {
        Err(PlaybackError::BackendUnavailable(
            "libmpv (rsmpv) playback backend is not integrated in this build".into(),
        ))
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), PlaybackError> {
        self.volume = volume;
        Ok(())
    }

    fn set_speed(&mut self, speed: f32) -> Result<(), PlaybackError> {
        self.speed = speed;
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
        self.capabilities.clone()
    }

    fn is_device_ready(&self) -> bool {
        false
    }
}
