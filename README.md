# Cavia Music 2.0

A local-first music application built with **Tauri 2, React, TypeScript, and Rust**.

This is a clean Tauri 2 project, not a migration of the old Flutter codebase. Rust owns the local library, SQLite persistence, playback contract and native integrations; React owns view state and presentation.

> **Honest status:** This pass targets Windows desktop and Android. Windows UI and live YouTube Music search were verified in the Tauri runtime; the Rust live-provider smoke test resolved, cached, and decoded a real track. Android local and YouTube Music playback were exercised on an Android 16 x86_64 emulator, including live search and Media3 playback. Android physical-device output and background playback remain unverified. YouTube Music uses undocumented endpoints and may stop working when the service changes. See [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) for evidence.

## Target platforms

- Windows desktop
- Android
- Linux is deliberately excluded from the 2.0 target.
- macOS and iOS are outside the current verification scope.

Tauri 2 supports desktop and mobile targets from a shared frontend, but mobile plugins may need Kotlin and Swift implementations. See the platform notes in `docs/architecture.md`.

## Technology choices

| Layer | Choice |
|---|---|
| UI | React 19 + TypeScript |
| Frontend build | Vite 8 |
| App runtime | Tauri 2 |
| Native core | Rust |
| Local database | SQLite through SQLx |
| Native file dialogs | Tauri dialog plugin (desktop); Android SAF with private staging |
| Audio metadata parsing | Lofty |
| Desktop audio | Rodio + Symphonia; optional MPV feature is not enabled by default |
| Android audio | Media3 ExoPlayer + MediaSessionService; emulator local and cache-then-decode provider playback verified |
| Lyrics | Embedded tags + LRCLIB lookup and cache |
| Provider catalogue | YouTube Music InnerTube adapter (undocumented API) |
| Playlist metadata import | Spotify public playlist import and background matching |

## Requirements

- Node.js 22.12 or newer
- npm 10 or newer
- Rust 1.90 or newer and the platform's C/C++ build tools
- For Android: Android Studio, Android SDK, NDK and a configured JDK
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

## Initialize Android project

```powershell
npm run tauri android init
```

Build Android from a configured Android development environment:

```powershell
npm run tauri android build
```

## Current behavior

- Creates the application data directory on first launch.
- Opens a SQLite database and applies versioned SQL migrations.
- Exposes `app_health` and `library_summary` through Tauri commands.
- Displays a deliberately empty library on fresh installation.
- Uses the shared playback contract; Android routes transport to Media3 and stages SAF folder imports into app-private storage.
- Supports desktop queue and playback persistence, embedded/LRCLIB synchronized lyrics, artwork cache maintenance and optional Supabase sync. Enhanced-LRC word highlighting is used only when the source supplies explicit per-word timestamps.
- Keeps YouTube Music catalogue search and playback behind a provider-specific service; the endpoints are undocumented and may change. Current network playback support is platform-dependent and is listed in `PLATFORM_MATRIX.md`.
- Records measured bundle and runtime evidence, and gaps, in [`docs/performance.md`](docs/performance.md).

## Next implementation milestones

1. Exercise Android playback controls, SAF folder import, background audio and media keys end to end on physical hardware.
2. Measure performance and complete end-to-end regression checks across Windows and Android.

## Security and policy principles

- Never ship a Spotify client secret, Supabase service-role key or other privileged credential in the client.
- Store refresh tokens in platform secure storage, not SQLite plaintext.
- YouTube Music is retained for this personal-use project through undocumented endpoints; keep that provider code isolated, avoid DRM/paywall circumvention, and do not describe the integration as stable or officially supported.
- Keep provider caching bounded and cancellable, and do not claim playback or caching permissions that the service has not granted.
- Treat remote stream URLs as short-lived and sensitive; never log query tokens.
- Validate imported file types and handle Android content URIs and iOS sandbox URLs through native staging logic.

## License and distribution

Before shipping, decide the libmpv build/license configuration and include the required third-party notices. See `docs/audio-backends.md` and `docs/decisions.md`.
