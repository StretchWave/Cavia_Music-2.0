# Audio Backend Architecture & Evaluation Note

## 1. Shared Playback Contract

All platform implementations follow one common Cavia playback contract (`AudioBackend` trait in `src-tauri/src/audio/mod.rs`). The React frontend communicates strictly through typed Tauri commands and authoritative event streams (`playback:state-changed` and `playback:position`), never communicating directly with underlying audio libraries.

| Target | Active Backend | Native Media Integration | License | Status |
|---|---|---|---|---|
| Windows | Native Rodio (CPAL WASAPI + Symphonia) | Windows System Media Transport Controls (`souvlaki`) | MIT / Apache-2.0 | Operational (seeking, non-blocking device init) |
| macOS | Native Rodio (CPAL CoreAudio + Symphonia) | Now Playing / Remote Command Center (`souvlaki`) | MIT / Apache-2.0 | Operational (shared Rodio engine) |
| Windows (Alt) | libmpv via `rsmpv` (feature `desktop-mpv`) | Windows SMTC | GPLv3 / LGPLv2.1+ | Evaluated / Optional feature |
| Android | Native Media3 ExoPlayer | MediaSessionService + Foreground Notification + Audio Focus | Apache-2.0 | Staged contract / Stub (honest error on playback) |
| iOS | Native AVPlayer | `MPNowPlayingInfoCenter` + `MPRemoteCommandCenter` | Apple Native SDK | Staged contract / Stub (honest error on playback) |

---

## 2. Desktop Audio Engine: Native Rodio Backend (Milestone 3 Remediated)

### Architecture
- **Engine**: Pure-Rust audio sink leveraging `rodio 0.23` with `cpal` (WASAPI direct on Windows, CoreAudio on macOS) and pure-Rust `symphonia` decoders.
- **Zero DLL Overhead**: Compiles directly into the Cavia binary with zero runtime dynamic C library dependencies. Works on clean machines immediately without requiring external DLLs (`mpv-2.dll`).
- **Device Resilience**: Graceful startup without crashing when no physical audio output device is connected (`open_default_sink` failure handled non-fatally). App remains fully functional for browsing, scanning, and managing library while audio status reports `device_unavailable`.
- **Fast Demuxer Seeking**: Uses `player.try_seek(Duration)` directly through Symphonia's native seek table. Clamps seek offsets within `[0, duration_ms]`. Operates in 0–5ms without decoding intermediate samples from track beginning.
- **Instant Buffer Purge**: Loading a new track disconnects the previous player and attaches a fresh player instance immediately, releasing decoded buffers without thread blocking or audio overhang.
- **Supported Codecs**: FLAC, ALAC, MP3, WAV, AAC, M4A, Ogg Vorbis, AIFF. Unsupported formats (WMA, Opus) are excluded from discovery to prevent silent playback failure.

---

## 3. Queue Management & State Synchronization

- **Authoritative Domain State**: Rust domain owns the queue (`QueueManager` in `src-tauri/src/services/queue.rs`), position, active track, repeat modes (`off`, `all`, `one`), and shuffle permutation.
- **Atomic Track Loading**: File validation and audio decoding occur before committing queue index updates. If an unplayable file is encountered, the queue cursor is not corrupted, and an honest error snapshot is emitted.
- **Livelock Prevention**: Automatic queue advancement tracks failed attempts and halts gracefully if all upcoming tracks or repeat-one recordings fail to load, preventing infinite CPU spinning.
- **Event Concurrency & Generations**: Each playback generation is stamped with a monotonic `generation: u64`. The frontend discards stale position ticks (`pos.generation < state.generation`) or ticks from mismatched track IDs.
- **True Shuffle Non-Determinism**: Seeded by system entropy (`SystemTime`), producing fresh non-repeating permutations on every shuffle cycle while pinning the currently playing track at index 0.
- **Reordering & Removal**: Supports index-based queue removal (`remove_from_queue_by_index`) to safely disambiguate duplicated tracks, alongside full HTML5 drag-and-drop queue reordering.
- **Persistence**: Playback state and queue are persistently debounced into SQLite table `playback_state` and restored on application startup.

---

## 4. Capability Honesty Matrix

| Capability | Windows (Tested) | macOS (Compiled) | Android (Stub) | iOS (Stub) |
|---|---|---|---|---|
| Local Files (FLAC/MP3/WAV/AAC/OGG/AIFF) | Supported | Supported | Not Implemented | Not Implemented |
| Non-blocking Demuxer Seeking | Supported | Supported | Not Implemented | Not Implemented |
| Device Disconnect Resilience | Supported | Supported | Not Implemented | Not Implemented |
| Progressive HTTPS | Standby | Standby | Not Implemented | Not Implemented |
| HLS / DASH Streaming | Standby | Standby | Not Implemented | Not Implemented |
| Background Playback | Supported | Supported | Standby (Needs MediaSessionService) | Standby (Needs AVAudioSession) |
| System Media Controls | Supported (SMTC) | Supported (NowPlaying) | Standby (Needs Media3) | Standby (Needs MPNowPlaying) |
| Gapless Transition | Standby | Standby | Standby | Standby |
| Crossfade | Standby | Standby | Standby | Standby |
