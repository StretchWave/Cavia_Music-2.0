# Cavia Music — Safe Database Backup & Disaster Recovery Guide

**Document Version**: 2.0.0  
**Scope**: Library, Playlist & State Recovery  

---

## 1. Overview & Data Storage Locations

Cavia Music stores your indexed track metadata, playlists, liked tracks, lyrics cache, and playback state in a local SQLite database with Write-Ahead Logging (WAL).

### Application Data Paths
- **Windows**: `%APPDATA%\com.stretchwave.cavia\`
- **macOS**: `~/Library/Application Support/com.stretchwave.cavia/`
- **Linux (if migrated)**: `~/.local/share/com.stretchwave.cavia/`
- **Android**: `/data/user/0/com.stretchwave.cavia/` (internal app data)
- **iOS**: App container `Library/Application Support/com.stretchwave.cavia/`

### Primary Data Files
- `cavia-library.sqlite3`: The primary SQLite relational database.
- `cavia-library.sqlite3-wal`: Active write-ahead log (present during application runtime).
- `cavia-library.sqlite3-shm`: Shared-memory index for WAL mode.
- `artworks/`: Directory of cached album cover images named by their SHA-256 hash.

> [!WARNING]
> Never copy `cavia-library.sqlite3` directly using File Explorer while Cavia Music is running! Doing so while write transactions are in progress may copy an incomplete WAL state, resulting in a malformed database.

---

## 2. Safe Atomic Backup Procedure

To take an atomic, consistent backup of your library database without shutting down or risking corruption:

### Option A: Clean Process Shutdown (Recommended)
1. Close Cavia Music completely (ensure `cavia-music.exe` is not running in Task Manager).
2. Upon graceful exit, Cavia checkpoints and flushes all pending transactions from the WAL file into the main database.
3. Copy the entire `%APPDATA%\com.stretchwave.cavia` directory to your backup drive or secure storage.

### Option B: Atomic SQLite `VACUUM INTO` (Online Backup)
If scripting an automated backup, use SQLite's native `VACUUM INTO` command. This creates a vacuumed, transaction-consistent, single-file snapshot:
```powershell
# Using Python's built-in sqlite3 module
$dbPath = "$env:APPDATA\com.stretchwave.cavia\cavia-library.sqlite3"
$backupPath = "$env:USERPROFILE\Desktop\cavia_backup_$(Get-Date -Format 'yyyyMMdd').sqlite3"
python -c "import sqlite3; con = sqlite3.connect(r'$dbPath'); con.execute(f\"VACUUM INTO '{r'$backupPath'.replace('\\', '/')}'\"); con.close(); print('Backup complete!')"
```

---

## 3. Safe Database Restoration Procedure

If you are migrating to a new computer or recovering from an accidental deletion:

1. **Install Cavia Music** on the new computer.
2. Launch Cavia Music once to initialize the directory structure, then **close Cavia Music completely**.
3. Navigate to the application data directory:
   - Press `Win + R`, type `%APPDATA%\com.stretchwave.cavia`, and press Enter.
4. Replace `cavia-library.sqlite3` with your backed-up database file.
5. If you also backed up the `artworks/` folder, copy it into `%APPDATA%\com.stretchwave.cavia\artworks`.
6. Launch Cavia Music. The application will automatically inspect the database, run any required forward migrations, and restore your tracks, playlists, and favorites.

> [!NOTE]
> Restoring `cavia-library.sqlite3` restores your playlist structures, play counts, and metadata, but does **not** restore physical audio files. If your audio files are stored in a new location, trigger a scan of your new music directory; Cavia's content-hash matching will automatically update track file paths without losing your liked status or playlist positions!
