# Cavia Music — Third-Party Software Licenses & Attribution

**Release**: Cavia Music 2.0.0  
**Generated**: November 20, 2026  
**SPDX 2.3 SBOM Reference**: [`docs/security/cavia-music-sbom.spdx.json`](file:///c:/Programs/Spoty/Cavia_Music%202.0/docs/security/cavia-music-sbom.spdx.json)  

---

## 1. Overview

Cavia Music is built with open-source software libraries. This document provides notice and attribution for third-party dependencies used in production and development builds, in accordance with their respective open-source license agreements.

All direct and transitive runtime dependencies use permissive open-source licenses:
- **MIT License**
- **Apache License, Version 2.0**
- **ISC License**
- **BSD 2-Clause and 3-Clause Licenses**
- **Mozilla Public License 2.0 (MPL-2.0)**
- **Zlib License**
- **Unicode License Agreement (Unicode-3.0)**

Zero copyleft GPL or AGPL dependencies are linked into the native production binary.

---

## 2. Frontend Dependencies (npm)

| Package | Version | Declared License | Purpose | Source |
| :--- | :--- | :--- | :--- | :--- |
| **`react`** | 19.3.1 | MIT | UI View Component Engine | [npm](https://www.npmjs.com/package/react) |
| **`react-dom`** | 19.3.1 | MIT | DOM Renderer | [npm](https://www.npmjs.com/package/react-dom) |
| **`@tauri-apps/api`** | 2.12.4 | MIT OR Apache-2.0 | Native IPC Bridge | [npm](https://www.npmjs.com/package/@tauri-apps/api) |
| **`@tauri-apps/plugin-dialog`**| 2.8.4 | MIT OR Apache-2.0 | Native File Picker Dialog | [npm](https://www.npmjs.com/package/@tauri-apps/plugin-dialog) |
| **`lucide-react`** | 1.61.0 | ISC | Iconography Suite | [npm](https://www.npmjs.com/package/lucide-react) |
| **`typescript`** (dev) | 7.0.2 | Apache-2.0 | Type System & Compiler | [npm](https://www.npmjs.com/package/typescript) |
| **`vite`** (dev) | 8.3.6 | MIT | Frontend Build Tool & Bundler | [npm](https://www.npmjs.com/package/vite) |
| **`@vitejs/plugin-react`** (dev)| 6.2.1 | MIT | Vite React Fast Refresh | [npm](https://www.npmjs.com/package/@vitejs/plugin-react) |
| **`axe-core`** (dev) | 4.14.1 | MPL-2.0 | Accessibility Automated Auditing | [npm](https://www.npmjs.com/package/axe-core) |

---

## 3. Backend Core Dependencies (Rust / Cargo)

| Crate | Version | License | Primary Purpose | Source |
| :--- | :--- | :--- | :--- | :--- |
| **`tauri`** | 2.12.3 | MIT OR Apache-2.0 | Desktop/Mobile Application Framework | [crates.io](https://crates.io/crates/tauri) |
| **`tokio`** | 1.x | MIT | Async Runtime & Threading | [crates.io](https://crates.io/crates/tokio) |
| **`sqlx`** | 0.9.x | MIT OR Apache-2.0 | Async SQLite ORM & Migrations | [crates.io](https://crates.io/crates/sqlx) |
| **`lofty`** | 0.25.x | MIT OR Apache-2.0 | Audio Tag & Metadata Extraction | [crates.io](https://crates.io/crates/lofty) |
| **`rodio`** | 0.23.x | MIT OR Apache-2.0 | Audio Playback & Decoding Engine | [crates.io](https://crates.io/crates/rodio) |
| **`souvlaki`** | 0.9.3 | MIT OR Apache-2.0 | Windows SMTC / OS Media Controls | [crates.io](https://crates.io/crates/souvlaki) |
| **`reqwest`** | 0.12.x | MIT OR Apache-2.0 | HTTPS Client (Lyrics & Cloud Sync) | [crates.io](https://crates.io/crates/reqwest) |
| **`serde`** | 1.0.x | MIT OR Apache-2.0 | Serialization / Deserialization | [crates.io](https://crates.io/crates/serde) |
| **`keyring`** | 4.2.x | MIT OR Apache-2.0 | OS Secure Credential Storage | [crates.io](https://crates.io/crates/keyring) |
| **`sha2`** | 0.10.x | MIT OR Apache-2.0 | SHA-256 Hashing for Audio Content | [crates.io](https://crates.io/crates/sha2) |
| **`walkdir`** | 2.5.x | Unlicense/MIT | Recursive Audio Folder Traversal | [crates.io](https://crates.io/crates/walkdir) |
| **`uuid`** | 1.x | MIT OR Apache-2.0 | UUID Generation | [crates.io](https://crates.io/crates/uuid) |
| **`url`** | 2.x | MIT OR Apache-2.0 | URL Parsing & Sanitization | [crates.io](https://crates.io/crates/url) |

*A complete, machine-readable inventory of all 590 direct and transitive crates with package URLs and cryptographic checksums is maintained in `docs/security/cavia-music-sbom.spdx.json`.*

---

## 4. Typography & Iconography Attribution

1. **System Font Stack**: Cavia Music utilizes native operating system typography stacks (`system-ui`, `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `Roboto`, `Georgia`, `Cascadia Code`). No commercial, proprietary, or restricted font binaries are embedded or distributed within the application packages.
2. **Lucide Icons**: Lucide is licensed under the ISC License (Copyright (c) 2026 Lucide Contributors). Permitted for open-source and commercial use without royalty.
3. **Japanese Calligraphy Elements**: The archival calligraphic watermarks (`藏`, `音`, `盤`, `選`, `尋`, `奏`, `曲`) represent standard Unicode CJK Unified Ideographs rendered via local system fonts and vector outlines.

---

## 5. Licensing Texts Summary

Standard license texts applicable to dependencies:
- **MIT License**: Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files, to deal in the Software without restriction.
- **Apache License 2.0**: Licensed under the Apache License, Version 2.0. You may not use this file except in compliance with the License.
- **ISC License**: Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted.
- **MPL-2.0**: Used solely by development test tools (`axe-core`); not linked into production consumer binaries.
