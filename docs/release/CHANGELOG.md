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
- **Accessibility & Interaction Suite**: Keyboard navigation, ARIA announcements, modal focus handling, and responsive controls. A current WCAG conformance audit is not claimed.
- **Optional Account & Cloud Synchronization**: Opt-in multi-device sync for playlists and liked tracks via Supabase Auth & PostgreSQL with Row-Level Security and OS Credential Manager protection.
- **Standalone Windows Installer**: Self-contained NSIS installer (`currentUser` install, no administrator rights required) with clean uninstaller and upgrade data preservation.
- **CI/CD Build Automation**: GitHub Actions workflows for continuous integration (`ci.yml`) and multi-platform packaging (`release.yml`) for Windows, macOS, and Android.

### Changed
- **Performance Optimization**: Playback position updates are separated from root component state. No comparable before/after render benchmark is currently recorded.
- **Database Indexing**: Scanner queries use composite indexes. No reproducible 50,000-track timing benchmark is currently recorded.
- **Binary Footprint**: Release compiler settings include size-oriented optimizations. The historical executable-size comparison has not been reproduced for the current build.
- **Editorial Obsidian Aesthetic**: Standardized container constraints, grooved vinyl centerpiece animation, and typography hierarchy across all 15 views.

### Fixed
- **Shutdown Resilience**: Eliminated lost-reply worker races and watch-channel lost wakeups during two-phase application exit.
- **Upgrade Data Reconciliation**: Implemented automatic migration checksum reconciliation in SQLite startup, preventing startup panics and guaranteeing 100% data preservation across schema updates.
- **Artwork Cache Cleanup**: Fail-closed transaction locks preventing concurrent race conditions between background folder scanning and cache orphan cleanup.

### Security
- **Strict Privacy**: Zero telemetry, zero analytics SDKs, and zero behavioral tracking.
- **Local-Only Audio**: User audio files and filesystem directory paths never leave the local machine.
- **Credential Protection**: Windows refresh tokens use Windows Credential Manager. Android uses Android Keystore-backed AES-GCM storage (emulator round-trip verified). macOS runtime storage is unverified; iOS Keychain is not integrated. Credential failures do not fall back to plaintext storage.
