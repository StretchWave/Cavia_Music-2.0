use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Valid artwork filename checker.
/// Artwork in the Cavia cache is named `<64-hex-sha256>.<ext>`.
/// Extensions recognized are jpg, jpeg, png, webp, gif.
/// Hidden, temporary (e.g. `.tmp_...`), and non-artwork files are rejected.
pub fn is_valid_artwork_filename(name: &str) -> bool {
    if name.starts_with('.') {
        return false;
    }
    let parts: Vec<&str> = name.rsplitn(2, '.').collect();
    if parts.len() != 2 {
        return false;
    }
    let ext = parts[0].to_ascii_lowercase();
    let hash = parts[1];
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif")
}

/// Resolves a file URI or local path string into a local PathBuf.
pub fn local_path_from_uri(uri: &str) -> PathBuf {
    if let Some(rest) = uri.strip_prefix("file:///") {
        PathBuf::from(rest)
    } else if let Some(rest) = uri.strip_prefix("file://") {
        PathBuf::from(rest)
    } else {
        PathBuf::from(uri)
    }
}

/// Normalizes a path string for comparison across slashes and platforms.
pub fn normalize_path_string(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// Atomically writes artwork picture bytes to `<artworks_dir>/<sha256>.<ext>`.
///
/// Write safety:
/// 1. Computes SHA-256 of the raw picture bytes.
/// 2. If the destination `<sha256>.<ext>` already exists and has the expected byte length,
///    it is verified and reused immediately (zero redundant disk writes).
/// 3. Otherwise, writes to a temporary file (`.tmp_<hash>_<uuid>`) in the exact same cache directory.
/// 4. Flushes and calls `sync_all` on the file descriptor to ensure bytes are durable.
/// 5. Closes the file descriptor before performing an atomic rename to the final hash-based filename.
/// 6. Cleans up temporary files if any error occurs.
///
/// Returns `(hash, final_path, was_written)` where `was_written` is true if a new file was committed.
pub fn write_artwork_atomic(
    artworks_dir: &Path,
    pic_bytes: &[u8],
    ext: &str,
) -> Result<(String, PathBuf, bool), io::Error> {
    let mut hasher = Sha256::new();
    hasher.update(pic_bytes);
    let art_hash = hex::encode(hasher.finalize());
    let normalized_ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let art_filename = format!("{art_hash}.{normalized_ext}");
    let final_path = artworks_dir.join(&art_filename);

    if final_path.exists() {
        if let Ok(meta) = fs::metadata(&final_path) {
            if meta.len() == pic_bytes.len() as u64 && meta.len() > 0 {
                return Ok((art_hash, final_path, false));
            }
        }
    }

    let _ = fs::create_dir_all(artworks_dir);

    let temp_filename = format!(".tmp_{}_{}", art_hash, uuid::Uuid::new_v4());
    let temp_path = artworks_dir.join(&temp_filename);

    let write_res = (|| -> Result<(), io::Error> {
        let mut file = File::create(&temp_path)?;
        file.write_all(pic_bytes)?;
        file.flush()?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&temp_path);
        return Err(e);
    }

    match fs::rename(&temp_path, &final_path) {
        Ok(()) => Ok((art_hash, final_path, true)),
        Err(e) => {
            let _ = fs::remove_file(&temp_path);
            if final_path.exists() {
                if let Ok(meta) = fs::metadata(&final_path) {
                    if meta.len() == pic_bytes.len() as u64 && meta.len() > 0 {
                        return Ok((art_hash, final_path, false));
                    }
                }
            }
            Err(e)
        }
    }
}

/// DTO reporting real filesystem and database artwork cache state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkCacheSummary {
    pub total_bytes: u64,
    pub total_files: u64,
    pub referenced_files: u64,
    pub orphaned_files: u64,
    pub reclaimable_bytes: u64,
    pub missing_referenced_files: u64,
    pub cache_dir_exists: bool,
    pub cache_dir_path: String,
    pub inspection_errors: Vec<String>,
    pub format_counts: HashMap<String, u64>,
}

/// DTO reporting results of safe orphan cleanup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkCleanupResult {
    pub deleted_files: u64,
    pub reclaimed_bytes: u64,
    pub skipped_files: u64,
    pub failures: Vec<String>,
}

/// DTO reporting results of missing artwork detection & recovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkRecoveryResult {
    pub total_checked: u64,
    pub recovered_tracks: u64,
    pub already_valid_tracks: u64,
    pub missing_audio_files: u64,
    pub no_embedded_artwork: u64,
    pub failed_tracks: u64,
    pub reclaimed_or_written_bytes: u64,
    pub errors: Vec<String>,
}

/// DTO reporting individual track recovery outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackRecoveryOutcome {
    pub track_id: String,
    pub success: bool,
    pub status: String,
    pub new_artwork_uri: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OrphanedArtworkFile {
    pub path: PathBuf,
    pub filename: String,
    pub size_bytes: u64,
}

/// The ArtworkService manages the filesystem cache under `<app_data_dir>/artworks`,
/// verifies database references, identifies orphans, performs safe cleanup, and
/// recovers missing artwork from audio files.
#[derive(Clone)]
pub struct ArtworkService {
    pool: SqlitePool,
    artworks_dir: PathBuf,
    op_lock: Arc<Mutex<()>>,
}

