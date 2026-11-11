use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::domain::{ScanProgress, ScanResult};
use crate::services::metadata::extract_metadata;

const SUPPORTED_EXTENSIONS: &[&str] = &["flac", "mp3", "wav", "m4a", "aac", "ogg", "aiff", "alac"];

pub fn calculate_file_hash(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn determine_mime_type(ext: &str) -> &'static str {
    match ext {
        "flac" => "audio/flac",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" | "aac" | "alac" => "audio/mp4",
        "ogg" => "audio/ogg",
        "opus" => "audio/opus",
        "aiff" => "audio/aiff",
        "wma" => "audio/x-ms-wma",
        _ => "audio/octet-stream",
    }
}

pub async fn scan_directory(
    app: AppHandle,
    pool: SqlitePool,
    folder_path: PathBuf,
    artworks_dir: PathBuf,
    cancel_flag: Arc<AtomicBool>,
    op_lock: Arc<Mutex<()>>,
) -> Result<ScanResult, String> {
    scan_directory_with_progress(
        pool,
        folder_path,
        artworks_dir,
        cancel_flag,
        op_lock,
        move |p| {
            let _ = app.emit("library:scan-progress", &p);
        },
    )
    .await
}

#[cfg(test)]
pub(crate) type ScanPauseHook =
    Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

pub async fn scan_directory_with_progress<F>(
    pool: SqlitePool,
    folder_path: PathBuf,
    artworks_dir: PathBuf,
    cancel_flag: Arc<AtomicBool>,
    op_lock: Arc<Mutex<()>>,
    on_progress: F,
) -> Result<ScanResult, String>
where
    F: Fn(ScanProgress) + Send + Sync,
{
    #[cfg(test)]
    {
        scan_directory_with_progress_internal(
            pool,
            folder_path,
            artworks_dir,
            cancel_flag,
            op_lock,
            on_progress,
            None,
        )
        .await
    }
    #[cfg(not(test))]
    {
        scan_directory_with_progress_internal(
            pool,
            folder_path,
            artworks_dir,
            cancel_flag,
            op_lock,
            on_progress,
        )
        .await
    }
}

#[cfg(test)]
pub(crate) async fn scan_directory_with_progress_and_pause<F>(
    pool: SqlitePool,
    folder_path: PathBuf,
    artworks_dir: PathBuf,
    cancel_flag: Arc<AtomicBool>,
    op_lock: Arc<Mutex<()>>,
    pause_hook: ScanPauseHook,
    on_progress: F,
) -> Result<ScanResult, String>
where
    F: Fn(ScanProgress) + Send + Sync,
{
    scan_directory_with_progress_internal(
        pool,
        folder_path,
        artworks_dir,
        cancel_flag,
        op_lock,
        on_progress,
        Some(pause_hook),
    )
    .await
}

