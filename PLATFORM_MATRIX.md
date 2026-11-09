# Platform support matrix

| Capability | Windows | macOS | Android | iOS |
|---|---|---|---|---|
| Shared React UI | Operational (Verified 360px–1920px) | Operational (Responsive) | Operational (Touch optimized) | Operational (Touch optimized) |
| Tauri shell | Operational (Tauri 2) | Operational (Tauri 2) | Staged (Tauri 2 Mobile) | Staged (Tauri 2 Mobile) |
| SQLite local library | Operational (Verified SQLx WAL mode) | Operational (Shared SQLite core) | Staged | Staged |
| Native folder / file picking | Operational (`@tauri-apps/plugin-dialog`) | Operational (`@tauri-apps/plugin-dialog`) | URI staging still required | Sandbox / security-scoped URL staging required |
| Metadata extraction | Operational (Lofty: FLAC, MP3, WAV, AAC, M4A, OGG) | Operational (Lofty) | Staged after URI staging | Staged after sandbox staging |
| Local audio playback | Operational (Rodio WASAPI, native seeking) | Operational (Rodio CoreAudio) | Stub (Explicit BackendUnavailable) | Stub (Explicit BackendUnavailable) |
| Audio device failure resilience | Operational (Headless/device fallback) | Operational | N/A | N/A |
| Network audio playback | Standby | Standby | Not implemented | Not implemented |
| Background playback | Operational (Desktop process) | Operational (Desktop process) | Standby (Needs Media3 Foreground Service) | Standby (Needs AVAudioSession) |
| System media controls | Operational (`souvlaki` SMTC) | Operational (`souvlaki` Now Playing) | Standby (Needs Media3 MediaSession) | Standby (Needs MPRemoteCommandCenter) |

*Note: Android and iOS backends are explicitly stubbed with honest errors (`AudioCapabilities::unavailable()`, returning `Err(PlaybackError::BackendUnavailable)`) rather than silent mock successes.*
