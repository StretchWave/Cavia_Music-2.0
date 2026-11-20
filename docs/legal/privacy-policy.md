# Cavia Music — Privacy Policy

**Effective Date**: November 20, 2026  
**Application**: Cavia Music (Version 2.0.0)  
**Publisher**: [PROJECT_OWNER_LEGAL_ENTITY_PLACEHOLDER]  
**Contact**: [PRIVACY_CONTACT_EMAIL_PLACEHOLDER]  
**Public Policy URL**: [PRIVACY_POLICY_URL_PLACEHOLDER]  

---

## 1. Overview & Core Philosophy

Cavia Music is engineered as an **offline-first, local-first audio player**. We believe your music collection, listening habits, playlists, and personal metadata belong exclusively to you. 

- **We do not track you**: Cavia contains **zero telemetry**, **zero analytics SDKs**, **zero crash reporting daemons**, and **zero advertising frameworks**.
- **Your audio never leaves your device**: Cavia never uploads, streams, mirrors, or transmits your audio files or embedded images to any remote server.
- **Offline by default**: All core capabilities—including audio decoding, library indexing, metadata extraction, search, playlist management, and playback state restoration—operate 100% offline without requiring an internet connection or an account.

---

## 2. Information We Store Locally on Your Device

All local application data is stored in your operating system's designated application data directory:
- **Windows**: `%APPDATA%\com.stretchwave.cavia\`
- **macOS**: `~/Library/Application Support/com.stretchwave.cavia/`
- **Android**: `/data/user/0/com.stretchwave.cavia/` (internal app storage)
- **iOS**: `<AppHome>/Library/Application Support/com.stretchwave.cavia/` (sandbox container)

| Data Category | Purpose | Storage Mechanism | Retention |
| :--- | :--- | :--- | :--- |
| **Track Metadata** | Displaying song title, artist, album, duration, year, track number | SQLite (`cavia-library.sqlite3`) | Retained until you remove tracks or reset library |
| **Local File Paths** | Locating and playing audio files on disk | SQLite (`cavia-library.sqlite3`) | Stored locally only; **never transmitted externally** |
| **Content Hashes** | Detecting moved or duplicate audio files via SHA-256 | SQLite (`cavia-library.sqlite3`) | Retained while track is indexed in library |
| **Playlists & Liked Tracks**| Custom collections and favorited songs | SQLite (`cavia-library.sqlite3`) | Retained until deleted by the user |
| **Cached Artwork** | High-performance thumbnail and cover art display | Local filesystem (`artworks/<hash>.<ext>`) | Retained while referenced; cleanable in Settings |
| **Cached Lyrics** | Offline synchronized LRC and plain lyrics display | SQLite (`lyrics_cache` table) | Retained locally; updated upon user refresh |
| **Playback State** | Resuming playback position, speed, and active queue | SQLite (`cavia-library.sqlite3`) | Persisted across application restarts |
| **Credentials** | Storing refresh tokens for optional cloud synchronization | OS Secure Storage (Windows Credential Manager / Keychain / Keystore) | Deleted upon sign-out or account disconnection |

---

## 3. Information Transmitted Externally

Cavia initiates network requests **only in two specific, user-controlled circumstances**:

### 3.1 External Lyrics Lookup (LRCLIB)
When a track does not contain embedded lyrics tags, Cavia queries the public community lyrics database at [LRCLIB](https://lrclib.net).
- **Data Transmitted**: HTTP GET parameters containing public track metadata only:
  - Track Title
  - Artist Name
  - Album Name
  - Duration (rounded to integer seconds)
- **Data NOT Transmitted**: Local filenames, folder paths, IP-tracking headers, user identifiers, or audio bytes.
- **Protocol**: Encrypted HTTPS with strict timeout bounds and automated retry cooldowns (`Retry-After`).
- **User Control**: You can disable external lyrics fetching at any time in **Settings → Audio & Engine → Prefer Embedded Lyrics**. If disabled or offline, Cavia displays embedded lyrics only.

### 3.2 Optional Account & Cloud Synchronization (Supabase)
Cloud synchronization is **100% optional**. You are never required to create an account or connect to the internet to use Cavia Music.
- **Provider**: Supabase Auth & PostgreSQL.
- **When Active**: Only when you explicitly configure a backend in Settings, create an account, and sign in.
- **Data Transmitted When Enabled**:
  - Your account email and encrypted authentication credentials.
  - Cryptographic content hashes (SHA-256) of liked tracks and playlist tracks.
  - Playlist metadata (names, descriptions, and sort positions).
- **Data NEVER Transmitted**:
  - Audio files or audio streams (Cavia never uploads your music).
  - Local filesystem paths or directory structures.
  - High-resolution embedded artwork binaries.
- **Security & Authorization**: Row-Level Security (RLS) policies enforce that you can only access records matching your verified `auth.uid()`. Tokens are transmitted solely over TLS 1.3 and refresh tokens are housed in platform secure credential storage.

---

## 4. Third-Party Services Summary

| Provider | Purpose | Authentication | Privacy Policy / Terms |
| :--- | :--- | :--- | :--- |
| **LRCLIB** (`lrclib.net`) | Community lyrics metadata lookup | None (Unauthenticated HTTPS) | [LRCLIB Privacy](https://lrclib.net/docs) |
| **Supabase** (`supabase.com`) | Optional cross-device playlist & likes sync | User-provided credentials / JWT | [Supabase Privacy Policy](https://supabase.com/privacy) |

---

## 5. Data Retention & Deletion Practices

You maintain sovereign control over your data at all times:
- **Removing a Track or Playlist**: Immediately deletes the record from local SQLite. If Cloud Sync is enabled, a tombstone is synchronized to purge the record from your remote account.
- **Clearing Cached Artwork**: You can trigger **Clean Orphaned Artwork** or **Reset Cache** in Settings. Unreferenced image files are immediately erased from disk.
- **Signing Out of Cloud Sync**: Immediately wipes active session tokens from application memory and purges the refresh token from OS Secure Storage. Your local library remains completely intact.
- **Purging Cloud Data**: Selecting **Delete Cloud Data** in Settings executes a cascading deletion across all remote tables (`cavia_liked_tracks`, `cavia_playlist_tracks`, `cavia_playlists`) associated with your `auth.uid()`.
- **Application Uninstallation**: Uninstalling Cavia removes all application binaries and executables. By design, your local database and audio files are preserved in your OS user profile to prevent catastrophic accidental data loss. To permanently erase all data, delete the `%APPDATA%\com.stretchwave.cavia` directory.

---

## 6. Operating System Permissions

Cavia requests only the minimum OS permissions required to perform audio playback:

| Operating System | Permission | Purpose |
| :--- | :--- | :--- |
| **Windows** | Filesystem Access | Reading local audio folders selected by the user via native file dialog. |
| **Windows** | Audio Device Output | Playing decoded audio streams through default WASAPI audio endpoints. |
| **macOS** | User-Selected Files | Accessing user-specified music directories via App Sandbox security bookmarks. |
| **Android** | `READ_MEDIA_AUDIO` / Storage | Scanning and playing music stored in device media stores. |
| **iOS** | Security-Scoped URLs | Accessing user-selected audio folders staged within iOS sandbox containers. |

---

## 7. Children's Privacy

Cavia Music does not knowingly collect, solicit, or maintain personal information from children under the age of 13 (or under 16 in the European Economic Area). Because the application operates offline without mandatory accounts or user tracking, no personal information is indexed.

---

## 8. Changes to this Privacy Policy

We may periodically update this policy to reflect new capabilities or platform security updates. Material changes will be clearly documented in the application changelog and GitHub release notes.

---

## 9. Contact & Legal Inquiry

For questions, concerns, or requests regarding this Privacy Policy:

- **Entity**: [PROJECT_OWNER_LEGAL_ENTITY_PLACEHOLDER]
- **Email**: [PRIVACY_CONTACT_EMAIL_PLACEHOLDER]
- **Address**: [PROJECT_OWNER_PHYSICAL_ADDRESS_PLACEHOLDER]
- **Repository**: [https://github.com/StretchWave/Cavia_Music-2.0](https://github.com/StretchWave/Cavia_Music-2.0)
