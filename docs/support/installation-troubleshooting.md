# Cavia Music — Installation & Update Troubleshooting

---

## 1. Windows SmartScreen Warning on First Install

### Symptom
Windows displays: *"Windows protected your PC — Microsoft Defender SmartScreen prevented an unrecognized app from starting."*

### Cause
Cavia Music release binaries are newly compiled and building reputational trust with Microsoft SmartScreen. If the release candidate is unsigned or self-signed during community distribution review, Windows displays an informational advisory.

### Resolution
1. Click **More info**.
2. Click **Run anyway**.
3. The official installer SHA-256 checksum can be validated against [`release-manifest.json`](file:///c:/Programs/Spoty/Cavia_Music%202.0/release-manifest.json) before running.

---

## 2. Missing Microsoft Edge WebView2 Runtime

### Symptom
Application fails to launch with an error indicating `WebView2Loader.dll` or missing runtime.

### Cause
Cavia Music uses Microsoft Edge WebView2 for UI rendering. Windows 11 and recent Windows 10 updates include WebView2 by default, but stripped LTSC or enterprise machines may lack the runtime.

### Resolution
Download and install the official Microsoft Edge WebView2 Evergreen Bootstrapper from:
`https://developer.microsoft.com/en-us/microsoft-edge/webview2/`

---

## 3. Upgrade Installation & Data Preservation

### Behavior
Installing a newer version of Cavia Music over an existing version automatically replaces the application binaries in `%LOCALAPPDATA%\Programs\Cavia Music\` without touching your library data, playlists, or artwork cache in `%APPDATA%\com.stretchwave.cavia\`.

### Safe Downgrade / Rollback
If a newer release introduces an issue with your setup:
1. Do not delete your database or application data directory.
2. Run the installer for the previous stable version.
3. Cavia's resilient schema reconciliation will recognize the database and boot safely.
