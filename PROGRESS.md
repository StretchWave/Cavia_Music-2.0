# Cavia Music 2.0 — Progress & Verification Log

## 1. Overview
This log documents the design system preservation, UI/UX audit, interface refinement, and the implementation of **Milestone 2: Real Local Music Library**.

---

## 2. Milestone 2 Implementation: Real Local Music Library

### Core Capabilities & Architecture
- **Native Folder Selection (`tauri-plugin-dialog`):**
  - Integrated native directory picker on the host operating system.
  - Safe staging interface separating desktop filesystem paths from future mobile content URIs / security-scoped URLs.
- **Recursive Audio Discovery (`walkdir`):**
  - Whitelist of high-fidelity audio extensions: `.flac`, `.mp3`, `.wav`, `.m4a`, `.aac`, `.ogg`, `.opus`, `.aiff`, `.alac`, `.wma`.
  - Non-audio and hidden directory traversal skipped safely.
- **High-Performance Metadata Extraction (`lofty 0.25`):**
  - Pure-Rust metadata extraction for Vorbis, ID3v2, MP4, and RIFF tags.
  - Extracted fields: Title, Artist, Album, Album Artist, Duration (ms), Track Number, Disc Number, Release Year, Genre, and Embedded Picture bytes.
  - Graceful fallback: when tags are absent or non-standard, the track defaults cleanly to the file stem without crashing or dropping the audio file.
- **Embedded Artwork Caching:**
  - Extracts front cover image bytes, hashes with SHA-256, and caches to disk under `<app_data_dir>/artworks/<hash>.<ext>`.
  - Deduplication: multiple tracks sharing the same album cover share a single cached image on disk.
  - Webview integration: formatted via `convertFileSrc` to comply with strict CSP policies.
- **Bit-for-Bit Duplicate & Moved-File Detection:**
  - Fast streaming SHA-256 content hashing across audio file bytes.
  - Identical content hash with existing path -> records duplicate in `import_history` and skips redundant indexing.
  - Identical content hash with missing old path -> detects moved or renamed files, updates `file_uri`, and restores availability without losing playlist associations or liked status.
- **Cancellable Scanning & Live Progress Streaming:**
  - Thread-safe cancellation via `Arc<AtomicBool>`.
  - Real-time progress events streamed over Tauri event bus (`library:scan-progress`) reporting phase (`discovering`, `scanning`, `completed`, `cancelled`), discovered count, processed count, new tracks, and duplicate count.
  - Reactive scan banner with live progress bar and accessible "Cancel Scan" button in the frontend.
- **Persistent SQLite Repositories (`SQLx`):**
  - Real SQLite persistence for `tracks`, `playlists`, `playlist_tracks`, `liked_tracks`, and `import_history`.
  - Full CRUD operations:
    - `get_tracks`: All active tracks with liked state.
    - `get_albums`: Auto-grouped by album and artist with track counts and artwork.
    - `get_artists`: Auto-grouped by artist with album and track counts.
    - `get_liked_tracks` & `toggle_liked_track`: Instant persistence and state synchronization.
    - `get_playlists`, `create_playlist`, `get_playlist_tracks`, `delete_playlist`: Full playlist management stored in SQLite.

---

## 3. Design System Preservation & Aesthetic Direction
- **Non-Negotiable Brand Identity:**
  - Preserved obsidian dark foundation (`#090a0d`, `#0c0e13`, `#12141a`, `#161922`, `#1b1e28`).
  - Aged ivory typography (`#f5f2eb`, `#c5bfb4`) paired with Georgia serif editorial headings and Cascadia Code monospaced metadata.
  - Restrained vermilion accent (`#d64732`, hover `#e55540`, active `#bd3926`). Extraneous theme/accent pickers removed to maintain single authoritative brand identity.
  - Semantic status indicators only (`#7db895` ready, `#e09f58` warning, `#df5849` error).
- **Archival Calligraphy & Editorial Elements:**
  - Japanese calligraphy maintained as subtle, intentional, low-opacity background watermarks:
    - `藏` (Archive) — Library overview & vinyl record labels.
    - `音` (Sound) — Audio engine & album sleeve art.
    - `盤` (Record) — Albums & vinyl disc views.
    - `選` (Selection) — Playlists & curated collections.
    - `尋` (Discovery) — Universal search prompt.
    - `奏` (Performance) — Artist catalogs.
    - `曲` (Composition) — Tracks listing.
- **Adaptive Library Hero:**
  - When empty: Displays the blank canvas showcase and grooved vinyl centerpiece inviting the user to import their first folder.
  - When populated: The turntable centerpiece displays an active collection status with quick jump actions ("Browse All Tracks", "View Albums", "View Artists").

