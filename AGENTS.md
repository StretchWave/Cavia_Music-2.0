# Cavia Music 2.0 — Agent rules

You are working in a clean Tauri 2 + React + TypeScript + Rust project. The old Flutter repository is a reference only. Do not copy its state architecture, mocks or UI implementation.

## Product priorities
1. Premium, distinctive UI/UX.
2. Lightweight and responsive operation.
3. Real local playback and legally authorized streaming.
4. Reliable Windows, macOS, Android and iOS behavior. Linux is not a target for this release.

## Non-negotiable rules
- Never add fabricated tracks, playlists, artists, album art, playback progress, cache size or sync timestamps to production.
- Never make a control appear functional when its backend is absent. Disable it and state what is missing.
- Never report a provider as operational unless a real health check succeeds.
- Never confuse catalogue search, metadata lookup, playback permission and caching permission.
- Never scrape undocumented endpoints or bypass DRM, authentication, paywalls or provider restrictions.
- Never store provider secrets or refresh tokens in plaintext SQLite. Use platform secure storage.
- Never silently replace a failed track with a different recording.
- Never reset or delete user data to fix a schema problem. Add forward-only migrations and preserve backups.
- Rust owns persisted domain state and native integrations. React owns ephemeral view state only.
- All audio backends implement the same Cavia playback contract. Do not call libmpv, Media3 or AVPlayer directly from page components.
- Handle Windows/macOS paths, Android content URIs and iOS security-scoped file URLs separately. Stage imports safely before treating them as normal file paths.
- Keep network calls cancellable, timeout-bounded and redacted in logs.

## UI quality bar
- Preserve the obsidian / aged ivory / restrained vermilion identity.
- No generic dashboard cards, excessive glow, or glass blur on every row.
- Desktop is a first-class layout; mobile is not a squeezed desktop layout.
- Review screenshots at 360px, 390px, 430px, 768px, 1024px, 1440px and 1920px.
- Verify keyboard, mouse, touch, focus, contrast, reduced motion, loading, empty, error and disabled states.
- Do not use sample music content to make empty screens look populated.

## Verification
- Run `npm run build` after frontend changes.
- Run `cargo fmt --check`, `cargo check` and `cargo test` after Rust changes.
- Run the real Tauri application and exercise the changed feature.
- Test Windows and macOS desktop, Android and iOS separately. A host-only build is not proof of mobile support.
- Record unverified platform behavior explicitly. Do not claim a feature is complete because it compiles.
