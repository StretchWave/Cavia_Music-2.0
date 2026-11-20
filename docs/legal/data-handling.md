# Cavia Music — Technical Data Handling Architecture

**Document Version**: 2.0.0  
**Scope**: Milestone 5.6 Final Release Readiness  
**Target Architectures**: Windows, macOS, Android, iOS  

---

## 1. Data Classification & Flow Architecture

```
+-------------------------------------------------------------------------------+
|                             CLIENT DEVICE (LOCAL)                             |
|                                                                               |
|  [ Local Audio Storage ]                                                      |
|           |                                                                   |
|           v                                                                   |
|   +---------------+     Lofty Tag Parsing     +----------------------------+  |
|   | Audio Scanner | ------------------------> | Embedded Artwork & Lyrics  |  |
|   +---------------+                           +----------------------------+  |
|           |                                                  |                |
|           | SHA-256 Content Hash                             v                |
|           v                                        [ Disk: artworks/ ]        |
|   +---------------------------------------+                                   |
|   |         SQLite (cavia-library)        |                                   |
|   |  - tracks (file_uri, title, artist)   |                                   |
|   |  - playlists & playlist_tracks        |                                   |
|   |  - liked_tracks                       |                                   |
|   |  - lyrics_cache                       |                                   |
|   +---------------------------------------+                                   |
|           |                                                                   |
|           | Optional Cloud Sync                                               |
|           v                                                                   |
|   +---------------------------------------+                                   |
|   |             Sync Engine               |                                   |
|   |  - outbox mutation queue              |                                   |
|   |  - SHA-256 track matching             |                                   |
|   |  - OS Keyring for auth tokens         |                                   |
|   +---------------------------------------+                                   |
+-------------------------------------------------------------------------------+
         |                                           |
         | Unauthenticated HTTPS GET                 | Authenticated HTTPS (TLS 1.3)
         | Title, Artist, Album, Duration only       | User ID, Track Hashes, Playlists
         v                                           v
+-----------------------+                   +-----------------------------------+
|  LRCLIB (lrclib.net)  |                   |  Supabase (Cloud PostgreSQL/Auth) |
|   Lyrics lookup API   |                   |   Multi-device Sync (Optional)    |
+-----------------------+                   +-----------------------------------+
```

---

## 2. Complete Data Category Inventory

| Data Category | Local Storage Path / Mechanism | Transmitted Remotely? | Destination | Purpose | User Deletion Mechanism |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Audio Files** | User media directories | **NEVER** | N/A | Local playback | User manages audio files directly |
| **Local File Paths** | SQLite `tracks.file_uri` | **NEVER** | N/A | Locating file on disk | Track deletion / library reset |
| **Track Metadata** | SQLite `tracks` table | **Only for Lyrics** (Title, Artist, Album, Duration) | `lrclib.net` | Display & lyrics matching | Track deletion / library reset |
| **Content Hashes** | SQLite `tracks.content_hash` | **Optional Sync Only** (when signed in) | Supabase DB | Bit-for-bit duplicate check & multi-device sync | Unlink cloud account / delete cloud data |
| **Playlists** | SQLite `playlists` & `playlist_tracks` | **Optional Sync Only** (when signed in) | Supabase DB | Custom user collections | In-app playlist deletion |
| **Liked Tracks** | SQLite `liked_tracks` | **Optional Sync Only** (when signed in) | Supabase DB | Favorited tracks list | Toggle unlike in UI |
| **Artwork Images** | `<app_data>/artworks/<hash>.<ext>` | **NEVER** | N/A | High-resolution cover display | "Clean Orphaned Artwork" in Settings |
| **Lyrics (Embedded)** | `<app_data>/cavia-library.sqlite3` | **NEVER** | N/A | Synchronized LRC / plain display | Clear lyrics cache in Settings |
| **Lyrics (Remote)** | `<app_data>/cavia-library.sqlite3` | **Cached from LRCLIB** | `lrclib.net` (incoming) | Synchronized LRC display | Clear lyrics cache in Settings |
| **Auth Tokens** | OS Credential Manager (Keyring) | **To Auth Provider** (TLS 1.3) | Supabase Auth | Session maintenance | Sign out in Settings |
| **Device IDs** | **NOT COLLECTED** | **NEVER** | N/A | N/A | N/A |
| **Telemetry / Logs**| Standard OS stdout/stderr | **NEVER** | N/A | Local debugging | Flushed upon process exit |

---

## 3. Cryptographic Storage & Security Measures

### 3.1 Platform Secure Storage (`keyring`)
Authentication refresh tokens are strictly segregated from unencrypted SQLite database files:
- **Windows**: Stored via Windows Credential Manager (`wincred` API).
- **macOS**: Stored in Apple Keychain (`SecItemAdd` / `SecItemCopyMatching`).
- **Android**: Stored in Android KeyStore / EncryptedSharedPreferences.
- **iOS**: Stored in iOS Keychain Services with `kSecAttrAccessibleAfterFirstUnlock`.

### 3.2 Transit Encryption
All outbound network calls require **HTTPS with TLS 1.3** and mandatory certificate validation. Cleartext HTTP connections are rejected by native network drivers and blocked by Tauri's Content Security Policy.

### 3.3 Relational Database Isolation
The SQLite database is configured with:
- **Write-Ahead Logging (`journal_mode = WAL`)**: Ensures readers and writers never block, eliminating database corruption during sudden termination.
- **Foreign Key Enforcement (`foreign_keys = ON`)**: Cascading deletions clean up associations when tracks or playlists are deleted.
- **Strict OS File Permissions**: Created with default user-only read/write permissions inside the OS application container.