impl ArtworkService {
    pub fn new(pool: SqlitePool, artworks_dir: PathBuf) -> Self {
        Self {
            pool,
            artworks_dir,
            op_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn artworks_dir(&self) -> &Path {
        &self.artworks_dir
    }

    /// Exposes the shared operation lock to coordinate between artwork cleanup,
    /// recovery, and library scanner imports under a single mutual-exclusion boundary.
    pub fn op_lock(&self) -> Arc<Mutex<()>> {
        self.op_lock.clone()
    }

    /// Fetches all referenced artwork filenames and normalized paths from SQLite.
    /// Includes both active and soft-deleted tracks (`is_deleted = 1`), as well
    /// as any playlists with artwork references, ensuring soft-deleted tracks
    /// preserve their cached images.
    async fn fetch_referenced_identifiers(
        &self,
    ) -> Result<(HashSet<String>, HashSet<String>), String> {
        let track_rows = sqlx::query(
            "SELECT artwork_uri FROM tracks WHERE artwork_uri IS NOT NULL AND trim(artwork_uri) != ''",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to query track artwork references: {e}"))?;

        let playlist_rows = sqlx::query(
            "SELECT artwork_uri FROM playlists WHERE artwork_uri IS NOT NULL AND trim(artwork_uri) != ''",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to query playlist artwork references: {e}"))?;

        let mut referenced_filenames = HashSet::new();
        let mut referenced_paths = HashSet::new();

        for row in track_rows.iter().chain(playlist_rows.iter()) {
            let uri: String = row.get(0);
            let trimmed = uri.trim();
            if trimmed.is_empty() {
                continue;
            }

            let path = local_path_from_uri(trimmed);
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                referenced_filenames.insert(file_name.to_ascii_lowercase());
            }

            let norm = normalize_path_string(&path);
            referenced_paths.insert(norm);
        }

        Ok((referenced_filenames, referenced_paths))
    }

    /// Inspects the artwork cache directory and database references to compute
    /// an accurate summary of cache usage and integrity.
    pub async fn get_summary(&self) -> Result<ArtworkCacheSummary, String> {
        let dir_exists = self.artworks_dir.exists() && self.artworks_dir.is_dir();
        if !dir_exists {
            return Ok(ArtworkCacheSummary {
                total_bytes: 0,
                total_files: 0,
                referenced_files: 0,
                orphaned_files: 0,
                reclaimable_bytes: 0,
                missing_referenced_files: 0,
                cache_dir_exists: false,
                cache_dir_path: self.artworks_dir.to_string_lossy().to_string(),
                inspection_errors: vec![format!(
                    "Cache directory does not exist or is not a directory: {}",
                    self.artworks_dir.display()
                )],
                format_counts: HashMap::new(),
            });
        }

        let (referenced_filenames, referenced_paths) = self.fetch_referenced_identifiers().await?;

        // Check for missing referenced files across active library tracks
        let active_rows = sqlx::query(
            "SELECT DISTINCT artwork_uri FROM tracks WHERE is_deleted = 0 AND artwork_uri IS NOT NULL AND trim(artwork_uri) != ''",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to query active track artwork: {e}"))?;

        let mut missing_referenced_files = 0u64;
        let mut checked_active_uris = HashSet::new();

        for row in active_rows {
            let uri: String = row.get(0);
            let trimmed = uri.trim();
            if trimmed.is_empty() {
                continue;
            }
            let norm = trimmed.to_ascii_lowercase();
            if checked_active_uris.insert(norm) {
                let local_path = local_path_from_uri(trimmed);
                if !local_path.exists() {
                    missing_referenced_files += 1;
                }
            }
        }

        let read_dir = match fs::read_dir(&self.artworks_dir) {
            Ok(rd) => rd,
            Err(e) => {
                return Ok(ArtworkCacheSummary {
                    total_bytes: 0,
                    total_files: 0,
                    referenced_files: 0,
                    orphaned_files: 0,
                    reclaimable_bytes: 0,
                    missing_referenced_files,
                    cache_dir_exists: true,
                    cache_dir_path: self.artworks_dir.to_string_lossy().to_string(),
                    inspection_errors: vec![format!("Failed to read cache directory: {e}")],
                    format_counts: HashMap::new(),
                });
            }
        };

        let mut total_bytes = 0u64;
        let mut total_files = 0u64;
        let mut referenced_files = 0u64;
        let mut orphaned_files = 0u64;
        let mut reclaimable_bytes = 0u64;
        let mut inspection_errors = Vec::new();
        let mut format_counts = HashMap::new();

        let canonical_dir = self.artworks_dir.canonicalize().ok();

        for entry_res in read_dir {
            let entry = match entry_res {
                Ok(e) => e,
                Err(e) => {
                    inspection_errors.push(format!("Failed to inspect directory entry: {e}"));
                    continue;
                }
            };

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => {
                    inspection_errors.push(format!(
                        "Failed to inspect file type for {}: {e}",
                        entry.path().display()
                    ));
                    continue;
                }
            };

            if file_type.is_dir() {
                // Directories inside cache are never treated as artwork files
                continue;
            }

            if file_type.is_symlink() {
                // Reparse point / symlink safety: verify target stays within canonical cache dir
                if let (Some(canon_dir), Ok(canon_target)) =
                    (&canonical_dir, entry.path().canonicalize())
                {
                    if !canon_target.starts_with(canon_dir) {
                        // Symlink points outside cache; fail closed and ignore
                        continue;
                    }
                } else {
                    continue;
                }
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            if !is_valid_artwork_filename(&file_name) {
                // Non-artwork or temporary files are ignored in artwork counts
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // File disappeared during inspection
                    continue;
                }
                Err(e) => {
                    inspection_errors.push(format!(
                        "Failed to read metadata for {}: {e}",
                        entry.path().display()
                    ));
                    continue;
                }
            };

            let file_size = metadata.len();
            total_files += 1;
            total_bytes = total_bytes.saturating_add(file_size);

            if let Some(ext) = Path::new(&file_name).extension().and_then(|s| s.to_str()) {
                *format_counts.entry(ext.to_ascii_lowercase()).or_insert(0) += 1;
            }

            let lower_name = file_name.to_ascii_lowercase();
            let norm_path = normalize_path_string(&entry.path());

            let is_referenced =
                referenced_filenames.contains(&lower_name) || referenced_paths.contains(&norm_path);

            if is_referenced {
                referenced_files += 1;
            } else {
                orphaned_files += 1;
                reclaimable_bytes = reclaimable_bytes.saturating_add(file_size);
            }
        }

        Ok(ArtworkCacheSummary {
            total_bytes,
            total_files,
            referenced_files,
            orphaned_files,
            reclaimable_bytes,
            missing_referenced_files,
            cache_dir_exists: true,
            cache_dir_path: self.artworks_dir.to_string_lossy().to_string(),
            inspection_errors,
            format_counts,
        })
    }

    /// Finds all orphaned artwork files currently residing in `artworks_dir`.
    /// A file is orphaned if and only if it is a valid artwork file and no track
    /// (active or soft-deleted) or playlist references it.
    pub async fn find_orphaned_artwork(&self) -> Result<Vec<OrphanedArtworkFile>, String> {
        if !self.artworks_dir.exists() || !self.artworks_dir.is_dir() {
            return Ok(Vec::new());
        }

        let (referenced_filenames, referenced_paths) = self.fetch_referenced_identifiers().await?;

        let canonical_dir = self.artworks_dir.canonicalize().ok();
        let read_dir = fs::read_dir(&self.artworks_dir)
            .map_err(|e| format!("Failed to read cache directory: {e}"))?;

        let mut orphans = Vec::new();

        for entry in read_dir.flatten() {
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                continue;
            }

            if file_type.is_symlink() {
                if let (Some(canon_dir), Ok(canon_target)) =
                    (&canonical_dir, entry.path().canonicalize())
                {
                    if !canon_target.starts_with(canon_dir) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            if !is_valid_artwork_filename(&file_name) {
                continue;
            }

            let lower_name = file_name.to_ascii_lowercase();
            let norm_path = normalize_path_string(&entry.path());

            if referenced_filenames.contains(&lower_name) || referenced_paths.contains(&norm_path) {
                // Referenced
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                orphans.push(OrphanedArtworkFile {
                    path: entry.path(),
                    filename: file_name,
                    size_bytes: meta.len(),
                });
            }
        }

        Ok(orphans)
    }

    /// Safely deletes orphaned artwork files.
    ///
    /// Safety guarantees:
    /// - Holds an internal asynchronous operation lock to serialize cleanup against scans/recovery.
    /// - Re-checks database references immediately before removing each file to eliminate
    ///   time-of-check to time-of-use (TOCTOU) races.
    /// - Verifies the file still resides within the canonical `artworks_dir`.
    /// - Never follows symlinks outside the directory, and never deletes directories.
    /// - Returns a comprehensive cleanup summary with exact reclaimed byte count.
    pub async fn clean_orphans(&self) -> Result<ArtworkCleanupResult, String> {
        let _guard = self.op_lock.lock().await;
        self.clean_orphans_locked(None).await
    }

    #[cfg(test)]
    pub(crate) async fn clean_orphans_with_precheck_hook<F, Fut>(
        &self,
        hook: F,
    ) -> Result<ArtworkCleanupResult, String>
    where
        F: FnOnce(SqlitePool, String) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let _guard = self.op_lock.lock().await;
        self.clean_orphans_locked(Some(Box::new(|pool, filename| {
            Box::pin(hook(pool, filename))
        })))
        .await
    }

    async fn clean_orphans_locked(
        &self,
        #[allow(unused_mut, unused_variables)] mut precheck_hook: Option<
            Box<
                dyn FnOnce(
                        SqlitePool,
                        String,
                    )
                        -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
                    + Send,
            >,
        >,
    ) -> Result<ArtworkCleanupResult, String> {
        let orphans = self.find_orphaned_artwork().await?;
        if orphans.is_empty() {
            return Ok(ArtworkCleanupResult {
                deleted_files: 0,
                reclaimed_bytes: 0,
                skipped_files: 0,
                failures: Vec::new(),
            });
        }

        let mut deleted_files = 0u64;
        let mut reclaimed_bytes = 0u64;
        let mut skipped_files = 0u64;
        let mut failures = Vec::new();

        let canonical_dir = self.artworks_dir.canonicalize().ok();

        for orphan in orphans {
            #[cfg(test)]
            if let Some(hook) = precheck_hook.take() {
                hook(self.pool.clone(), orphan.filename.clone()).await;
            }

            // Immediate re-check in SQLite to guard against concurrent imports
            let like_pattern = format!("%{}", orphan.filename);
            let track_ref = sqlx::query("SELECT 1 FROM tracks WHERE artwork_uri LIKE ? LIMIT 1")
                .bind(&like_pattern)
                .fetch_optional(&self.pool)
                .await;

            let playlist_ref =
                sqlx::query("SELECT 1 FROM playlists WHERE artwork_uri LIKE ? LIMIT 1")
                    .bind(&like_pattern)
                    .fetch_optional(&self.pool)
                    .await;

            // Fail-closed validation: A candidate may ONLY be deleted when BOTH database
            // checks succeed (Ok) and BOTH confirm no reference exists (None).
            // Any Err(_) indicates unknown reference status and must fail closed (preserve the file).
            let (track_has_ref, playlist_has_ref) = match (track_ref, playlist_ref) {
                (Ok(t), Ok(p)) => (t.is_some(), p.is_some()),
                (Err(e), _) => {
                    skipped_files += 1;
                    failures.push(format!(
                        "Database error checking track reference for {}: {e}",
                        orphan.filename
                    ));
                    continue;
                }
                (_, Err(e)) => {
                    skipped_files += 1;
                    failures.push(format!(
                        "Database error checking playlist reference for {}: {e}",
                        orphan.filename
                    ));
                    continue;
                }
            };

            if track_has_ref || playlist_has_ref {
                skipped_files += 1;
                continue;
            }

            // Path containment verification
            if let Some(canon_dir) = &canonical_dir {
                if let Ok(canon_file) = orphan.path.canonicalize() {
                    if !canon_file.starts_with(canon_dir) {
                        failures.push(format!(
                            "Refusing to delete file outside cache: {}",
                            orphan.path.display()
                        ));
                        continue;
                    }
                }
            }

            match fs::remove_file(&orphan.path) {
                Ok(()) => {
                    deleted_files += 1;
                    reclaimed_bytes = reclaimed_bytes.saturating_add(orphan.size_bytes);
                }
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // Already gone; ignore
                }
                Err(e) => {
                    failures.push(format!("Failed to delete {}: {e}", orphan.path.display()));
                }
            }
        }

        Ok(ArtworkCleanupResult {
            deleted_files,
            reclaimed_bytes,
            skipped_files,
            failures,
        })
    }

    /// Recovers missing artwork for a single track.
    pub async fn recover_track(&self, track_id: &str) -> Result<TrackRecoveryOutcome, String> {
        let _guard = self.op_lock.lock().await;
        self.recover_track_internal(track_id).await
    }

    async fn recover_track_internal(&self, track_id: &str) -> Result<TrackRecoveryOutcome, String> {
        let row = sqlx::query("SELECT id, file_uri, artwork_uri FROM tracks WHERE id = ? LIMIT 1")
            .bind(track_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Database error querying track {track_id}: {e}"))?;

        let row = match row {
            Some(r) => r,
            None => {
                return Ok(TrackRecoveryOutcome {
                    track_id: track_id.to_string(),
                    success: false,
                    status: "error".to_string(),
                    new_artwork_uri: None,
                    error_message: Some(format!("Track '{track_id}' not found in library")),
                });
            }
        };

        let current_art_uri: Option<String> = row.get("artwork_uri");
        if let Some(uri) = &current_art_uri {
            let p = local_path_from_uri(uri);
            if p.exists() {
                if let Ok(meta) = fs::metadata(&p) {
                    if meta.len() > 0 {
                        return Ok(TrackRecoveryOutcome {
                            track_id: track_id.to_string(),
                            success: true,
                            status: "already_valid".to_string(),
                            new_artwork_uri: Some(uri.clone()),
                            error_message: None,
                        });
                    }
                }
            }
        }

        let file_uri: Option<String> = row.get("file_uri");
        let audio_path = match file_uri {
            Some(ref u) if !u.trim().is_empty() => local_path_from_uri(u),
            _ => {
                return Ok(TrackRecoveryOutcome {
                    track_id: track_id.to_string(),
                    success: false,
                    status: "missing_audio".to_string(),
                    new_artwork_uri: None,
                    error_message: Some("Track has no file URI".to_string()),
                });
            }
        };

        if !audio_path.exists() || !audio_path.is_file() {
            return Ok(TrackRecoveryOutcome {
                track_id: track_id.to_string(),
                success: false,
                status: "missing_audio".to_string(),
                new_artwork_uri: None,
                error_message: Some(format!(
                    "Source audio file does not exist: {}",
                    audio_path.display()
                )),
            });
        }

        let path_clone = audio_path.clone();
        let metadata_res = tokio::task::spawn_blocking(move || {
            crate::services::metadata::extract_metadata(&path_clone)
        })
        .await
        .map_err(|e| format!("Metadata task join error: {e}"))?;

        let metadata = match metadata_res {
            Ok(m) => m,
            Err(e) => {
                return Ok(TrackRecoveryOutcome {
                    track_id: track_id.to_string(),
                    success: false,
                    status: "error".to_string(),
                    new_artwork_uri: None,
                    error_message: Some(format!("Failed to probe audio tags: {e}")),
                });
            }
        };

        let (pic_bytes, ext) = match metadata.picture_data {
            Some(pd) => pd,
            None => {
                // Audio file has no embedded artwork; clear dangling reference so it does not
                // permanently point to a missing file.
                let _ = sqlx::query("UPDATE tracks SET artwork_uri = NULL WHERE id = ?")
                    .bind(track_id)
                    .execute(&self.pool)
                    .await;

                return Ok(TrackRecoveryOutcome {
                    track_id: track_id.to_string(),
                    success: false,
                    status: "no_embedded_artwork".to_string(),
                    new_artwork_uri: None,
                    error_message: Some("No embedded artwork found in audio tags".to_string()),
                });
            }
        };

        let dir = self.artworks_dir.clone();
        let write_res =
            tokio::task::spawn_blocking(move || write_artwork_atomic(&dir, &pic_bytes, &ext))
                .await
                .map_err(|e| format!("Write task join error: {e}"))?;

        let (_hash, art_path, _was_written) = match write_res {
            Ok(res) => res,
            Err(e) => {
                return Ok(TrackRecoveryOutcome {
                    track_id: track_id.to_string(),
                    success: false,
                    status: "error".to_string(),
                    new_artwork_uri: None,
                    error_message: Some(format!("Failed to write recovered artwork: {e}")),
                });
            }
        };

        let new_uri = art_path.to_string_lossy().to_string();
        sqlx::query("UPDATE tracks SET artwork_uri = ? WHERE id = ?")
            .bind(&new_uri)
            .bind(track_id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to update track artwork_uri in database: {e}"))?;

        Ok(TrackRecoveryOutcome {
            track_id: track_id.to_string(),
            success: true,
            status: "recovered".to_string(),
            new_artwork_uri: Some(new_uri),
            error_message: None,
        })
    }

    /// Scans all active library tracks whose referenced artwork file is missing or invalid on disk,
    /// and attempts to recover each from its local audio file.
    pub async fn recover_missing(&self) -> Result<ArtworkRecoveryResult, String> {
        let _guard = self.op_lock.lock().await;

        let active_rows = sqlx::query(
            "SELECT id, file_uri, artwork_uri FROM tracks WHERE is_deleted = 0 AND artwork_uri IS NOT NULL AND trim(artwork_uri) != ''",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to query active tracks: {e}"))?;

        let mut tracks_to_recover = Vec::new();

        for row in active_rows {
            let track_id: String = row.get("id");
            let art_uri: String = row.get("artwork_uri");
            let p = local_path_from_uri(&art_uri);
            if !p.exists() || fs::metadata(&p).map(|m| m.len() == 0).unwrap_or(true) {
                tracks_to_recover.push(track_id);
            }
        }

        let total_checked = tracks_to_recover.len() as u64;
        let mut recovered_tracks = 0u64;
        let mut already_valid_tracks = 0u64;
        let mut missing_audio_files = 0u64;
        let mut no_embedded_artwork = 0u64;
        let mut failed_tracks = 0u64;
        let mut reclaimed_or_written_bytes = 0u64;
        let mut errors = Vec::new();

        for track_id in tracks_to_recover {
            match self.recover_track_internal(&track_id).await {
                Ok(outcome) => match outcome.status.as_str() {
                    "recovered" => {
                        recovered_tracks += 1;
                        if let Some(uri) = outcome.new_artwork_uri {
                            let p = local_path_from_uri(&uri);
                            if let Ok(meta) = fs::metadata(&p) {
                                reclaimed_or_written_bytes =
                                    reclaimed_or_written_bytes.saturating_add(meta.len());
                            }
                        }
                    }
                    "already_valid" => {
                        already_valid_tracks += 1;
                    }
                    "missing_audio" => {
                        missing_audio_files += 1;
                        if let Some(err) = outcome.error_message {
                            errors.push(format!("{track_id}: {err}"));
                        }
                    }
                    "no_embedded_artwork" => {
                        no_embedded_artwork += 1;
                    }
                    _ => {
                        failed_tracks += 1;
                        if let Some(err) = outcome.error_message {
                            errors.push(format!("{track_id}: {err}"));
                        }
                    }
                },
                Err(e) => {
                    failed_tracks += 1;
                    errors.push(format!("{track_id}: {e}"));
                }
            }
        }

        Ok(ArtworkRecoveryResult {
            total_checked,
            recovered_tracks,
            already_valid_tracks,
            missing_audio_files,
            no_embedded_artwork,
            failed_tracks,
            reclaimed_or_written_bytes,
            errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::config::WriteOptions;
    use lofty::id3::v2::Id3v2Tag;
    use lofty::picture::{MimeType, Picture, PictureType};
    use lofty::tag::{Tag, TagExt};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    struct TempDirGuard {
        path: PathBuf,
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn temp_test_dir(prefix: &str) -> TempDirGuard {
        let dir = std::env::temp_dir().join(format!(
            "cavia_art_test_{}_{}",
            prefix,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        TempDirGuard { path: dir }
    }

    struct TempAudioGuard {
        path: PathBuf,
    }

    impl Drop for TempAudioGuard {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn temp_audio_fixture(ext: &str) -> TempAudioGuard {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("audio")
            .join(format!("base.{ext}"));
        let target =
            std::env::temp_dir().join(format!("cavia_art_audio_{}.{}", uuid::Uuid::new_v4(), ext));
        fs::copy(&base, &target).expect("copy base audio fixture");
        TempAudioGuard { path: target }
    }

    async fn create_test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[test]
    fn test_valid_artwork_filename_rules() {
        // Valid 64-hex SHA-256 + recognized extensions
        assert!(is_valid_artwork_filename(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.jpg"
        ));
        assert!(is_valid_artwork_filename(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.png"
        ));
        assert!(is_valid_artwork_filename(
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF.webp"
        ));
        assert!(is_valid_artwork_filename(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.gif"
        ));
        assert!(is_valid_artwork_filename(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.jpeg"
        ));

        // Invalid cases
        assert!(!is_valid_artwork_filename(
            ".tmp_e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.jpg"
        ));
        assert!(!is_valid_artwork_filename("short.jpg"));
        assert!(!is_valid_artwork_filename(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.txt"
        ));
        assert!(!is_valid_artwork_filename(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.mp3"
        ));
        assert!(!is_valid_artwork_filename(""));
        assert!(!is_valid_artwork_filename("noextension"));
    }

    #[test]
    fn test_atomic_write_creates_file_and_reuses_identical() {
        let guard = temp_test_dir("atomic");
        let bytes = b"SAMPLE_IMAGE_DATA_12345";
        let (hash, path, was_written) = write_artwork_atomic(&guard.path, bytes, "jpg").unwrap();

        assert!(was_written);
        assert!(path.exists());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            format!("{hash}.jpg")
        );

        // Calling write again with identical bytes reuses existing file
        let (hash2, path2, was_written2) = write_artwork_atomic(&guard.path, bytes, "jpg").unwrap();
        assert!(!was_written2);
        assert_eq!(hash, hash2);
        assert_eq!(path, path2);
    }

    #[tokio::test]
    async fn test_cache_summary_empty_cache() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("summary_empty");
        let service = ArtworkService::new(pool, guard.path.clone());

        let summary = service.get_summary().await.unwrap();
        assert_eq!(summary.total_bytes, 0);
        assert_eq!(summary.total_files, 0);
        assert_eq!(summary.referenced_files, 0);
        assert_eq!(summary.orphaned_files, 0);
        assert_eq!(summary.reclaimable_bytes, 0);
        assert_eq!(summary.missing_referenced_files, 0);
        assert!(summary.cache_dir_exists);
        assert!(summary.inspection_errors.is_empty());
    }

    #[tokio::test]
    async fn test_cache_summary_with_referenced_and_orphan() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("summary_mixed");

        // Write 2 images to cache
        let bytes1 = b"IMAGE_ONE_REFERENCED";
        let bytes2 = b"IMAGE_TWO_ORPHANED";
        let (_hash1, path1, _) = write_artwork_atomic(&guard.path, bytes1, "jpg").unwrap();
        let (_hash2, _path2, _) = write_artwork_atomic(&guard.path, bytes2, "png").unwrap();

        // Also add an ignored temporary file and a subdirectory
        let tmp_file = guard.path.join(".tmp_unrelated");
        fs::write(&tmp_file, b"TEMP_DATA").unwrap();
        let sub_dir = guard.path.join("subfolder");
        fs::create_dir_all(&sub_dir).unwrap();

        // Reference path1 in tracks table
        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri) VALUES ('t1', 'local', 'Song 1', 'file:///song1.mp3', ?)",
        )
        .bind(path1.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool, guard.path.clone());
        let summary = service.get_summary().await.unwrap();

        assert_eq!(summary.total_files, 2); // only valid artwork files counted
        assert_eq!(summary.referenced_files, 1);
        assert_eq!(summary.orphaned_files, 1);
        assert_eq!(summary.reclaimable_bytes, bytes2.len() as u64);
        assert_eq!(summary.total_bytes, (bytes1.len() + bytes2.len()) as u64);
        assert_eq!(summary.missing_referenced_files, 0);
        assert_eq!(summary.format_counts.get("jpg"), Some(&1));
        assert_eq!(summary.format_counts.get("png"), Some(&1));
    }

    #[tokio::test]
    async fn test_shared_artwork_deduplication_not_orphaned() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("shared");

        let bytes = b"ALBUM_SHARED_COVER_ART";
        let (_hash, path, _) = write_artwork_atomic(&guard.path, bytes, "webp").unwrap();
        let uri = path.to_string_lossy().to_string();

        // 3 tracks share the same artwork file
        for i in 1..=3 {
            sqlx::query(
                "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri) VALUES (?, 'local', 'Song', 'file:///s.mp3', ?)",
            )
            .bind(format!("t{i}"))
            .bind(&uri)
            .execute(&pool)
            .await
            .unwrap();
        }

        let service = ArtworkService::new(pool, guard.path.clone());
        let summary = service.get_summary().await.unwrap();
        assert_eq!(summary.total_files, 1);
        assert_eq!(summary.referenced_files, 1);
        assert_eq!(summary.orphaned_files, 0);

        let orphans = service.find_orphaned_artwork().await.unwrap();
        assert!(
            orphans.is_empty(),
            "Shared artwork must not be detected as orphan"
        );
    }

    #[tokio::test]
    async fn test_soft_deleted_track_preserves_artwork() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("soft_del");

        let bytes = b"SOFT_DELETED_TRACK_ART";
        let (_hash, path, _) = write_artwork_atomic(&guard.path, bytes, "jpg").unwrap();

        // Track is soft-deleted (is_deleted = 1)
        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_del', 'local', 'Del Song', 'file:///d.mp3', ?, 1)",
        )
        .bind(path.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool, guard.path.clone());
        let orphans = service.find_orphaned_artwork().await.unwrap();
        assert!(
            orphans.is_empty(),
            "Artwork referenced by soft-deleted track must be preserved"
        );
    }

    #[tokio::test]
    async fn test_clean_orphans_exact_deletion_and_preservation() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("cleanup");

        let ref_bytes = b"REFERENCED_IMAGE";
        let orphan_bytes = b"ORPHAN_IMAGE";
        let (_h1, ref_path, _) = write_artwork_atomic(&guard.path, ref_bytes, "jpg").unwrap();
        let (_h2, orphan_path, _) = write_artwork_atomic(&guard.path, orphan_bytes, "jpg").unwrap();

        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri) VALUES ('t_keep', 'local', 'Keep', 'file:///k.mp3', ?)",
        )
        .bind(ref_path.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool, guard.path.clone());
        let res = service.clean_orphans().await.unwrap();

        assert_eq!(res.deleted_files, 1);
        assert_eq!(res.reclaimed_bytes, orphan_bytes.len() as u64);
        assert_eq!(res.failures.len(), 0);

        // Referenced image remains intact!
        assert!(ref_path.exists());
        // Orphaned image is gone!
        assert!(!orphan_path.exists());

        // Calling clean_orphans again is idempotent
        let res2 = service.clean_orphans().await.unwrap();
        assert_eq!(res2.deleted_files, 0);
        assert_eq!(res2.reclaimed_bytes, 0);
    }

    #[tokio::test]
    async fn test_missing_artwork_detection() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("missing");

        let ghost_path = guard
            .path
            .join("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.jpg");
        assert!(!ghost_path.exists());

        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_miss', 'local', 'Missing Art Track', 'file:///m.mp3', ?, 0)",
        )
        .bind(ghost_path.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool, guard.path.clone());
        let summary = service.get_summary().await.unwrap();
        assert_eq!(summary.missing_referenced_files, 1);
    }

    #[tokio::test]
    async fn test_recovery_from_embedded_mp3_artwork() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("recover_mp3");
        let audio_fixture = temp_audio_fixture("mp3");

        // Write an ID3v2 picture tag into the MP3 container
        let pic_bytes = b"RECOVERED_EMBEDDED_JPEG_DATA_ABCDEFG";
        let mut tag = Id3v2Tag::new();
        let picture = Picture::unchecked(pic_bytes.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Jpeg)
            .build();
        tag.insert_picture(picture);
        tag.save_to_path(&audio_fixture.path, WriteOptions::default())
            .unwrap();

        // Insert track pointing to a missing artwork file
        let missing_art = guard.path.join("nonexistent_art.jpg");
        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_rec', 'local', 'Recoverable Track', ?, ?, 0)",
        )
        .bind(audio_fixture.path.to_string_lossy().to_string())
        .bind(missing_art.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());
        let outcome = service.recover_track("t_rec").await.unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.status, "recovered");
        assert!(outcome.new_artwork_uri.is_some());

        let new_path = local_path_from_uri(outcome.new_artwork_uri.as_ref().unwrap());
        assert!(new_path.exists());
        assert_eq!(fs::read(&new_path).unwrap(), pic_bytes);

        // Verify database was updated with the recovered path
        let updated_row = sqlx::query("SELECT artwork_uri FROM tracks WHERE id = 't_rec'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let db_uri: Option<String> = updated_row.get("artwork_uri");
        assert_eq!(db_uri, outcome.new_artwork_uri);
    }

    #[tokio::test]
    async fn test_recovery_from_embedded_flac_artwork() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("recover_flac");
        let audio_fixture = temp_audio_fixture("flac");

        // Write VorbisComments picture into the FLAC container
        let pic_bytes = b"FLAC_EMBEDDED_PNG_DATA_123456789";
        let mut tag = Tag::new(lofty::tag::TagType::VorbisComments);
        let picture = Picture::unchecked(pic_bytes.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Png)
            .build();
        tag.push_picture(picture);
        tag.save_to_path(&audio_fixture.path, WriteOptions::default())
            .unwrap();

        let missing_art = guard.path.join("missing_flac_art.png");
        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_flac', 'local', 'FLAC Track', ?, ?, 0)",
        )
        .bind(audio_fixture.path.to_string_lossy().to_string())
        .bind(missing_art.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());
        let outcome = service.recover_track("t_flac").await.unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.status, "recovered");
        let new_path = local_path_from_uri(outcome.new_artwork_uri.as_ref().unwrap());
        assert!(new_path.exists());
        assert_eq!(fs::read(&new_path).unwrap(), pic_bytes);
    }

    #[tokio::test]
    async fn test_recovery_handles_missing_audio_honestly() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("missing_audio");

        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_no_audio', 'local', 'Ghost Audio', 'file:///C:/nonexistent_audio.mp3', 'file:///ghost_art.jpg', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool, guard.path.clone());
        let outcome = service.recover_track("t_no_audio").await.unwrap();

        assert!(!outcome.success);
        assert_eq!(outcome.status, "missing_audio");
        assert!(outcome.error_message.is_some());
    }

    #[tokio::test]
    async fn test_recovery_handles_audio_with_no_embedded_artwork() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("no_art");
        let audio_fixture = temp_audio_fixture("mp3"); // raw base mp3 has no picture tags

        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_no_art', 'local', 'Plain Audio', ?, 'file:///broken_ref.jpg', 0)",
        )
        .bind(audio_fixture.path.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());
        let outcome = service.recover_track("t_no_art").await.unwrap();

        assert!(!outcome.success);
        assert_eq!(outcome.status, "no_embedded_artwork");

        // Broken reference was cleared to NULL
        let row = sqlx::query("SELECT artwork_uri FROM tracks WHERE id = 't_no_art'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let art_uri: Option<String> = row.get("artwork_uri");
        assert_eq!(art_uri, None);
    }

    #[tokio::test]
    async fn test_cache_summary_missing_directory_returns_error_safely() {
        let pool = create_test_pool().await;
        let non_existent_dir =
            std::env::temp_dir().join(format!("cavia_ghost_dir_{}", uuid::Uuid::new_v4()));
        assert!(!non_existent_dir.exists());

        let service = ArtworkService::new(pool, non_existent_dir);
        let summary = service.get_summary().await.unwrap();

        assert!(!summary.cache_dir_exists);
        assert_eq!(summary.total_files, 0);
        assert_eq!(summary.total_bytes, 0);
        assert!(!summary.inspection_errors.is_empty());
    }

    #[tokio::test]
    async fn test_batch_recovery_multiple_tracks_sharing_artwork() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("batch_rec");
        let audio_fixture = temp_audio_fixture("mp3");

        let pic_bytes = b"SHARED_ALBUM_PICTURE_BYTES_99999";
        let mut tag = Id3v2Tag::new();
        let picture = Picture::unchecked(pic_bytes.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Jpeg)
            .build();
        tag.insert_picture(picture);
        tag.save_to_path(&audio_fixture.path, WriteOptions::default())
            .unwrap();

        let ghost1 = guard.path.join("ghost1.jpg");
        let ghost2 = guard.path.join("ghost2.jpg");

        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_batch_1', 'local', 'Track 1', ?, ?, 0)",
        )
        .bind(audio_fixture.path.to_string_lossy().to_string())
        .bind(ghost1.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_batch_2', 'local', 'Track 2', ?, ?, 0)",
        )
        .bind(audio_fixture.path.to_string_lossy().to_string())
        .bind(ghost2.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());
        let res = service.recover_missing().await.unwrap();

        assert_eq!(res.total_checked, 2);
        assert_eq!(res.recovered_tracks, 2);
        assert_eq!(res.failed_tracks, 0);

        // Both tracks now point to the exact same deduplicated file on disk
        let row1 = sqlx::query("SELECT artwork_uri FROM tracks WHERE id = 't_batch_1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let row2 = sqlx::query("SELECT artwork_uri FROM tracks WHERE id = 't_batch_2'")
            .fetch_one(&pool)
            .await
            .unwrap();

        let uri1: String = row1.get("artwork_uri");
        let uri2: String = row2.get("artwork_uri");
        assert_eq!(uri1, uri2);

        let recovered_path = local_path_from_uri(&uri1);
        assert!(recovered_path.exists());
        assert_eq!(fs::read(&recovered_path).unwrap(), pic_bytes);
    }

    #[tokio::test]
    async fn test_recovery_replaces_corrupt_zero_byte_cache_file() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("corrupt_zero_byte");
        let audio_fixture = temp_audio_fixture("mp3");

        let pic_bytes = b"PROPER_NON_CORRUPT_BYTES";
        let mut tag = Id3v2Tag::new();
        let picture = Picture::unchecked(pic_bytes.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Jpeg)
            .build();
        tag.insert_picture(picture);
        tag.save_to_path(&audio_fixture.path, WriteOptions::default())
            .unwrap();

        // Create a 0-byte corrupt file at the target hash path
        let mut hasher = Sha256::new();
        hasher.update(pic_bytes);
        let hash = hex::encode(hasher.finalize());
        let target_path = guard.path.join(format!("{hash}.jpg"));
        fs::write(&target_path, b"").unwrap();
        assert_eq!(fs::metadata(&target_path).unwrap().len(), 0);

        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_corrupt', 'local', 'Corrupt Track', ?, ?, 0)",
        )
        .bind(audio_fixture.path.to_string_lossy().to_string())
        .bind(target_path.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());
        let outcome = service.recover_track("t_corrupt").await.unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.status, "recovered");
        // Verify the 0-byte file was replaced with the real bytes
        assert_eq!(fs::read(&target_path).unwrap(), pic_bytes);
    }

    #[tokio::test]
    async fn test_track_reference_prevents_deletion() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("track_ref_prevent");
        let art_path = guard
            .path
            .join("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.jpg");
        fs::write(&art_path, b"TEST_IMAGE_BYTES").unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());

        // Use precheck hook to dynamically insert a track reference after discovery
        // but before deletion check
        let art_path_str = art_path.to_string_lossy().to_string();
        let res = service
            .clean_orphans_with_precheck_hook(move |p, _filename| async move {
                sqlx::query(
                    "INSERT INTO tracks (id, source_kind, title, file_uri, artwork_uri, is_deleted) VALUES ('t_prevent', 'local', 'Prevent Track', 'file:///test.mp3', ?, 0)",
                )
                .bind(&art_path_str)
                .execute(&p)
                .await
                .unwrap();
            })
            .await
            .unwrap();

        assert_eq!(res.deleted_files, 0, "Candidate must not be deleted");
        assert_eq!(
            res.skipped_files, 1,
            "Candidate must be skipped due to track reference"
        );
        assert!(art_path.exists(), "Artwork file must remain on disk");
    }

    #[tokio::test]
    async fn test_playlist_reference_prevents_deletion() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("playlist_ref_prevent");
        let art_path = guard
            .path
            .join("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789.png");
        fs::write(&art_path, b"TEST_IMAGE_BYTES").unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());

        let art_path_str = art_path.to_string_lossy().to_string();
        let res = service
            .clean_orphans_with_precheck_hook(move |p, _filename| async move {
                sqlx::query(
                    "INSERT INTO playlists (id, name, artwork_uri, is_deleted) VALUES ('p_prevent', 'Prevent Playlist', ?, 0)",
                )
                .bind(&art_path_str)
                .execute(&p)
                .await
                .unwrap();
            })
            .await
            .unwrap();

        assert_eq!(res.deleted_files, 0, "Candidate must not be deleted");
        assert_eq!(
            res.skipped_files, 1,
            "Candidate must be skipped due to playlist reference"
        );
        assert!(art_path.exists(), "Artwork file must remain on disk");
    }

    #[tokio::test]
    async fn test_database_error_in_track_check_prevents_deletion() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("db_err_track");
        let art_path = guard
            .path
            .join("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.jpg");
        fs::write(&art_path, b"TEST_IMAGE_BYTES").unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());

        // In precheck hook, simulate a database failure specifically on the tracks table
        let res = service
            .clean_orphans_with_precheck_hook(move |p, _filename| async move {
                sqlx::query("DROP TABLE tracks").execute(&p).await.unwrap();
            })
            .await
            .unwrap();

        assert_eq!(
            res.deleted_files, 0,
            "Database error must fail closed and refuse deletion"
        );
        assert_eq!(res.skipped_files, 1, "Candidate must be skipped");
        assert_eq!(res.failures.len(), 1, "Failure diagnostic must be recorded");
        assert!(
            res.failures[0].contains("Database error checking track reference"),
            "Error diagnostic should specify track reference check: {}",
            res.failures[0]
        );
        assert!(
            art_path.exists(),
            "Artwork file must remain on disk when database check fails"
        );
    }

    #[tokio::test]
    async fn test_database_error_in_playlist_check_prevents_deletion() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("db_err_playlist");
        let art_path = guard
            .path
            .join("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.jpg");
        fs::write(&art_path, b"TEST_IMAGE_BYTES").unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());

        // In precheck hook, simulate a database failure specifically on the playlists table
        let res = service
            .clean_orphans_with_precheck_hook(move |p, _filename| async move {
                sqlx::query("DROP TABLE playlists")
                    .execute(&p)
                    .await
                    .unwrap();
            })
            .await
            .unwrap();

        assert_eq!(
            res.deleted_files, 0,
            "Database error must fail closed and refuse deletion"
        );
        assert_eq!(res.skipped_files, 1, "Candidate must be skipped");
        assert_eq!(res.failures.len(), 1, "Failure diagnostic must be recorded");
        assert!(
            res.failures[0].contains("Database error checking playlist reference"),
            "Error diagnostic should specify playlist reference check: {}",
            res.failures[0]
        );
        assert!(
            art_path.exists(),
            "Artwork file must remain on disk when database check fails"
        );
    }

    #[tokio::test]
    async fn test_unreferenced_file_is_deleted_successfully() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("unreferenced_deleted");
        let art_path = guard
            .path
            .join("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.jpg");
        let img_bytes = b"DEFINITE_ORPHAN_BYTES_12345";
        fs::write(&art_path, img_bytes).unwrap();

        let service = ArtworkService::new(pool.clone(), guard.path.clone());
        let res = service.clean_orphans().await.unwrap();

        assert_eq!(res.deleted_files, 1, "Unreferenced file must be deleted");
        assert_eq!(res.skipped_files, 0);
        assert_eq!(res.reclaimed_bytes, img_bytes.len() as u64);
        assert!(!art_path.exists(), "Artwork file must be removed from disk");
    }

    #[tokio::test]
    async fn test_scanner_cleanup_concurrency_race_prevented() {
        let pool = create_test_pool().await;
        let guard = temp_test_dir("scan_cleanup_concurrency");
        let audio_dir = guard.path.join("audio");
        let artworks_dir = guard.path.join("artworks");
        fs::create_dir_all(&audio_dir).unwrap();
        fs::create_dir_all(&artworks_dir).unwrap();

        // Create an audio fixture inside audio_dir with embedded front cover picture
        let base_audio = temp_audio_fixture("mp3");
        let audio_target = audio_dir.join("track_with_art.mp3");
        fs::copy(&base_audio.path, &audio_target).unwrap();

        let pic_bytes = b"CONCURRENCY_TEST_EMBEDDED_JPEG_BYTES_777";
        let mut tag = Id3v2Tag::new();
        let picture = Picture::unchecked(pic_bytes.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Jpeg)
            .build();
        tag.insert_picture(picture);
        tag.save_to_path(&audio_target, WriteOptions::default())
            .unwrap();

        let artwork_service = ArtworkService::new(pool.clone(), artworks_dir.clone());
        let op_lock = artwork_service.op_lock();

        // Channels to orchestrate deterministic pause in the scan
        let (pause_tx, pause_rx) = tokio::sync::oneshot::channel::<()>();
        let (continue_tx, continue_rx) = tokio::sync::oneshot::channel::<()>();

        let pause_tx_mutex = Arc::new(std::sync::Mutex::new(Some(pause_tx)));
        let continue_rx_mutex = Arc::new(tokio::sync::Mutex::new(Some(continue_rx)));

        let pause_hook: crate::services::scanner::ScanPauseHook = Arc::new(move || {
            let p_tx = pause_tx_mutex.clone();
            let c_rx = continue_rx_mutex.clone();
            Box::pin(async move {
                if let Some(tx) = p_tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                let mut lock = c_rx.lock().await;
                if let Some(rx) = lock.take() {
                    let _ = rx.await;
                }
            })
        });

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pool_scan = pool.clone();
        let artworks_dir_clone = artworks_dir.clone();
        let scan_task = tokio::spawn(async move {
            crate::services::scanner::scan_directory_with_progress_and_pause(
                pool_scan,
                audio_dir,
                artworks_dir_clone,
                cancel_flag,
                op_lock,
                pause_hook,
                |_| {},
            )
            .await
        });

        // Await the signal that scanner has written the artwork file to disk
        pause_rx.await.unwrap();

        // 1. Verify the artwork file now exists on disk
        let mut hasher = Sha256::new();
        hasher.update(pic_bytes);
        let art_hash = hex::encode(hasher.finalize());
        let expected_art_path = artworks_dir.join(format!("{art_hash}.jpg"));
        assert!(
            expected_art_path.exists(),
            "Artwork file must be written to disk"
        );

        // 2. Verify that the track is NOT yet committed in SQLite
        let track_count: i64 = sqlx::query_scalar("SELECT count(*) FROM tracks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(track_count, 0, "Track must not yet be committed in SQLite");

        // 3. Start cleanup concurrently. Because the scanner holds op_lock, cleanup must wait!
        let service_clone = artwork_service.clone();
        let cleanup_task = tokio::spawn(async move { service_clone.clean_orphans().await });

        // Yield execution to allow cleanup_task to attempt to acquire op_lock
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify cleanup has NOT deleted the artwork file
        assert!(
            expected_art_path.exists(),
            "Cleanup must not delete artwork while scan holds op_lock"
        );

        // 4. Resume the scanner to commit the track and release op_lock
        continue_tx.send(()).unwrap();

        let scan_res = scan_task.await.unwrap().unwrap();
        assert_eq!(scan_res.new_tracks, 1);

        // Cleanup now unblocks and finishes
        let cleanup_res = cleanup_task.await.unwrap().unwrap();
        assert_eq!(
            cleanup_res.deleted_files, 0,
            "Cleanup must not delete newly referenced artwork"
        );

        // The committed track references the existing artwork file
        let row = sqlx::query("SELECT artwork_uri FROM tracks LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let art_uri: String = row.get("artwork_uri");
        let final_path = local_path_from_uri(&art_uri);
        assert!(final_path.exists());
        assert_eq!(fs::read(&final_path).unwrap(), pic_bytes);
    }
}