async fn scan_directory_with_progress_internal<F>(
    pool: SqlitePool,
    folder_path: PathBuf,
    artworks_dir: PathBuf,
    cancel_flag: Arc<AtomicBool>,
    op_lock: Arc<Mutex<()>>,
    on_progress: F,
    #[cfg(test)] pause_after_artwork_write: Option<ScanPauseHook>,
) -> Result<ScanResult, String>
where
    F: Fn(ScanProgress) + Send + Sync,
{
    if !folder_path.exists() || !folder_path.is_dir() {
        return Err(format!(
            "Directory does not exist: {}",
            folder_path.display()
        ));
    }

    let _ = fs::create_dir_all(&artworks_dir);

    // Phase 1: File Discovery
    let mut audio_paths: Vec<PathBuf> = Vec::new();
    let initial_progress = ScanProgress {
        phase: "discovering".to_string(),
        current_file: folder_path.to_string_lossy().to_string(),
        discovered_count: 0,
        processed_count: 0,
        new_tracks_count: 0,
        duplicates_count: 0,
        errors_count: 0,
        error_message: None,
    };
    on_progress(initial_progress);

    for entry in WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if cancel_flag.load(Ordering::Relaxed) {
            on_progress(ScanProgress {
                phase: "cancelled".to_string(),
                current_file: String::new(),
                discovered_count: audio_paths.len(),
                processed_count: 0,
                new_tracks_count: 0,
                duplicates_count: 0,
                errors_count: 0,
                error_message: Some("Scan cancelled by user during discovery.".to_string()),
            });
            return Ok(ScanResult {
                total_discovered: audio_paths.len(),
                new_tracks: 0,
                duplicates: 0,
                errors: 0,
                cancelled: true,
            });
        }

        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                let ext_lower = ext.to_ascii_lowercase();
                if SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str()) {
                    audio_paths.push(entry.path().to_path_buf());
                }
            }
        }
    }

    let total_discovered = audio_paths.len();
    let mut processed_count = 0;
    let mut new_tracks_count = 0;
    let mut duplicates_count = 0;
    let mut errors_count = 0;

    // Phase 2: Metadata Extraction & Persistence
    for path in audio_paths {
        if cancel_flag.load(Ordering::Relaxed) {
            on_progress(ScanProgress {
                phase: "cancelled".to_string(),
                current_file: path.to_string_lossy().to_string(),
                discovered_count: total_discovered,
                processed_count,
                new_tracks_count,
                duplicates_count,
                errors_count,
                error_message: Some("Scan cancelled by user.".to_string()),
            });
            return Ok(ScanResult {
                total_discovered,
                new_tracks: new_tracks_count,
                duplicates: duplicates_count,
                errors: errors_count,
                cancelled: true,
            });
        }

        processed_count += 1;
        let file_path_str = path.to_string_lossy().to_string();
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&file_path_str)
            .to_string();

        on_progress(ScanProgress {
            phase: "scanning".to_string(),
            current_file: file_name.clone(),
            discovered_count: total_discovered,
            processed_count,
            new_tracks_count,
            duplicates_count,
            errors_count,
            error_message: None,
        });

        let file_size_bytes = match fs::metadata(&path) {
            Ok(m) => m.len() as i64,
            Err(_) => 0,
        };

        let content_hash = match calculate_file_hash(&path) {
            Ok(h) => h,
            Err(e) => {
                errors_count += 1;
                let _ = sqlx::query(
                    "INSERT INTO import_history (id, source_uri, status, error_message) VALUES (?, ?, 'error', ?)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&file_path_str)
                .bind(format!("Hash calculation failed: {e}"))
                .execute(&pool)
                .await;
                continue;
            }
        };

        // Duplicate & Moved-File Detection
        // 1. Check if track with identical content hash already exists
        let existing_by_hash = sqlx::query(
            "SELECT id, file_uri, is_deleted FROM tracks WHERE content_hash = ? LIMIT 1",
        )
        .bind(&content_hash)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();

        if let Some(row) = existing_by_hash {
            let existing_id: String = row.get("id");
            let existing_uri: Option<String> = row.get("file_uri");

            let existing_file_exists = existing_uri
                .as_deref()
                .map(|u| Path::new(u).exists())
                .unwrap_or(false);

            if !existing_file_exists {
                // Audio file was moved or renamed! Update its file_uri.
                let _ = sqlx::query("UPDATE tracks SET file_uri = ?, is_deleted = 0 WHERE id = ?")
                    .bind(&file_path_str)
                    .bind(&existing_id)
                    .execute(&pool)
                    .await;

                new_tracks_count += 1;
                continue;
            } else if existing_uri.as_deref() == Some(&file_path_str) {
                // Exact same file already imported
                duplicates_count += 1;
                continue;
            } else {
                // Duplicate audio file located at a different path
                duplicates_count += 1;
                let _ = sqlx::query(
                    "INSERT INTO import_history (id, source_uri, content_hash, status) VALUES (?, ?, ?, 'duplicate')",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&file_path_str)
                .bind(&content_hash)
                .execute(&pool)
                .await;
                continue;
            }
        }

        // 2. Check if file with exact file_uri already exists
        let existing_by_uri = sqlx::query("SELECT id FROM tracks WHERE file_uri = ? LIMIT 1")
            .bind(&file_path_str)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten();

        if existing_by_uri.is_some() {
            duplicates_count += 1;
            continue;
        }

        // Extract metadata using Lofty
        let metadata = match extract_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                errors_count += 1;
                let _ = sqlx::query(
                    "INSERT INTO import_history (id, source_uri, content_hash, status, error_message) VALUES (?, ?, ?, 'error', ?)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&file_path_str)
                .bind(&content_hash)
                .bind(format!("Metadata extraction failed: {e}"))
                .execute(&pool)
                .await;
                continue;
            }
        };

        // The critical section covers atomic artwork caching AND track insertion in SQLite,
        // synchronizing through the exact same op_lock that ArtworkService::clean_orphans uses.
        let (_artwork_uri, insert_res) = {
            let _artwork_guard = if metadata.picture_data.is_some() {
                Some(op_lock.lock().await)
            } else {
                None
            };

            let mut artwork_uri: Option<String> = None;
            if let Some((pic_bytes, ext)) = &metadata.picture_data {
                if let Ok((_art_hash, art_path, _was_written)) =
                    crate::services::artwork::write_artwork_atomic(&artworks_dir, pic_bytes, ext)
                {
                    artwork_uri = Some(art_path.to_string_lossy().to_string());
                }
            }

            #[cfg(test)]
            if let Some(ref hook) = pause_after_artwork_write {
                hook().await;
            }

            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let mime_type = determine_mime_type(&ext);
            let track_id = format!("track-{}", Uuid::new_v4());

            let insert_res = sqlx::query(
                r#"
                INSERT INTO tracks (
                    id,
                    source_kind,
                    title,
                    artist,
                    album,
                    album_artist,
                    duration_ms,
                    file_uri,
                    artwork_uri,
                    mime_type,
                    file_size_bytes,
                    content_hash,
                    release_year,
                    track_number,
                    disc_number,
                    genre,
                    is_deleted
                ) VALUES (?, 'local', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
                "#,
            )
            .bind(&track_id)
            .bind(&metadata.title)
            .bind(&metadata.artist)
            .bind(&metadata.album)
            .bind(&metadata.album_artist)
            .bind(metadata.duration_ms)
            .bind(&file_path_str)
            .bind(&artwork_uri)
            .bind(mime_type)
            .bind(file_size_bytes)
            .bind(&content_hash)
            .bind(metadata.release_year)
            .bind(metadata.track_number)
            .bind(metadata.disc_number)
            .bind(&metadata.genre)
            .execute(&pool)
            .await;

            (artwork_uri, insert_res)
        };

        match insert_res {
            Ok(_) => {
                new_tracks_count += 1;
                let _ = sqlx::query(
                    "INSERT INTO import_history (id, source_uri, content_hash, status) VALUES (?, ?, ?, 'success')",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&file_path_str)
                .bind(&content_hash)
                .execute(&pool)
                .await;
            }
            Err(e) => {
                errors_count += 1;
                let _ = sqlx::query(
                    "INSERT INTO import_history (id, source_uri, content_hash, status, error_message) VALUES (?, ?, ?, 'error', ?)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&file_path_str)
                .bind(&content_hash)
                .bind(format!("Database insert failed: {e}"))
                .execute(&pool)
                .await;
            }
        }
    }

    // Final completed emission
    let final_progress = ScanProgress {
        phase: "completed".to_string(),
        current_file: String::new(),
        discovered_count: total_discovered,
        processed_count,
        new_tracks_count,
        duplicates_count,
        errors_count,
        error_message: None,
    };
    on_progress(final_progress);

    Ok(ScanResult {
        total_discovered,
        new_tracks: new_tracks_count,
        duplicates: duplicates_count,
        errors: errors_count,
        cancelled: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::io::Write;

    #[test]
    fn test_determine_mime_type() {
        assert_eq!(determine_mime_type("flac"), "audio/flac");
        assert_eq!(determine_mime_type("mp3"), "audio/mpeg");
        assert_eq!(determine_mime_type("wav"), "audio/wav");
        assert_eq!(determine_mime_type("m4a"), "audio/mp4");
        assert_eq!(determine_mime_type("opus"), "audio/opus");
        assert_eq!(determine_mime_type("ogg"), "audio/ogg");
        assert_eq!(determine_mime_type("xyz"), "audio/octet-stream");
    }

    #[test]
    fn test_supported_extensions() {
        assert!(SUPPORTED_EXTENSIONS.contains(&"flac"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"mp3"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"wav"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"aiff"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"alac"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"wma"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"opus"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"exe"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"txt"));
    }

    #[test]
    fn test_calculate_file_hash() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("cavia_test_hash.tmp");
        {
            let mut f = File::create(&test_file).unwrap();
            f.write_all(b"cavia audio test content").unwrap();
        }
        let hash = calculate_file_hash(&test_file).unwrap();
        assert_eq!(hash.len(), 64);
        let _ = fs::remove_file(test_file);
    }

    fn create_valid_wav_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        // RIFF header
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&36u32.to_le_bytes()); // ChunkSize
        bytes.extend_from_slice(b"WAVE");
        // fmt subchunk
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
        bytes.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 for PCM)
        bytes.extend_from_slice(&2u16.to_le_bytes()); // NumChannels (2)
        bytes.extend_from_slice(&44100u32.to_le_bytes()); // SampleRate
        bytes.extend_from_slice(&(44100u32 * 4).to_le_bytes()); // ByteRate
        bytes.extend_from_slice(&4u16.to_le_bytes()); // BlockAlign
        bytes.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample
        // data subchunk
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&0u32.to_le_bytes()); // Subchunk2Size
        bytes
    }

    #[tokio::test]
    async fn test_scan_multiple_formats_and_duplicates() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let temp_base = std::env::temp_dir().join(format!("cavia_test_scan_{}", Uuid::new_v4()));
        let music_dir = temp_base.join("music");
        let artworks_dir = temp_base.join("artworks");
        fs::create_dir_all(&music_dir).unwrap();

        // 1. Create a valid WAV file
        let wav_path = music_dir.join("track_one.wav");
        fs::write(&wav_path, create_valid_wav_bytes()).unwrap();

        // 2. Create another WAV file
        let mut wav2_bytes = create_valid_wav_bytes();
        wav2_bytes.extend_from_slice(b"pcm_data_variation");
        let wav2_path = music_dir.join("track_two.wav");
        fs::write(&wav2_path, wav2_bytes).unwrap();

        // 3. Create a non-audio file (should be ignored)
        let txt_path = music_dir.join("notes.txt");
        fs::write(&txt_path, b"do not import me").unwrap();

        // First scan
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let op_lock = Arc::new(Mutex::new(()));
        let res1 = scan_directory_with_progress(
            pool.clone(),
            music_dir.clone(),
            artworks_dir.clone(),
            cancel_flag.clone(),
            op_lock.clone(),
            |_| {},
        )
        .await
        .unwrap();

        println!("res1: {:?}", res1);
        let rows = sqlx::query("SELECT source_uri, status, error_message FROM import_history")
            .fetch_all(&pool)
            .await
            .unwrap();
        for r in rows {
            let u: String = r.get("source_uri");
            let s: String = r.get("status");
            let e: Option<String> = r.get("error_message");
            println!("history: {u} -> {s} ({e:?})");
        }

        assert_eq!(res1.total_discovered, 2, "Discovered exactly 2 audio files");
        assert_eq!(res1.new_tracks, 2, "Imported 2 new tracks");
        assert_eq!(res1.duplicates, 0, "Zero duplicates on first scan");

        // Second scan (Duplicate detection)
        let res2 = scan_directory_with_progress(
            pool.clone(),
            music_dir.clone(),
            artworks_dir.clone(),
            cancel_flag.clone(),
            op_lock.clone(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(res2.total_discovered, 2);
        assert_eq!(res2.new_tracks, 0, "Duplicate scan adds 0 new tracks");
        assert_eq!(res2.duplicates, 2, "Duplicate scan detects 2 duplicates");

        // Cleanup
        let _ = fs::remove_dir_all(temp_base);
    }

    #[tokio::test]
    async fn test_scan_cancellation() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let temp_base = std::env::temp_dir().join(format!("cavia_test_cancel_{}", Uuid::new_v4()));
        let music_dir = temp_base.join("music");
        fs::create_dir_all(&music_dir).unwrap();

        let wav_path = music_dir.join("test.wav");
        fs::write(&wav_path, create_valid_wav_bytes()).unwrap();

        let cancel_flag = Arc::new(AtomicBool::new(true)); // Pre-cancelled
        let res = scan_directory_with_progress(
            pool,
            music_dir,
            temp_base.join("artworks"),
            cancel_flag,
            Arc::new(Mutex::new(())),
            |_| {},
        )
        .await
        .unwrap();

        assert!(res.cancelled, "Scan should be marked cancelled");
        assert_eq!(res.new_tracks, 0);

        let _ = fs::remove_dir_all(temp_base);
    }
}
