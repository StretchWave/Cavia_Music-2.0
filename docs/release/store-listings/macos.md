# Cavia Music — macOS Distribution Metadata

**Application Name**: Cavia Music  
**Bundle Identifier**: `com.stretchwave.cavia`  
**Current Version**: `2.0.0`  
**Target Architectures**: Universal (`x86_64` Intel + `arm64` Apple Silicon)  
**Package Format**: Apple Disk Image (`.dmg`), App Bundle (`.app`)  
**Developer Account**: [APPLE_DEVELOPER_TEAM_ID_PLACEHOLDER]  
**Signing Identity**: [DEVELOPER_ID_APPLICATION_PLACEHOLDER]  
**Support Email**: [SUPPORT_CONTACT_EMAIL_PLACEHOLDER]  
**Privacy Policy URL**: [PRIVACY_POLICY_URL_PLACEHOLDER]  

---

## 1. Store Product Descriptions

### Short Description (100 characters max)
Editorial local-first music player with obsidian aesthetics and native performance.

### Full Description
Cavia Music is a precision, editorial local-first music player engineered with Tauri 2, React, and Rust. Designed for listeners who curate their own collections, Cavia delivers gapless audio playback, instant local indexing, and synchronized lyrics with zero telemetry.

**Key Features**:
- **Native Audio Engine**: High-fidelity decoding for FLAC, MP3, WAV, ALAC, AAC, AIFF, OGG, and Opus.
- **Local-First Architecture**: Your music files, playlists, and listening history remain strictly on your Mac.
- **Synchronized Lyrics**: Rich LRC scrolling lyrics with duration clamping and click-to-seek following.
- **Archival Design**: Tailored obsidian dark canvas with editorial serif typography and subtle Japanese calligraphy watermarks.
- **Apple Ecosystem Harmony**: Designed to adapt gracefully to macOS light/dark preferences and trackpad gestures.
- **Optional Multi-Device Sync**: Optional Supabase-backed playlist synchronization without audio uploads.

---

## 2. macOS Technical Specifications

- **Minimum System Version**: macOS 10.15 (Catalina) or later.
- **Recommended**: macOS 14 (Sonoma) or macOS 15 (Sequoia) on Apple Silicon (M1/M2/M3/M4) or Intel Core.
- **Sandboxing & Entitlements**:
  - `com.apple.security.app-sandbox`: Enabled.
  - `com.apple.security.files.user-selected.read-write`: Enabled (for user music folder indexing).
  - `com.apple.security.network.client`: Enabled (for optional lyrics retrieval and cloud sync).
- **Code Signing & Notarization**:
  - Cryptographically signed with Developer ID Application certificate.
  - Notarized via Apple Notary Service (`xcrun notarytool`) with stapled ticket.
