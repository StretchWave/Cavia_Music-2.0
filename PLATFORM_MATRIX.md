# Platform support matrix

Legend: **Verified** means exercised in this remediation on the stated platform. **Implemented, runtime-unverified** means code exists and compiles for that target but was not run here. **Not integrated** means an honest stub returns `BackendUnavailable` and reports `not_integrated`. **Unsupported** means no implementation and no claim.

| Capability | Windows | macOS | Android | iOS |
|---|---|---|---|---|
| Shared React UI | Implemented; runtime-verified at 1440×900 | Shared code; runtime-unverified | Shared code; runtime-unverified | Shared code; runtime-unverified |
| Tauri shell | Verified (Tauri 2) | Configured; runtime-unverified | Initialized / staged | Initialized / staged |
| SQLite local library | Verified (SQLx WAL, migrations 0001 + 0002) | Shared SQLite code; runtime-unverified | Not integrated | Not integrated |
| Native folder / file picking | Implemented (native dialog) | Shared plugin; runtime-unverified | URI staging required | Security-scoped URL staging required |
| Metadata extraction | Implemented (Lofty) | Shared code; runtime-unverified | Requires platform URI staging | Requires sandbox staging |
| Local audio playback | **Implemented and runtime-verified** (Rodio/WASAPI + Symphonia; 30 s FLAC; UI Play/Pause/Seek/Speed/Volume/Mute; Next/Previous; media-key Play/Pause) | Compiled only; not runtime-verified | **Not integrated** — stub returns `BackendUnavailable`; health `not_integrated` | **Not integrated** — stub returns `BackendUnavailable`; health `not_integrated` |
| Audio backend health | `ready` / `device_unavailable` / `error` | Same logic; runtime-unverified | `not_integrated` | `not_integrated` |
| Startup without an output device | Implemented (non-fatal; UI shows NO AUDIO DEVICE) | Same code; runtime-unverified | N/A | N/A |
| Mid-stream device disconnect recovery | **Unsupported** — no detection; manual retry button only | **Unsupported** | N/A | N/A |
| Network audio playback | Standby | Standby | Not implemented | Not implemented |
| Background playback | Process open | Process open; runtime-unverified | Standby (needs Media3 foreground service) | Standby (needs AVAudioSession) |
| System media controls | SMTC initialized; OS Play/Pause verified; Seek/Next/Previous mapped but not exercised; attach failures surfaced | Standby (needs physical macOS verification) | Standby (needs Media3 MediaSession) | Standby (needs MPRemoteCommandCenter) |

**Verification scope:** Windows 11 desktop, current debug binary launched with the Vite dev server and driven through WebView2 UI Automation. macOS, Android and iOS were not run in this remediation. Android/iOS are explicitly not integrated rather than silently mocked. Mid-stream device-disconnect resilience is not claimed.
