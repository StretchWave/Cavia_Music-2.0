# Cavia Music 2.0 — Build order

## Foundation (this package)
- [x] New Tauri/React/Rust project structure
- [x] Obsidian / vermilion UI shell and responsive navigation
- [x] Rust-owned SQLite connection and migration runner
- [x] App health and library summary commands
- [x] Explicit audio capability boundary and honest unconfigured state
- [x] Desktop/mobile architecture decisions

## Milestone 1 — First runnable native build
- [x] Install dependencies and generate lockfiles
- [x] Verify Vite production build
- [x] Verify Tauri Windows dev build
- [x] Initialize Android target (Tauri Android initialized)
- [x] Verify Windows startup and persistent SQLite path
- [ ] Add CI for frontend and Rust checks

## Milestone 2 — Real local library
- [x] Native file picker on each target (`tauri-plugin-dialog`)
- [x] Android content URI staging / iOS security-scope file staging (desktop filesystem paths, staging interface)
- [x] Metadata extraction and embedded artwork (Lofty metadata parser and SHA-256 artwork cache)
- [x] Import deduplication and scan progress (`library:scan-progress` event stream, SHA-256 content hashes)
- [x] Library CRUD, filtering, sorting and persistence (Tracks, Albums, Artists, Playlists, Liked Songs in SQLite)
- [x] Missing/moved-file detection (automatic path update on hash match when previous file is missing)

## Milestone 3 — Playback
- [x] Evaluated libmpv (rsmpv) and zero-dependency native Rodio/CPAL/Symphonia engine
- [x] Common playback contract (`AudioBackend` trait) and platform factory
- [x] Staged Android Media3 ExoPlayer and iOS AVPlayer contract adapters
- [x] Common commands and playback event model (`playback:state-changed`, `playback:position`)
- [x] Authoritative queue manager with deterministic shuffle, repeat (off/all/one), and auto-advance
- [x] Windows SMTC metadata and event forwarding (OS Play/Pause verified at runtime; Next/Previous/Seek mapped but not exercised; macOS Now Playing deferred to physical hardware verification)
- [x] Playback state persistence and session restoration in SQLite (`playback_state`)
- [x] PlayerDock, ExpandedPlayer, and QueueDrawer connected to real backend with honest UI states
- [x] Format/stream capability matrix documented in `docs/audio-backends.md`
- [x] Milestone 3 Playback Remediation: Dedicated audio actor control thread, native seeking without fallback, speed-adjusted position model across 0.5x–2.0x, atomic queue transitions, bounded skip policy, debounced volume slider, and ComCtl32 v6 test manifest.
- [x] Third audit remediation: single Windows manifest for app and tests, owned persistence runtime, Tauri exit shutdown with durable flush, 5 s position persistence, atomic `set_queue` decoder probe, desktop/mobile health distinction, SMTC attach/status/progress, shuffle regression coverage, and 36 unit + 1 integration tests.
- [x] Final correctness remediation: durable persistence write error propagation, restore position preservation across unmounted drives and failed loads, idempotent FIFO shutdown handshake without lost-reply races, immediate SMTC cache refresh and error propagation, atomic `set_queue` with payload decodability validation, UI control disabling and command failure toast surfacing, dynamic settings information, and explicit physical audio hardware test annotations (43 passed, 5 ignored for physical output device, 0 failed).
- [x] Final remediation audit: persistent pending payload tracking with flush/shutdown retry, two-phase shutdown error propagation with `RunEvent::ExitRequested` prevention and retry/force-exit UI recovery, typed `SmtcStatus` health and runtime error isolation via `MediaControlsHost` abstraction, transactional track replacement preserving active audio sessions across loading failures, and full sequential verification suite (72 passed, 5 ignored for physical output device, 0 failed).
- [x] Final audit corrections: typed shutdown error event contract, watch-channel lost-wakeup race elimination, paused candidate player initialization preventing audio overlap, persistence health exposure in AppHealth/SettingsView, and SMTC diagnostics display with Initializing audit (86 passed, 5 hardware tests passed, 0 failed, 10 node contract tests passed).
- [ ] Mid-stream audio-device disconnect detection and automatic recovery (manual retry exists; currently unsupported).

## Milestone 4 — Product features
- [x] Search orchestration and provider adapters (Milestone 4.2: local search implementation completed with SQLite FTS5 trigram indexing and 1-2 char short-query fallback; 4 supported categories [Tracks, Albums, Artists, Playlists]; 103 Rust + 29 Node.js contract tests passed; Windows runtime verified; mobile runtime remains unverified; external provider adapters deferred to M4.3)
- [x] Playlist management and liked tracks (Milestone 4.1: complete local playlist management, transactional addition with duplicate support, collision-proof compaction removal, accessible reordering, metadata editing, and instant count synchronization)
- [x] LRCLIB lyrics and synced scrolling (Milestone 4.3: embedded lyrics priority [ID3v2 USLT/SYLT (millisecond format only; MPEG-frame SYLT ignored), Vorbis Comments, MP4], LRCLIB lookup with response identity validation and valid-range duration only, enforced Retry-After cooldowns on a serialized request gate, SQLite lyrics cache with size/mtime-hardened fingerprint and 7-day bounded negative caching, shared embedded-content classification, robust LRC parser, prop-driven synchronized highlighting with manual-scroll follow control and duration-clamped click-to-seek; 166 Rust + 1 smoke + 29 Node.js tests passing; Windows runtime exercised through the live UI; live 429/offline conditions and mobile runtime remain unverified)
- [ ] Artwork cache and audio cache where permitted
- [ ] Spotify PKCE playlist metadata import
- [ ] Optional account/cloud sync

## Milestone 5 — Design and release quality
- [ ] Final design pass across all screens
- [ ] UI screenshot review at mobile/tablet/desktop sizes
- [ ] Keyboard, mouse, touch, accessibility and reduced-motion audits
- [ ] Performance/memory/startup benchmarks
- [ ] Clean-machine installer and mobile release builds
- [ ] License notices, privacy policy and release checklist
