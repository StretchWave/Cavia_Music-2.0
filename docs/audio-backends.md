# Audio Backend Architecture & Evaluation Note

## 1. Shared playback contract

All platform implementations follow one Cavia playback contract (`AudioBackend` in `src-tauri/src/audio/mod.rs`). React communicates only through typed Tauri commands and authoritative events (`playback:state-changed`, `playback:position`); page components never call Rodio, Media3 or AVPlayer directly.

| Target | Active backend | Native media integration | License | Status |
|---|---|---|---|---|
| Windows | Native Rodio (CPAL WASAPI + Symphonia) | Windows SMTC (`souvlaki`) | MIT / Apache-2.0 | **Implemented and runtime-verified**: local UI transport, media-key Play/Pause, persistence, restore |
| macOS | Native Rodio (CPAL CoreAudio + Symphonia) | Standby / deferred | MIT / Apache-2.0 | Compiled only; not runtime-verified |
| Windows (alt) | libmpv via `rsmpv` (feature `desktop-mpv`) | Windows SMTC | GPLv3 / LGPLv2.1+ | Evaluated / optional feature; not bundled |
| Android | Native Media3 ExoPlayer (planned) | MediaSessionService + foreground notification | Apache-2.0 | **Not integrated** — stub returns `BackendUnavailable`, health `not_integrated` |
| iOS | Native AVPlayer (planned) | `MPNowPlayingInfoCenter` + `MPRemoteCommandCenter` | Apple SDK | **Not integrated** — stub returns `BackendUnavailable`, health `not_integrated` |

---

## 2. Desktop engine: native Rodio backend

- **Engine:** `rodio 0.23` with `cpal` (WASAPI on Windows, CoreAudio on macOS) and pure-Rust Symphonia decoders (`symphonia-alac`, `symphonia-aiff` enabled).
- **Audio actor:** all synchronous decoding, file I/O and backend calls run on the dedicated `cavia-audio-control` thread. Tauri commands send `oneshot` requests over an `mpsc` channel; the actor owns `PlaybackCore`, the queue and the backend. The actor loop continues until it explicitly processes `PlaybackCommand::Shutdown`, eliminating lost-reply races.
- **Persistence owner:** a dedicated `cavia-persist` std thread owns a current-thread Tokio runtime. It verifies runtime initialization via handshake, serializes and debounces writes to SQLite, propagates SQLx errors through `write_playback_state() -> Result<(), sqlx::Error>`, and acknowledges `Flush` and `Shutdown` only after writes succeed.
- **Shutdown:** `RunEvent::ExitRequested` prevents immediate exit, calls `PlaybackService::shutdown()`, and then exits. Shutdown is idempotent: an atomic flag blocks incoming commands (`guard_shutdown!`); already-queued FIFO commands receive defined replies; the actor drains commands until `Shutdown`; state is captured and flushed with a 3 s bound; persistence worker flushes and acknowledges; worker and persistence threads are joined safely.
- **Periodic persistence:** while Playing, the actor saves at most once every 5 s. Pause, stop, track changes and application exit flush explicitly.
- **Position semantics & restore resilience:** `media_ms = wall_clock * speed_factor`, with seek-aware adaptation. When restored from SQLite, `restored_position_ms` preserves the saved position even if the audio file or external volume is missing or fails to decode. `snapshot()` and `persist_state()` prioritize `restored_position_ms` over zeroed backend position, preventing zero-overwrites when media is temporarily unavailable. Full queue metadata is restored even from unmounted volumes.
- **Device resilience:** startup without an output device is non-fatal; the app remains usable and reports `device_unavailable`. A manual retry re-opens the device. **Mid-stream disconnect detection and automatic recovery are not implemented.**
- **Seeking:** `player.try_seek(Duration)` uses Symphonia's container seek index; failures return an honest `SeekError` rather than decoding from the start.
- **Codecs:** FLAC, ALAC, MP3, WAV, AAC/M4A, Ogg Vorbis, AIFF. WMA/Opus are excluded from discovery rather than silently failing.
- **Tests:** real fixtures for FLAC, MP3, AAC/M4A, ALAC/M4A, AIFF and a 30 s FLAC; decode/seek and long-seek latency tests. All tests that require physical audio output hardware are explicitly marked `#[ignore = "requires a physical audio output device"]` and assert device readiness.

---

## 3. Queue, shuffle and state synchronization

- **Authoritative state:** Rust owns the queue, current track, position, repeat mode, shuffle order and volume/mute.
- **Atomic loading:** `set_queue` validates the candidate track through a transactional probe before mutating the player or committing the queue. Validation checks both the container header and inspects the first decodable audio sample payload (`decoder.next().is_none()`). Corrupt headers, truncated payloads, missing files, or unsupported codecs leave the previous queue, track selection, and playback state completely intact and return an explicit error.
- **Bounded skip:** automatic advance tries a bounded number of tracks; all-failed and repeat-one failure terminate without livelock.
- **Shuffle:** repeat-all wrap builds a fresh shuffle cycle; queue removal/reorder remaps the existing order instead of resetting history.
- **Generations:** monotonic `generation` values let React discard stale position events from previous tracks.
- **Persistence:** queue, track, position, repeat, shuffle, speed, volume and mute are stored in `playback_state` (migration `0002_playback_state_volume.sql`) and restored on startup **paused**, never auto-resuming.
- **SMTC:** Windows SMTC (`souvlaki`) refreshes its snapshot cache and emits updated state immediately upon initialization, preventing stale status reporting. Runtime errors from metadata and playback state updates are propagated and surfaced.

---

## 4. Capability honesty matrix

| Capability | Windows | macOS | Android | iOS |
|---|---|---|---|---|
| Local files (FLAC/ALAC/MP3/WAV/AAC/OGG/AIFF) | Supported, runtime-verified | Compiled only | Not integrated | Not integrated |
| Non-blocking demuxer seeking | Supported, verified | Compiled only | Not integrated | Not integrated |
| Mid-stream device disconnect recovery | **Unsupported** (manual retry only) | **Unsupported** | N/A | N/A |
| Startup without output device | Supported, verified | Same code, unverified | N/A | N/A |
| Progressive HTTPS / HLS / DASH | Standby | Standby | Not implemented | Not implemented |
| Background playback | Process open | Process open, unverified | Standby | Standby |
| System media controls | SMTC initialized; Play/Pause verified; Seek/Next/Previous mapped but not exercised | Standby | Standby | Standby |
| Gapless / crossfade | Standby | Standby | Standby | Standby |

**Scope note:** Windows was exercised with a real UI and OS media keys. macOS, Android and iOS were not run. Device-disconnect resilience is explicitly unsupported, not merely unverified.
