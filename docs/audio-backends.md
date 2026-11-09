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
- **Audio actor:** all synchronous decoding, file I/O and backend calls run on the dedicated `cavia-audio-control` thread. Tauri commands send `oneshot` requests over an `mpsc` channel; the actor owns `PlaybackCore`, the queue and the backend.
- **Persistence owner:** a dedicated `cavia-persist` std thread owns a current-thread Tokio runtime. It debounces serialized writes to SQLite. It is not started with `tokio::spawn` from synchronous Tauri setup.
- **Shutdown:** `RunEvent::ExitRequested` prevents immediate exit, calls `PlaybackService::shutdown()`, and then exits. New commands are rejected with `"Playback service is shutting down"`; already-queued FIFO commands receive defined replies; the actor stops; state is persisted **before** `backend.stop()` (which zeroes Rodio's position), flushed with a 3 s bound, and worker/persistence threads are joined.
- **Periodic persistence:** while Playing, the actor saves at most once every 5 s. Pause, stop, track changes and application exit flush explicitly.
- **Position semantics:** `media_ms = wall_clock * speed_factor`, with seek-aware adaptation. When a track is loaded at an offset and paused immediately (startup restore), Rodio's audio callback may not yet have updated `get_pos()`; the paused position therefore falls back to the requested seek offset so shutdown cannot erase the resume point.
- **Device resilience:** startup without an output device is non-fatal; the app remains usable and reports `device_unavailable`. A manual retry re-opens the device. **Mid-stream disconnect detection and automatic recovery are not implemented.**
- **Seeking:** `player.try_seek(Duration)` uses Symphonia's container seek index; failures return an honest `SeekError` rather than decoding from the start.
- **Codecs:** FLAC, ALAC, MP3, WAV, AAC/M4A, Ogg Vorbis, AIFF. WMA/Opus are excluded from discovery rather than silently failing.
- **Tests:** real fixtures for FLAC, MP3, AAC/M4A, ALAC/M4A, AIFF and a 30 s FLAC; decode/seek and long-seek latency tests. Tests that require physical output state that requirement with `eprintln!`; the paused-offset regression test is marked `#[ignore]` and was run explicitly on hardware.

---

## 3. Queue, shuffle and state synchronization

- **Authoritative state:** Rust owns the queue, current track, position, repeat mode, shuffle order and volume/mute.
- **Atomic loading:** `set_queue` validates the selected track through a real decoder probe before replacing the queue. Corrupt-but-existing files leave the previous queue and selection intact and return the error.
- **Bounded skip:** automatic advance tries a bounded number of tracks; all-failed and repeat-one failure terminate without livelock.
- **Shuffle:** repeat-all wrap builds a fresh shuffle cycle; queue removal/reorder remaps the existing order instead of resetting history.
- **Generations:** monotonic `generation` values let React discard stale position events from previous tracks.
- **Persistence:** queue, track, position, repeat, shuffle, speed, volume and mute are stored in `playback_state` (migration `0002_playback_state_volume.sql`) and restored on startup **paused**, never auto-resuming.

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
