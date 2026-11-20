# Cavia Music — Google Play Store Distribution Metadata

**Application Name**: Cavia Music  
**Application ID**: `com.stretchwave.cavia`  
**Version Name**: `2.0.0`  
**Version Code**: `20000`  
**Category**: Music & Audio  
**Developer Name**: [GOOGLE_PLAY_DEVELOPER_NAME_PLACEHOLDER]  
**Contact Email**: [SUPPORT_CONTACT_EMAIL_PLACEHOLDER]  
**Privacy Policy URL**: [PRIVACY_POLICY_URL_PLACEHOLDER]  

---

## 1. Play Store Descriptions

### Short Description (80 characters max)
Editorial local music player with obsidian design and synchronized lyrics.

### Full Description (4000 characters max)
Cavia Music is a precision, editorial local-first music player engineered for listeners who curate their own collections. Experience high-fidelity audio, instant library indexing, and synchronized lyrics with complete privacy.

**Why Cavia Music?**
- **100% Offline & Private**: Zero tracking, zero ads, zero surveillance. Your music collection stays strictly on your device.
- **Hi-Res Audio Playback**: Supports FLAC, MP3, WAV, M4A, AAC, OGG, Opus, and ALAC formats natively.
- **Synchronized Lyrics**: Enjoy synchronized LRC scrolling lyrics with manual scroll resume and tap-to-seek.
- **Precision Library Management**: Automatic deduplication, metadata extraction, and smart playlist organization.
- **Obsidian Aesthetic**: Elegant dark interface designed specifically for OLED mobile displays with aged ivory typography and vermilion accents.
- **Optional Cloud Sync**: Sync your playlists and favorites across devices without uploading audio files.

---

## 2. Google Play Technical Specifications

- **Target SDK**: API Level 37 (Android 15+ readiness).
- **Minimum SDK**: API Level 24 (Android 7.0 Nougat).
- **Supported ABIs**: `arm64-v8a`, `armeabi-v7a`, `x86_64`.
- **Packaging Format**: Android App Bundle (`.aab`) and Universal Release APK (`.apk`).
- **Required Permissions**:
  - `android.permission.READ_MEDIA_AUDIO` (Android 13+ audio access)
  - `android.permission.READ_EXTERNAL_STORAGE` (Legacy Android 7-12 audio access)
  - `android.permission.INTERNET` (Optional lyrics & optional sync)

---

## 3. Google Play Data Safety Declaration

- **Data Collected**:
  - Personal Info: **None** (Optional email if cloud sync is activated).
  - Financial Info: **None**
  - Location: **None**
  - Photos & Videos: **None**
  - Audio Files: **Not Collected** (Processed strictly locally).
  - Device or Other IDs: **None**
- **Data Shared with Third Parties**: **None**
- **Security Practices**:
  - Data encrypted in transit: **Yes (TLS 1.3)**
  - Users can request data deletion: **Yes (In-app one-click cloud purge)**
