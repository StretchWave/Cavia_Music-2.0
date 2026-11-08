# Architecture decisions

## ADR-001 — New codebase

**Decision:** Cavia 2.0 is a clean Tauri/React/Rust codebase. The Flutter implementation is a behavioral and product reference only; its architecture and implementation are not copied wholesale.

**Reason:** The old code mixed UI state, playback state, provider behavior and platform assumptions. Carrying it forward would preserve known defects.

## ADR-002 — Tauri 2 + React + TypeScript

**Decision:** Use Tauri 2 with a Vite-powered React/TypeScript SPA. Tauri is frontend agnostic, uses the operating system web renderer and supports desktop plus mobile through Kotlin/Swift plugin extensions.

**Reason:** This provides a highly flexible UI stack and a small native shell. Actual app memory and package size still need benchmarking; a WebView is not free.

## ADR-003 — Rust owns application state and SQLite

**Decision:** Rust commands and services own persistence and native integration. SQLite is accessed through SQLx in the Rust core, rather than letting screens issue arbitrary SQL.

**Reason:** One source of truth across Windows, macOS, Android and iOS; typed domain boundaries and safe native access.

## ADR-004 — Platform-specific audio behind one contract

**Decision:** Desktop libmpv and native mobile playback adapters implement the same Cavia player contract.

**Reason:** Mobile background playback and system media integration are platform-specific. An abstraction lets UI and queue behavior remain shared.

## ADR-005 — Empty means empty

**Decision:** A new install contains no fabricated tracks, albums, artists, playlists, artwork, playback position or sync time. UI demonstrates its visual hierarchy without pretending sample data is real.

**Reason:** Feature status and test evidence must be honest.
