//! Audio backend boundary for Cavia Music.
//!
//! The UI and library domain must depend on this contract, never directly on
//! libmpv, ExoPlayer, AVPlayer, or a platform media-session API. Concrete
//! adapters are intentionally not wired until their platform build and
//! licensing requirements have been verified.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioBackendState {
    NotIntegrated,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
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
    /// Safe baseline while no real backend is installed. Never advertise a
    /// feature solely because the UI contains a control for it.
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
