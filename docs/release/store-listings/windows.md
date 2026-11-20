# Cavia Music — Windows Distribution Metadata

**Application Name**: Cavia Music  
**Package Identifier**: `com.stretchwave.cavia`  
**Current Version**: `2.0.0`  
**Target Architecture**: `x86_64-pc-windows-msvc`  
**Package Format**: NSIS Setup Executable (`.exe`)  
**Publisher**: [PROJECT_OWNER_LEGAL_ENTITY_PLACEHOLDER]  
**Publisher ID**: [MICROSOFT_PARTNER_CENTER_PUBLISHER_ID_PLACEHOLDER]  
**Support Email**: [SUPPORT_CONTACT_EMAIL_PLACEHOLDER]  
**Privacy Policy URL**: [PRIVACY_POLICY_URL_PLACEHOLDER]  

---

## 1. Store Product Descriptions

### Short Description (100 characters max)
Editorial local-first music player with obsidian aesthetics and native performance.

### Long Description
Cavia Music is a precision, editorial local-first music player engineered with Tauri 2, React, and Rust. Designed for music lovers who curate their own collections, Cavia delivers bit-perfect audio playback, seamless local indexing, and synchronized lyrics without surveillance or subscription paywalls.

**Core Capabilities**:
- **Real Local Music Playback**: Decodes FLAC, MP3, WAV, M4A, AAC, OGG, Opus, AIFF, and ALAC natively via zero-dependency Rust audio decoders.
- **Instant Library Indexing**: Recursively scans your audio folders, extracts embedded ID3v2/Vorbis metadata, caches album covers with SHA-256 deduplication, and detects moved files automatically.
- **Synchronized Lyrics**: Displays embedded LRC lyrics and community lyrics from LRCLIB with millisecond-accurate synchronized scrolling and click-to-seek navigation.
- **Editorial Obsidian Aesthetic**: Tailored dark visual system pairing Georgia editorial typography, warm ivory body text, and restrained vermilion accents with an adaptive turntable showcase.
- **Windows Integration**: Native System Media Transport Controls (SMTC) integration supporting media keys, lock screen playback controls, and volume synchronization.
- **100% Offline & Private**: Zero telemetry, zero analytics, zero ads. Your music files and personal listening habits never leave your computer.
- **Optional Cloud Synchronization**: Optional end-to-end multi-device sync for playlists and liked tracks via Supabase Auth. Audio files are never uploaded.

---

## 2. Windows Technical Specifications

- **Operating System**: Windows 10 (Version 1809+) or Windows 11 (64-bit).
- **Runtime Prerequisites**: Microsoft Edge WebView2 Runtime (preinstalled on Windows 10/11).
- **Binary Footprint**: ~13 MB installed executable; ~4.2 MB installer package.
- **Installation Path**: `%LOCALAPPDATA%\Programs\Cavia Music\` (Current user installation; no administrator rights required).
- **Application Data**: `%APPDATA%\com.stretchwave.cavia\` (Retained upon app upgrade/uninstall to prevent library data loss).

---

## 3. Microsoft Store Declarations

- **Category**: Multimedia & Audio > Music Players
- **Age Rating**: Everyone / 3+ (PEGI 3, ESRB Everyone).
- **Content Declarations**:
  - Contains Ads: **No**
  - In-App Purchases: **No**
  - Requires Account: **No**
  - Network Required: **No** (Operates 100% offline; network used only for optional lyrics and optional cloud sync).
- **Capability Declarations**:
  - `runFullTrust`: Desktop application bridge for native audio engine and local file access.
