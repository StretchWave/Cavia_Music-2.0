# Cavia Music — Apple App Store Distribution Metadata

**Application Name**: Cavia Music  
**Bundle ID**: `com.stretchwave.cavia`  
**Version**: `2.0.0`  
**Build Number**: `20000`  
**Primary Category**: Music  
**Secondary Category**: Utilities  
**Content Rating**: 4+ (No mature or objectionable content)  
**Developer**: [APPLE_DEVELOPER_NAME_PLACEHOLDER]  
**Support URL**: [SUPPORT_WEBSITE_URL_PLACEHOLDER]  
**Marketing URL**: [MARKETING_URL_PLACEHOLDER]  
**Privacy Policy URL**: [PRIVACY_POLICY_URL_PLACEHOLDER]  

---

## 1. App Store Copy

### Promotional Text (170 characters max)
Precision local-first music player with obsidian aesthetics, synchronized lyrics, and zero tracking.

### Description
Cavia Music is a precision, editorial local-first music player engineered with Tauri 2, React, and Rust. Designed for discerning music lovers who curate their own collections, Cavia delivers high-fidelity audio playback, seamless local indexing, and synchronized lyrics with complete privacy.

**Highlights**:
- **High-Resolution Playback**: Native decoding for lossless FLAC, ALAC, WAV, AIFF, and standard MP3, AAC, and OGG files.
- **Synchronized Lyrics**: Millisecond-accurate synchronized lyrics display powered by embedded tags and community LRCLIB integration.
- **Editorial Visual Design**: Distinctive obsidian surfaces paired with Georgia serif editorial headers, ivory body text, and vermilion accents.
- **Private by Design**: Zero telemetry, zero analytics SDKs, and zero behavioral tracking. Your music files never leave your device.
- **Optional Cloud Sync**: Sync playlist definitions and favorites across your devices with end-to-end user isolation.

---

## 2. App Store Privacy Nutrition Labels

- **Data Used to Track You**: **None**
- **Data Linked to You**: **None** (Unless user explicitly registers for optional cloud sync: Email address linked to account identity).
- **Data Not Linked to You**:
  - Diagnostic / Performance: **None** (No third-party analytics frameworks).
  - Usage Data: **None**

---

## 3. iOS Technical Specifications

- **Minimum iOS Version**: iOS 16.0 or later.
- **Supported Devices**: iPhone, iPad, iPod touch.
- **Capabilities Declared**:
  - Audio, AirPlay, and Picture in Picture background mode.
- **Storage Sandbox**: Accesses user-imported files via iOS Document Picker security-scoped bookmarks.
