# Performance and Measurement Notes

This page records measurements that were actually collected. A successful build or a responsive screenshot is not a runtime performance benchmark.

## Frontend bundle

Measured on Windows with `npm run build` on 2026-11-27:

| Asset | Raw size | Gzip size |
| --- | ---: | ---: |
| JavaScript | 412.94 kB | 116.10 kB |
| CSS | 115.97 kB | 20.60 kB |
| `index.html` | 0.53 kB | 0.33 kB |

This is the Vite production output, not the installed application size or a startup-time measurement. No before/after bundle baseline is available in this work session.

## Interaction and layout checks

- DOM viewport widths were checked at 360, 390, 430, 768, 1024, 1440, and 1920 CSS pixels; the checked library view had no horizontal overflow.
- Windows desktop Library, Search, Settings, expanded player, and synced lyric presentation were opened in the Tauri runtime at 1440×900.
- Android local playback was exercised on an Android 16 x86_64 emulator with a temporary WAV fixture; this does not measure physical-device latency or sustained playback performance.
- YouTube Music search requests are cancelled when superseded or when the search view is left. Category policy avoids online calls for local-only search categories.
- Reduced-motion settings avoid running the app's motion effects when motion is disabled or the OS preference requests reduced motion.

## Not measured

There is no reproducible current baseline for startup time, steady-state CPU or memory, play/pause latency, track-switch latency, seek latency, 50,000-track scan/search timing, artwork-cache throughput, or mobile battery usage. Historical numeric comparisons in older milestone notes were removed or marked unverified because their benchmark inputs and runs could not be reproduced here.

## Next benchmark pass

Capture repeatable before/after measurements with the same device, app build, fixture library, and warm/cold-cache conditions. Record at least startup time, local search latency, scan throughput, play/pause and seek response, idle/playing CPU and memory, and Android sustained playback. Keep fixture data isolated from the user's library and do not publish unmeasured estimates as results.
