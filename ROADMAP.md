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
- [x] System Media Transport Controls (`souvlaki` SMTC on Windows, Now Playing on macOS)
- [x] Playback state persistence and session restoration in SQLite (`playback_state`)
- [x] PlayerDock, ExpandedPlayer, and QueueDrawer connected to real backend with honest UI states
- [x] Format/stream capability matrix documented in `docs/audio-backends.md`
- [x] Milestone 3 Playback Remediation: Native demuxer seeking, non-fatal device startup fallback, atomic queue loading, generation-tracked event streams, scrubber commit debounce, and drag-and-drop queue reordering

## Milestone 4 — Product features
- [ ] Search orchestration and provider adapters
- [ ] Playlist management and liked tracks
- [ ] LRCLIB lyrics and synced scrolling
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
