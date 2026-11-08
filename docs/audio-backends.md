# Audio backend decision note

## Decision under evaluation

One shared Rust-side playback contract, with different native implementations by target:

| Target | Candidate backend | Native media integration |
|---|---|---|
| Windows | libmpv via `rsmpv` | Windows System Media Transport Controls adapter |
| macOS | libmpv via `rsmpv` | Now Playing / remote command adapter |
| Android | Media3 ExoPlayer | MediaSessionService + notification + audio focus |
| iOS | AVPlayer | MPNowPlayingInfoCenter + MPRemoteCommandCenter |

This is intentionally a split backend. A single desktop decoder should not be forced into iOS/Android if native background playback, audio focus and lock-screen controls are better handled by platform media stacks.

## Desktop candidate: libmpv

`rsmpv` provides safe Rust bindings to libmpv and is a credible candidate for local files and network playback. It depends on a separately built/packaged libmpv. It is therefore not a tiny, zero-dependency audio component: the shared library and codec/build configuration must be shipped and tested on Windows and macOS.

Before adopting it:
1. Build a minimal headless audio-only proof on Windows and macOS.
2. Verify local MP3, AAC/M4A, FLAC, WAV, Ogg Vorbis and Opus.
3. Verify permitted progressive HTTPS audio and HLS/DASH only where supported by the chosen stream protocol.
4. Verify seek, rate, volume, output-device changes, interruption and end-of-track events.
5. Bundle the correct dynamic libraries and test on a clean machine with no developer packages installed.
6. Review mpv/libmpv licensing. mpv is GPL by default; the non-GPL build option and its LGPL implications must be understood before distribution. Do not assume the Rust wrapper license changes the linked libmpv license.

The optional `desktop-mpv` Cargo feature is declared but not enabled by default. It is not yet wired to playback commands.

## Mobile candidates

Android should use Media3 ExoPlayer with a MediaSessionService for background audio, audio focus and notification/lock-screen actions. iOS should use AVPlayer with Now Playing metadata and remote command handling.

A community Tauri plugin (`tauri-plugin-native-audio`) provides an existing Android Media3 / iOS AVPlayer implementation, but it is not an official Tauri plugin and is not a desktop backend. Audit its source, license, lifecycle, supported formats, stream handling, background behavior and maintenance before adopting or forking it. The app should own a thin plugin boundary so the implementation can be replaced without rewriting React or the domain layer.

## Capability honesty

Do not mark HLS, DASH, gapless, crossfade, background playback, caching or a codec as supported until an integration test demonstrates it on that platform. The initial `AudioCapabilities::unavailable()` is the safe default.
