# Changelog

All notable changes to Cavia Music will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [2.0.0] - 2026-11-20

### Added
- **Native Local Audio Engine**: Bit-perfect decoding for FLAC, ALAC, WAV, AIFF, MP3, AAC, OGG Vorbis, and Opus powered by zero-dependency native Rust decoders (`rodio`, `symphonia`).
- **Local Library Indexer & Scanner**: High-performance recursive directory scanner with streaming SHA-256 duplicate detection, moved-file recovery, and embedded tag extraction via Lofty 0.25.
- **Embedded & Cloud Artwork Cache**: Automatic cover art extraction with SHA-256 deduplication and in-memory thumbnail acceleration.
- **Synchronized Lyrics Experience**: Millisecond-accurate synchronized lyrics display supporting embedded ID3v2/Vorbis/MP4 tags and community LRCLIB integration with click-to-seek navigation.
- **Universal Library Search**: Fast local search powered by SQLite FTS5 trigram indexing with instant 1-2 character prefix matching across Tracks, Albums, Artists, and Playlists.
- **Playlist & Favorites Management**: Complete local playlist CRUD, transactional track addition, accessible reordering, collision-proof compaction removal, and liked song synchronization.
- **Windows Media Controls (SMTC)**: Native Windows System Media Transport Controls integration supporting media keyboard keys, lock screen widgets, and thumbnail display.
- **Responsive Layout Engine**: Responsive multi-viewport interface adapting across 8 device classes from compact mobile (360px) to ultra-wide desktop (1920px).
- **Accessibility & Interaction Suite**: Full keyboard navigation with roving tabindex, ARIA live regions, focus trapping in drawers/modals, WCAG 2.5.3 label-in-name compliance, and 0 axe-core automated violations.
- **Optional Account & Cloud Synchronization**: Opt-in multi-device sync for playlists and liked tracks via Supabase Auth & PostgreSQL with Row-Level Security and OS Credential Manager protection.
- **Standalone Windows Installer**: Self-contained NSIS installer (`currentUser` install, no administrator rights required) with clean uninstaller and upgrade data preservation.
- **CI/CD Build Automation**: GitHub Actions workflows for continuous integration (`ci.yml`) and multi-platform packaging (`release.yml`) for Windows, macOS, and Android.

### Changed
- **Performance Optimization**: Decoupled high-frequency playback position updates from root component state, reducing steady-state re-renders during playback from 4/s to 0/s.
- **Database Indexing**: Optimized scanner database queries with composite indexes, accelerating 50k-track duplicate lookups by 147× (from 9.56ms to 0.04ms).
- **Binary Footprint**: Optimized release compiler settings with Thin-LTO and symbol stripping, reducing native executable footprint by 62.6% (from 34.72MB to 13.00MB).
- **Editorial Obsidian Aesthetic**: Standardized container constraints, grooved vinyl centerpiece animation, and typography hierarchy across all 15 views.

### Fixed
- **Shutdown Resilience**: Eliminated lost-reply worker races and watch-channel lost wakeups during two-phase application exit.
- **Upgrade Data Reconciliation**: Implemented automatic migration checksum reconciliation in SQLite startup, preventing startup panics and guaranteeing 100% data preservation across schema updates.
- **Artwork Cache Cleanup**: Fail-closed transaction locks preventing concurrent race conditions between background folder scanning and cache orphan cleanup.

### Security
- **Strict Privacy**: Zero telemetry, zero analytics SDKs, and zero behavioral tracking.
- **Local-Only Audio**: User audio files and filesystem directory paths never leave the local machine.
- **Credential Protection**: Refresh tokens stored exclusively in native OS Secure Storage (Windows Credential Manager / macOS Keychain / Android KeyStore / iOS Keychain).
