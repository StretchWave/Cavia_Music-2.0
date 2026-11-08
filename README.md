# Cavia Music 2.0 — Foundation

A new local-first music application foundation built with **Tauri 2, React, TypeScript, and Rust**.

This is a clean project, not a migration of the old Flutter codebase. It includes a responsive visual shell, a Rust-owned SQLite database, initial schema migrations, typed Tauri commands for application health and library counts, and an explicit audio-backend boundary.

> **Honest status:** this is the project foundation, not a finished music player. The UI contains no fabricated tracks. The real local audio import pipeline, metadata extraction, desktop/mobile playback engines, provider streaming and media controls are planned next milestones and are not claimed to work yet.

## Target platforms

- Windows desktop
- macOS desktop
- Android
- iOS
- Linux is deliberately excluded from the 2.0 target.

Tauri 2 supports desktop and mobile targets from a shared frontend, but mobile plugins may need Kotlin and Swift implementations. See the platform notes in `docs/architecture.md`.

## Technology choices

| Layer | Choice |
|---|---|
| UI | React 19 + TypeScript |
| Frontend build | Vite 8 |
| App runtime | Tauri 2 |
| Native core | Rust |
| Local database | SQLite through SQLx |
| Native file dialogs | Tauri dialog plugin |
| Audio metadata parsing (planned) | Lofty |
| Desktop audio candidate | libmpv through the `rsmpv` Rust bindings |
| Android audio | Android Media3 / ExoPlayer adapter |
| iOS audio | AVPlayer adapter |
| Lyrics | LRCLIB adapter (planned) |
| Provider metadata | Explicit provider adapters (planned) |

## Requirements

- Node.js 22.12 or newer
- npm 10 or newer
- Rust 1.90 or newer and the platform's C/C++ build tools
- For Android: Android Studio, Android SDK, NDK and a configured JDK
- For iOS: macOS with Xcode and command-line tools
- For Windows: Visual Studio Build Tools with the C++ desktop workload and Windows SDK

Follow the official Tauri prerequisites for the host OS before building.

## Install and run the frontend

```powershell
npm install
npm run dev
```

This opens the design shell in a browser. Browser preview does not have native Tauri commands or audio access; the app clearly labels itself as a design preview.

## Run the Windows desktop app

```powershell
npm run tauri dev
```

## Initialize mobile projects

```powershell
npm run tauri android init
npm run tauri ios init
```

Build Android from a configured Android development environment:

```powershell
npm run tauri android build
```

Build iOS from macOS with Xcode:

```powershell
npm run tauri ios build
```

## Current foundation behavior

- Creates the application data directory on first launch.
- Opens a SQLite database and applies versioned SQL migrations.
- Exposes `app_health` and `library_summary` through Tauri commands.
- Displays a deliberately empty library on fresh installation.
- Disables player controls until a real playback adapter is connected.
- Separates the audio capability model from UI controls so unsupported features cannot be advertised accidentally.

## Next implementation milestones

1. Verify the starter builds on Windows and validate mobile project generation.
2. Implement native file picking and safe file staging on desktop, Android and iOS.
3. Add audio metadata extraction using Lofty and real track CRUD/search.
4. Implement Windows/macOS libmpv playback and verify dynamic library packaging.
5. Implement Android Media3 and iOS AVPlayer adapters behind the same playback contract.
6. Add system media controls, background playback, queue, lyrics, playlists and cache management.
7. Integrate authorized provider catalogue/streaming APIs. Spotify integration is metadata/playlist import unless separately authorized playback is available.

## Security and policy principles

- Never ship a Spotify client secret, Supabase service-role key or other privileged credential in the client.
- Store refresh tokens in platform secure storage, not SQLite plaintext.
- Do not scrape undocumented services or bypass DRM, paywalls or playback restrictions.
- Do not cache provider audio unless the provider explicitly permits it.
- Treat remote stream URLs as short-lived and sensitive; never log query tokens.
- Validate imported file types and handle Android content URIs and iOS sandbox URLs through native staging logic.

## License and distribution

Before shipping, decide the libmpv build/license configuration and include the required third-party notices. See `docs/audio-backends.md` and `docs/decisions.md`.
