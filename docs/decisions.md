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

## ADR-006 — Dynamic Fair Queue, Sequential Preload, Cavia Connect & Offline Fallback

**Decision:**
- Separate collaborative state into **Shared Queue**, **Upcoming Playback Sequence**, and **Preload Window (`0–20`, default `10`)** using unique `queue_entry_id` identifiers independent of canonical track/provider IDs.
- Extend `YtMusicService`'s verified stream cache rather than introducing a second cache; bound preloading to concurrency 1 and yield immediately when active playback resolution runs.
- Persist Cavia Connect sessions, participant penalties, and idempotent outbox events in SQLite (`0010_fair_queue_preload_connect.sql`) with optional Supabase cloud synchronization (`20261127000000_cavia_connect_schema.sql`).
- Preserve the original queue intact during internet outages; shuffle only decoder-verified cached streaming tracks plus verified local library tracks.

**Reason:** Collaborative listening stays fair across bursts and disconnects, upcoming tracks prepare sequentially without starving active playback, and offline recovery resumes seamlessly without mutating the user's queue.

## ADR-007 — Account-Based Cavia Connect Identity, Requests, 24h Ignores & Saved Hosts

**Decision:**
- Issue each authenticated account a permanent, globally unique, immutable 6-character Connect ID from the unambiguous 32-character alphabet (`ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, omitting `0`, `O`, `1`, `I`) strictly for host discovery — never as an authentication secret or authorization grant.
- Enforce join requests, duplicate pending suppression (`UNIQUE` partial index), host-specific 24-hour temporary ignores, rate limits, and participant approval server-side in Supabase `SECURITY DEFINER` RPCs and Postgres RLS (`20261128000000_connect_accounts_requests_ignores.sql`), caching read-only state locally (`0011_connect_accounts_requests_ignores.sql`).
- Keep **Hosting**, **Accept Join Requests**, and **Show Connect ID** strictly independent, and require explicit host approval for saved-host reconnects rather than implicit auto-approval.

**Reason:** Reconnects, saved hosts, and 24-hour ignores survive device switches and reinstalls safely without credential leakage, ambiguous characters, or privilege escalation.
