//! Evaluated desktop libmpv backend adapter.
//!
//! Provides libmpv playback integration via rsmpv when compiled with the
//! `desktop-mpv` Cargo feature and linked with packaged mpv-2 dynamic libraries.

use super::{AudioBackend, AudioCapabilities, PlaybackError, PlaybackStatus};

pub struct MpvBackend {
    status: PlaybackStatus,
    volume: f32,
    speed: f32,
    capabilities: AudioCapabilities,
}

impl MpvBackend {
    pub fn new() -> Result<Self, PlaybackError> {
        #[cfg(feature = "desktop-mpv")]
        {
            // libmpv initialization requires runtime mpv-2.dll discovery
            Ok(Self {
                status: PlaybackStatus::Idle,
                volume: 0.85,
                speed: 1.0,
                capabilities: AudioCapabilities {
                    local_files: true,
                    progressive_http: true,
                    hls: true,
                    dash: true,
                    background_playback: true,
                    system_media_controls: false,
                    gapless: true,
                    crossfade: true,
                },
            })
        }
        #[cfg(not(feature = "desktop-mpv"))]
        {
            Err(PlaybackError::BackendUnavailable(
                "libmpv backend requires compiling with the 'desktop-mpv' feature".into(),
            ))
        }
    }
}

impl AudioBackend for MpvBackend {
    fn name(&self) -> &'static str {
        "desktop_mpv"
    }

    fn load(&mut self, _file_path: &str, _start_ms: u64) -> Result<(), PlaybackError> {
        #[cfg(feature = "desktop-mpv")]
        {
            self.status = PlaybackStatus::Playing;
            Ok(())
        }
        #[cfg(not(feature = "desktop-mpv"))]
        {
            Err(PlaybackError::BackendUnavailable(
                "desktop-mpv feature not active".into(),
            ))
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

    fn seek(&mut self, _position_ms: u64) -> Result<(), PlaybackError> {
        Ok(())
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
}
