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
- [ ] Initialize Android and iOS projects
- [x] Verify Windows startup and persistent SQLite path
- [ ] Add CI for frontend and Rust checks

## Milestone 2 — Real local library
- [ ] Native file picker on each target
- [ ] Android content URI staging / iOS security-scope file staging
- [ ] Metadata extraction and embedded artwork
- [ ] Import deduplication and scan progress
- [ ] Library CRUD, filtering, sorting and persistence
- [ ] Missing/moved-file detection

## Milestone 3 — Playback
- [ ] Finalize libmpv packaging and license choice for Windows/macOS
- [ ] Build Android Media3 adapter
- [ ] Build iOS AVPlayer adapter
- [ ] Common commands and playback event model
- [ ] Queue, seeking, volume, speed, repeat, shuffle and completion
- [ ] Media sessions, lock-screen and background playback
- [ ] Format/stream capability matrix by platform

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