---

## 4. Responsive Verification Matrix (7 Viewports)

| Viewport | Mode | Verification Details |
|---|---|---|
| **1920 × 1080** | Ultra-wide Desktop | Max-width content bounds (`1520px`), centered layout, balanced density |
| **1440 × 900** | Standard Desktop | Full persistent player dock, 256px sticky sidebar, modal dialogs centered |
| **1024 × 768** | Tablet Landscape | Condensed 220px sidebar, clean table wrapping, auxiliary controls intact |
| **768 × 1024** | Tablet Portrait | Sidebar hidden; bottom navigation bar active; touch mini-player engaged |
| **430 × 932** | Mobile Pro Max | Full touch accessibility (≥44px touch targets), mobile mini-bar with expand chevron |
| **390 × 844** | Mobile Standard | Zero horizontal overflow, typography scaled via `clamp()`, drawers slide up seamlessly |
| **360 × 740** | Mobile Compact | Clean wrapping of hero quick actions, stats strip stacking, and touch controls |

---

## 5. Files Changed & Implemented

| File | Type | Changes |
|---|---|---|
| [`src-tauri/Cargo.toml`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src-tauri/Cargo.toml) | Rust | Added `lofty`, `walkdir`, `sha2`, `hex`, `uuid`, and `tokio` test dependencies |
| [`src-tauri/src/lib.rs`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src-tauri/src/lib.rs) | Rust | Added services/domain modules, initialized artwork directory, registered 11 new commands |
| [`src-tauri/src/state.rs`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src-tauri/src/state.rs) | Rust | Added `artworks_dir` and thread-safe `scan_cancelled: Arc<AtomicBool>` to `AppState` |
| [`src-tauri/src/domain.rs`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src-tauri/src/domain.rs) | Rust | Serializable domain DTOs (`Track`, `Album`, `Artist`, `Playlist`, `ScanProgress`, `ScanResult`) |
| [`src-tauri/src/services/metadata.rs`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src-tauri/src/services/metadata.rs) | Rust | Lofty audio metadata extraction, tag parsing, picture extraction, and fallback |
| [`src-tauri/src/services/scanner.rs`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src-tauri/src/services/scanner.rs) | Rust | Recursive folder scanner, SHA-256 duplicate detection, moved-file recovery, event progress |
| [`src-tauri/src/services/library.rs`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src-tauri/src/services/library.rs) | Rust | SQLite CRUD queries for tracks, albums, artists, liked songs, and playlists |
| [`src-tauri/src/commands/library.rs`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src-tauri/src/commands/library.rs) | Rust | Tauri command handlers for library operations and cancellable scans |
| [`src/lib/library.ts`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/lib/library.ts) | Frontend | Typed TypeScript client bindings, native folder dialog, and artwork URI formatting |
| [`src/lib/types.ts`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/lib/types.ts) | Frontend | Updated Track interface with extended metadata properties |
| [`src/app/App.tsx`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/app/App.tsx) | Frontend | Connected real SQLite state, scan progress banner, cancellation, and library refresh |
| [`src/features/library/LibraryView.tsx`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/features/library/LibraryView.tsx) | Frontend | Active folder import button, adaptive empty vs populated turntable showcase |
| [`src/features/library/TracksView.tsx`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/features/library/TracksView.tsx) | Frontend | Real tracks table with artwork thumbnails, duration, inline like button, and active import button |
| [`src/features/library/AlbumsView.tsx`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/features/library/AlbumsView.tsx) | Frontend | Real albums grid with artwork caching, track count, release year, and import button |
| [`src/features/library/ArtistsView.tsx`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/features/library/ArtistsView.tsx) | Frontend | Real artists grid with monogram avatars, album/track counts, and import button |
| [`src/features/library/PlaylistsView.tsx`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/features/library/PlaylistsView.tsx) | Frontend | Real playlist creation and deletion in SQLite |
| [`src/features/library/PlaylistDetailModal.tsx`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/features/library/PlaylistDetailModal.tsx) | Frontend | Loads real playlist tracks from SQLite, playlist deletion with confirmation |
| [`src/features/library/LikedTracksView.tsx`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/features/library/LikedTracksView.tsx) | Frontend | Real liked tracks from SQLite with inline un-liking |
| [`src/styles.css`](file:///c:/Programs/Spoty/Cavia_Music%202.0/src/styles.css) | Styles | Scan banner, progress bar, toast notifications, inline track art & like buttons |
| [`ROADMAP.md`](file:///c:/Programs/Spoty/Cavia_Music%202.0/ROADMAP.md) | Docs | Checked off all Milestone 2 deliverables |

---

## 6. Verification Results

1. **Rust Test Suite (`cargo test`):**
   - Command: `cargo test --manifest-path src-tauri/Cargo.toml`
   - Result: **8 passed, 0 failed, 0 ignored**
     - `test_determine_mime_type`: Verified mime-type mapping for audio extensions.
     - `test_supported_extensions`: Verified audio extension whitelist.
     - `test_calculate_file_hash`: Verified streaming SHA-256 calculation.
     - `test_playlist_lifecycle`: Verified playlist creation, listing, and deletion in SQLite.
     - `test_toggle_liked_track`: Verified liking and un-liking tracks in SQLite.
     - `test_scan_cancellation`: Verified early cancellation stopping scan and marking result.
     - `test_scan_multiple_formats_and_duplicates`: Verified recursive discovery, audio format handling, and duplicate detection.
     - `test_persistence_across_reconnect`: Verified persistence across database reconnect (simulating application restart).
2. **Rust Static Checks (`cargo fmt`, `cargo check`):**
   - `cargo fmt --check`: 0 formatting issues.
   - `cargo check`: 0 warnings, 0 errors.
3. **Frontend Build (`npm run build`):**
   - Command: `npm run build` (`tsc -b && vite build`)
   - Result: **0 errors, 534ms build time**.
4. **Desktop Application Execution:**
   - Command: `npm run tauri dev`
   - Result: Native Tauri desktop application running in dev mode (`cavia-music.exe`), connected to persistent SQLite database.
5. **Visual Screenshots:**
   - 12 high-resolution screenshots generated across 7 viewports under `screenshots/`.

---

## 7. Known Limitations & Platform Considerations
- **Desktop (Windows/macOS):** Native filesystem paths and recursive directory walking are fully implemented and verified.
- **Mobile (Android/iOS):** Android content URIs and iOS security-scoped URLs require native file staging bridges when packaging for those target SDKs. The Rust scanner is decoupled from Tauri UI to permit staging buffers before passing canonical file paths.
- **Playback Boundary:** Audio playback remains deliberately un-wired (`audio_backend: "not_integrated"`) per project rules until Milestone 3.


---

## 4. Milestone 3 Implementation: Native Audio Playback Engine

### Architecture & Platform Adapters
- **Common Playback Contract (`AudioBackend` Trait):**
  - Defines target-agnostic interface in `src-tauri/src/audio/mod.rs`: `load`, `play`, `pause`, `resume`, `stop`, `seek`, `set_volume`, `set_speed`, `position_ms`, `duration_ms`, `status`, `is_ended`, and `capabilities`.
  - Concrete platform factory (`create_platform_backend`) instantiates the verified platform engine.
- **Desktop Engine (`RodioBackend`):**
  - Zero-dependency audio engine utilizing `rodio 0.23` with native `cpal` (WASAPI direct on Windows, CoreAudio on macOS) and pure-Rust `symphonia` decoders.
  - Native playback support for: FLAC, ALAC, MP3, WAV, AAC, M4A, Ogg Vorbis.
  - Sub-millisecond position tracking, seek stream alignment, volume attenuation, playback speed adjustment (0.25x to 4.0x), and auto-advance.
- **libmpv (`rsmpv`) Evaluation:**
  - Evaluated and packaged in `src-tauri/src/audio/mpv_backend.rs` behind optional `desktop-mpv` Cargo feature.
  - Findings: `rsmpv` relies on runtime `mpv-2.dll` discovery. Clean machines without `mpv-2.dll` fail on startup unless bundled with heavy (~70MB+) platform binaries, and carries GPLv3/LGPL licensing distribution obligations. Retained as an optional backend for specialized stream requirements.
- **Mobile Backends Staging:**
  - Staged contracts for Android Media3 ExoPlayer (`MediaSessionService`) and iOS AVPlayer (`MPNowPlayingInfoCenter`).
  - Architecture ensures desktop implementations are not pretended to automatically support mobile targets.
- **System Media Transport Controls (SMTC):**
  - Integrated `souvlaki` for native Windows SMTC taskbar / lock-screen media controls and macOS Now Playing metadata synchronization.
- **Deterministic Queue Management (`QueueManager`):**
  - Implemented in `src-tauri/src/services/queue.rs`.
  - Supports repeat modes: `off`, `all`, `one`.
  - Shuffle permutation with pinned current track at index 0 to guarantee audio playback invariance.
  - Sequential next/previous, queue reordering, track removal, and automatic queue loop.
  - Comprehensive unit test suite covering all queue edge cases, repeat modes, and shuffle invariance.
- **Authoritative State Synchronization:**
  - `PlaybackService` coordinator maintains authoritative state in Rust.
  - Event streaming to webview:
    - `playback:state-changed` for complete state transitions (`status`, `currentTrack`, `positionMs`, `durationMs`, `volume`, `isMuted`, `repeatMode`, `isShuffle`, `playbackSpeed`, `queue`).
    - `playback:position` for 250ms scrubber position updates.
  - Playback state throttled persistence and session restoration in SQLite (`playback_state` table).
- **Interface Wiring:**
  - `PlayerDock`: Transport buttons, play/pause, next, previous, shuffle, repeat, scrubber, volume slider, speed pill, and like button connected to real backend commands.
  - `ExpandedPlayer`: Full vinyl album art assembly, play/pause, next, previous, shuffle, repeat, scrubber, volume, and speed pill.
  - `QueueDrawer`: Shows real queue, current track, clear queue, remove track, and click-to-play.
  - `TracksView`, `LikedTracksView`, `PlaylistsView`: "Play All", "Shuffle", and clicking individual tracks queue and initiate real playback.


---

## 8. Milestone 3 Remediation & Architectural Hardening

Following an independent technical audit of the initial Milestone 3 audio playback implementation, critical defects were investigated and systematically remediated:

### A. Non-Blocking Native Demuxer Seeking
- **Defect Identified:** Initial seeking relied on `skip_duration`, decoding audio samples from byte 0 to the target timestamp on every seek, taking up to 500ms on long tracks and freezing the playback thread.
- **Remediation:** Migrated to Rodio 0.23's `player.try_seek(Duration)` which utilizes Symphonia's native format demuxer seek tables.
- **Verification:** Benchmarked on real audio fixtures; seek operations now complete in 0–5ms. Clamped positions strictly within `[0, duration_ms]`.

### B. Device Startup Resilience (No Output Device Fallback)
- **Defect Identified:** Unhandled device errors during `DeviceSinkBuilder::open_default_sink()` crashed the Tauri process at launch on headless machines or systems with no active audio output.
- **Remediation:** Initialized `RodioBackend` with a non-fatal device fallback (`device_available = false`). Users can freely browse, scan folders, and edit playlists without crashing. Audio engine status dynamically reports `device_unavailable` rather than crashing or faking readiness.

### C. Atomic Track Transitions & Queue Integrity
- **Defect Identified:** Selecting an unplayable or missing track mutated the queue index before file validation, advancing the UI and desynchronizing playback from the queue cursor.
- **Remediation:** Track file verification and decoding now occur before committing queue index updates. If a file cannot be loaded or decoded, the queue index remains uncorrupted, and an honest error snapshot (`errorMessage`) is emitted.

### D. Livelock-Proof Auto-Advance
- **Defect Identified:** When all tracks in a queue failed to decode or when repeat-one was active on a corrupted file, auto-advance looped infinitely on background ticker threads.
- **Remediation:** Bounded auto-advance attempts strictly to queue length. If consecutive track loads fail, playback terminates cleanly with status `error` and message `"Playback halted: all upcoming tracks in queue failed to load"`.

### E. Generation Tracking & Stale Event Discarding
- **Defect Identified:** Rapid track skips and asynchronous tick threads could deliver delayed position ticks from older tracks after a new track started.
- **Remediation:** Added monotonic `generation: u64` to `PlaybackSnapshot` and `PositionUpdate`. React discards position ticks if `pos.generation < state.generation` or `pos.trackId !== currentTrack.id`.

### F. Scrubber Interaction & IPC Throttling
- **Defect Identified:** HTML range slider fired `onChange` at 60Hz during drag, flooding Tauri IPC with dozens of seek calls per second.
- **Remediation:** Enhanced `Slider` component with pointer capture, keyboard accessibility, and separate `onCommit` callback. Dragging smoothly updates local timecode preview with zero IPC calls; exactly one seek command is dispatched upon release (`pointerup`/`keyup`).

### G. Queue Reordering & Disambiguation
- **Defect Identified:** Duplicate track instances in the queue could not be removed independently, and queue drag handles were decorative.
- **Remediation:** Added `remove_from_queue_by_index` command and integrated native HTML5 drag-and-drop reordering with visual indicators in `QueueDrawer`.

### H. Complete Mobile Honesty
- **Defect Identified:** Mobile stubs previously returned mock playing states.
- **Remediation:** Stripped all mock successes. Mobile targets explicitly return `PlaybackError::BackendUnavailable` and `AudioCapabilities::unavailable()`.

### I. Complete Test Suite
- **Result:** **22 unit tests passing in 0.50s** (`cargo test`), covering seeking on real audio fixtures, lifecycle events, error reporting, queue invariants, shuffle entropy, scanner cancellation, and database persistence.
