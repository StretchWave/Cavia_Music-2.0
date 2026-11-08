# Cavia Music 2.0 — Architecture

## Product constraints

1. Lightweight desktop application with a premium, distinctive UI.
2. Mobile builds for Android and iOS from the same product and design system.
3. Local library and playback must work without an account or cloud connection.
4. Streaming is provider-driven. The app must distinguish catalogue search, metadata, authorized playback and caching rights.
5. No fake content or simulated success states in production.

## High-level architecture

```text
React / TypeScript UI
  ├── Design system and motion
  ├── Library / Search / Collections
  ├── Player / Queue / Lyrics
  └── Settings / Provider status
          │ typed Tauri invoke + events
          ▼
Rust application core
  ├── Domain services
  │    ├── Library service
  │    ├── Playlist service
  │    ├── Search orchestration
  │    ├── Provider registry
  │    ├── Playback coordinator
  │    ├── Lyrics service
  │    └── Cache coordinator
  ├── Repositories / SQLite (SQLx)
  ├── Secure token references
  └── Platform adapters
       ├── Windows/macOS audio backend (libmpv candidate)
       ├── Android audio backend (Media3 / ExoPlayer)
       ├── iOS audio backend (AVPlayer)
       ├── File picker / sandbox staging
       └── OS media sessions / remote commands
```

## Key boundary: provider vs playback engine

A provider finds and authorizes content. An audio engine plays a resolved source. These must not be merged.

`ProviderAdapter` responsibilities:
- Search catalogue where supported.
- Resolve canonical metadata.
- Return a permitted playback descriptor only when the provider allows playback.
- Declare caching/background/region/auth capabilities from actual API rules.

`AudioBackend` responsibilities:
- Load an already-authorized local or remote playback descriptor.
- Play, pause, seek, set volume and rate.
- Report state, position, duration and buffering.
- Implement background audio/media-session behavior where the target supports it.

The engine never scrapes a provider or guesses stream URLs. A metadata provider is not automatically a playback provider.

## Shared domain contracts

Use serializable DTOs across the Tauri boundary. Do not expose SQL rows or native engine objects directly to React.

- `Track`: stable Cavia ID, source identity, title, artist, album, duration, artwork reference, local URI or provider identity, availability, metadata.
- `Playlist`: stable ID, name, description, artwork, timestamps and ordered entries.
- `PlaybackSnapshot`: state, active track, queue index, position, duration, buffered position, volume, rate and repeat/shuffle.
- `ProviderCapabilities`: search, metadata, playback, background playback, seeking, caching, offline availability, auth requirements and platform restrictions.
- `AppHealth`: database status, platform, app version, audio backend readiness and provider configuration status.

## Database ownership

SQLite is owned by the Rust core using SQLx. React calls use typed commands and never issues arbitrary SQL.

The initial schema is in `src-tauri/migrations/0001_initial.sql`. Add forward-only migrations. Never reset the database to repair schema problems. Store imported audio in an app-managed directory only when the user chooses copy/import; otherwise preserve a permission-safe original file reference and handle moved/missing files explicitly.

## Mobile-specific file access

A desktop path is not interchangeable with a mobile URI:

- Windows/macOS file dialog returns filesystem paths.
- Android pickers may return `content://` URIs and grant scoped access.
- iOS document pickers may return `file://` URLs with sandbox/security-scope constraints.

The import boundary must stage/read from these native references using platform-safe APIs, then expose an app-managed URI to the cross-platform domain. Never pass a `content://` URI to a normal filesystem API and assume it is a path.

## State and events

Rust is authoritative for persistent library, settings and playback state. React may hold ephemeral view state only: current tab, open sheet, hover, search text and transient UI animation state.

Use Tauri events for playback snapshots and import progress. Keep events typed and scoped. Unsubscribe when screens unmount. Persist queue and playback position using throttled writes, not one database write per audio position tick.

## Error model

Commands return typed, user-safe errors with a stable code and safe message. Detailed causes go to debug logs with tokens/URLs/file metadata redacted. A provider outage must not prevent local library browsing. An unavailable audio backend must not display a working Play state.

## Recommended project layout as implementation grows

```text
src/
  app/                  # app shell, router, app state
  components/           # reusable primitives
  design/               # tokens, themes, motion
  features/
    library/
    search/
    collections/
    player/
    queue/
    lyrics/
    settings/
    providers/
  lib/                  # Tauri invoke wrappers and DTOs
src-tauri/
  src/
    commands/           # narrow command handlers
    domain/             # entities and contracts
    services/           # use cases
    repositories/       # SQLite/provider persistence
    audio/              # common backend contract
    platform/           # desktop/mobile adapters
  migrations/
  capabilities/
```
