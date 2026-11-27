# Cavia Music — Release Security & Hardening Checklist

**Milestone**: M5.6 Final Release Readiness  
**Scope**: Pre-release application security audit  
**Application**: Cavia Music 2.0.0  

> Historical audit artifact. The checked items below are not a current release sign-off. Current platform credential limits are recorded here and in `docs/legal/data-handling.md`; re-verify the remaining controls before relying on this checklist.

---

## 1. Authentication & Credential Storage

- [x] **Windows Secure Token Storage**: The Windows credential adapter uses the OS credential manager through `keyring`.
- [x] **No Plaintext Fallback**: Credential failures return errors; tokens are not written to SQLite or plain settings.
- [ ] **macOS Secure Token Storage**: The code path uses the platform keyring, but runtime writes have not been verified on macOS.
- [x] **Android Keystore Storage**: Android Keystore-backed AES-GCM storage passed a set/get/delete round-trip on the Android 16 x86_64 emulator; physical-device behavior is unverified.
- [ ] **iOS Secure Token Storage**: Native iOS Keychain initialization is not integrated; iOS credential operations must fail closed.
- [ ] **Memory-Only Access Tokens, Sign-Out, and Log Redaction**: Re-audit these flows on each supported platform before marking complete. This historical checklist has not independently verified the current implementation.

---

## 2. Cloud Synchronization & Database Security (Optional Service)

- [x] **Row-Level Security (RLS)**: Remote PostgreSQL policies strictly enforce `auth.uid() = user_id` across all tables (`cavia_liked_tracks`, `cavia_playlists`, `cavia_playlist_tracks`).
- [x] **Zero Audio Data Transmission**: The sync engine synchronizes metadata and SHA-256 track content hashes only. User audio files and embedded artwork binaries are never transmitted.
- [x] **Local Filesystem Path Privacy**: Local file paths (`file_uri`) remain exclusively in local SQLite and are never pushed to the cloud outbox.
- [x] **Atomic Outbox Mutations**: Local mutations use transactional SQLite writes (`BEGIN IMMEDIATE`) with tombstone markers to guarantee collision-free synchronization.
- [ ] **Complete Cloud Purge**: The command and its provider-side behavior need a current end-to-end verification before this is marked complete.

---

## 3. External API Communication (LRCLIB & Supabase)

- [x] **Enforced HTTPS (TLS 1.3)**: All external connections require TLS with strict CA certificate validation. Cleartext HTTP is rejected.
- [x] **Bounded Request Timeouts**: Network requests enforce strict 8-second client timeouts, preventing hung connection states.
- [x] **Response Size Caps**: Lyrics responses are length-bounded to prevent memory exhaustion from maliciously crafted upstream payloads.
- [x] **Polite Cooldown Handling**: Implements serialized request gating and honors HTTP 429 `Retry-After` response headers.
- [x] **Zero Filename Leaks in Lyrics**: Lyrics lookup sends only sanitized track title, artist, album, and duration. Local filenames and paths are never transmitted.

---

## 4. Local Storage & SQLite Database

- [x] **Strict Parameterization**: 100% of SQL queries in Rust use SQLx parameterized queries (`.bind(...)`). Zero string concatenation or formatting in SQL statements.
- [x] **Forward-Only Schema Migrations**: Relational schema updates follow forward-only numbered migrations (`0001` through `0006`).
- [x] **Resilient Checksum Reconciliation**: Startup reconciliation prevents panics and preserves 100% of existing user data during upgrades.
- [x] **Production AppData Lock**: `resolve_app_data_dir` strictly enforces native OS application data directories in release builds (`cfg(not(debug_assertions))`).
- [x] **Atomic Cache Writes**: Artwork cache writes use temporary files with atomic rename and `sync_all()`, preventing truncated cache entries.

---

## 5. Tauri Desktop & Webview Security

- [x] **Strict Content Security Policy (CSP)**:
  ```text
  default-src 'self'; img-src 'self' asset: http://asset.localhost data: blob:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src ipc: http://ipc.localhost http://localhost:1420 ws://localhost:1420
  ```
- [x] **No Arbitrary Remote Scripts**: Inline scripts are disabled; scripts load exclusively from bundled Vite assets.
- [x] **Controlled IPC Allowlist**: Only explicitly registered Tauri commands (`commands::*`) can be invoked from the frontend.
- [x] **Zero Shell / Subprocess Execution from UI**: The frontend cannot spawn shell commands or execute arbitrary native binaries.
- [x] **Scoped File Dialog**: Native folder selection is brokered exclusively through `tauri-plugin-dialog`.

---

## 6. Untrusted Content Sanitization

- [x] **Audio Metadata Sanitization**: Title, artist, album, and lyrics strings from untrusted audio files are rendered via React DOM text nodes, preventing HTML injection or script execution.
- [x] **Filename Sanitization**: Cache filenames are strictly restricted to 64-hex SHA-256 strings with validated image extensions (`.jpg`, `.png`, `.webp`).
- [x] **Safe Fallbacks**: Malformed or unparseable audio tags gracefully fall back to file stems without crashing the player process.
