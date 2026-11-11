use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

/// Negative cache lifetime. LRCLIB's documentation notes that missing tracks
/// "may become available in a later request", and instrumental classifications
/// can be corrected upstream. Seven days bounds how long a user can be stuck on
/// a stale negative result while avoiding a network lookup on every drawer open.
/// Positive (`found`) results do not expire: they are real lyrics for this
/// recording and are only invalidated by the metadata/file fingerprint or an
/// explicit refresh.
pub const NEGATIVE_CACHE_TTL_SECS: i64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedLyrics {
    pub track_id: String,
    pub source: String, // 'embedded' | 'lrclib' | 'none'
    pub status: String, // 'found' | 'instrumental' | 'not_found'
    pub plain_lyrics: Option<String>,
    pub raw_synced_lyrics: Option<String>,
    pub is_instrumental: bool,
    pub lrclib_id: Option<i64>,
    pub track_signature: String,
}

/// Computes a SHA-256 fingerprint from track metadata, path and the file's
/// size/modification time.
///
/// Detects:
/// - Title, artist, album, duration or path edits.
/// - A different file replacing the original at the same path when its size or
///   modification time changes (including re-tagging embedded lyrics, which
///   updates the file mtime).
///
/// Does not detect:
/// - A replacement file that preserves both size and mtime (extremely rare).
/// - In-place byte edits that keep the same size and reset mtime.
/// - Embedded-lyrics changes on a filesystem that reports no mtime.
///
/// It never reads or hashes the audio payload, so cache lookups stay cheap.
/// `delete_cached_lyrics` / the refresh command remain the explicit escape
/// hatch.
pub fn compute_track_signature(
    title: &str,
    artist: &str,
    album: &str,
    duration_ms: i64,
    file_uri: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.trim().to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(artist.trim().to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(album.trim().to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(duration_ms.to_string().as_bytes());
    hasher.update(b"|");
    // Hash a normalized path (not the raw URI) so `file:///C:/x` and
    // `C:\x` describe the same file.
    hasher.update(normalized_uri(file_uri).as_bytes());
    hasher.update(b"|file:");
    hasher.update(file_identity(file_uri).as_bytes());
    hex::encode(hasher.finalize())
}

fn file_identity(file_uri: Option<&str>) -> String {
    let Some(uri) = file_uri else {
        return "none".to_string();
    };
    let path = local_path_from_uri(uri);
    match std::fs::metadata(&path) {
        Ok(metadata) => {
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|delta| format!("{}.{:09}", delta.as_secs(), delta.subsec_nanos()))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{}:{}", metadata.len(), mtime)
        }
        Err(_) => "missing".to_string(),
    }
}

fn local_path_from_uri(uri: &str) -> PathBuf {
    if let Some(rest) = uri.strip_prefix("file:///") {
        PathBuf::from(rest)
    } else if let Some(rest) = uri.strip_prefix("file://") {
        PathBuf::from(rest)
    } else {
        PathBuf::from(uri)
    }
}

fn normalized_uri(file_uri: Option<&str>) -> String {
    match file_uri {
        None => String::new(),
        Some(uri) => local_path_from_uri(uri)
            .to_string_lossy()
            .replace('\\', "/"),
    }
}

/// Retrieves cached lyrics from SQLite if present, signature-valid and not an
/// expired negative result. Expired negatives are treated as a cache miss but
/// are left in place until a legitimate fresh result replaces them.
pub async fn get_cached_lyrics(
    pool: &SqlitePool,
    track_id: &str,
    current_signature: &str,
    now_unix: i64,
) -> Result<Option<CachedLyrics>, sqlx::Error> {
    let row: Option<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
        Option<i64>,
        String,
        Option<i64>,
    )> = sqlx::query_as(
        r#"
        SELECT track_id, source, status, plain_lyrics, raw_synced_lyrics,
               is_instrumental, lrclib_id, track_signature,
               CAST(strftime('%s', updated_at) AS INTEGER) AS updated_unix
        FROM lyrics_cache
        WHERE track_id = ?
        "#,
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await?;

    if let Some((
        tid,
        source,
        status,
        plain_lyrics,
        raw_synced_lyrics,
        is_instrumental,
        lrclib_id,
        track_signature,
        updated_unix,
    )) = row
    {
        // Signature verification: if metadata or the file changed, invalidate.
        if track_signature != current_signature {
            return Ok(None);
        }

        let is_negative = status == "not_found" || status == "instrumental";
        if is_negative {
            let age = updated_unix
                .map(|updated| now_unix.saturating_sub(updated))
                .unwrap_or(i64::MAX);
            if age > NEGATIVE_CACHE_TTL_SECS {
                return Ok(None);
            }
        }

        return Ok(Some(CachedLyrics {
            track_id: tid,
            source,
            status,
            plain_lyrics,
            raw_synced_lyrics,
            is_instrumental: is_instrumental != 0,
            lrclib_id,
            track_signature,
        }));
    }

    Ok(None)
}

/// Upserts a lyrics cache record in SQLite.
pub async fn save_cached_lyrics(
    pool: &SqlitePool,
    entry: &CachedLyrics,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO lyrics_cache (
            track_id, source, status, plain_lyrics, raw_synced_lyrics,
            is_instrumental, lrclib_id, track_signature, fetched_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT(track_id) DO UPDATE SET
            source = excluded.source,
            status = excluded.status,
            plain_lyrics = excluded.plain_lyrics,
            raw_synced_lyrics = excluded.raw_synced_lyrics,
            is_instrumental = excluded.is_instrumental,
            lrclib_id = excluded.lrclib_id,
            track_signature = excluded.track_signature,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(&entry.track_id)
    .bind(&entry.source)
    .bind(&entry.status)
    .bind(&entry.plain_lyrics)
    .bind(&entry.raw_synced_lyrics)
    .bind(if entry.is_instrumental { 1 } else { 0 })
    .bind(entry.lrclib_id)
    .bind(&entry.track_signature)
    .execute(pool)
    .await?;

    Ok(())
}

/// Deletes a cache entry by track ID (e.g. for forced refresh).
pub async fn delete_cached_lyrics(pool: &SqlitePool, track_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM lyrics_cache WHERE track_id = ?")
        .bind(track_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cavia_lyrics_sig_{}_{}",
            uuid::Uuid::new_v4(),
            name
        ))
    }

    #[test]
    fn test_signature_detects_metadata_changes() {
        let base = compute_track_signature("Title", "Artist", "Album", 1000, None);
        assert_ne!(
            base,
            compute_track_signature("Title 2", "Artist", "Album", 1000, None)
        );
        assert_ne!(
            base,
            compute_track_signature("Title", "Other Artist", "Album", 1000, None)
        );
        assert_ne!(
            base,
            compute_track_signature("Title", "Artist", "Other Album", 1000, None)
        );
        assert_ne!(
            base,
            compute_track_signature("Title", "Artist", "Album", 2000, None)
        );
        assert_eq!(
            base,
            compute_track_signature(" title ", "ARTIST", "Album", 1000, None)
        );
    }

    #[test]
    fn test_signature_detects_same_path_file_replacement() {
        let path = temp_path("replacement.mp3");
        fs::write(&path, vec![0u8; 64]).unwrap();
        let uri = path.to_string_lossy().to_string();
        let before = compute_track_signature("Title", "Artist", "Album", 1000, Some(&uri));

        // Same path, different size.
        fs::write(&path, vec![0u8; 128]).unwrap();
        let after = compute_track_signature("Title", "Artist", "Album", 1000, Some(&uri));
        assert_ne!(before, after, "size change must invalidate the fingerprint");

        // Same path, same size, new modification time.
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&path, vec![1u8; 128]).unwrap();
        let after_mtime = compute_track_signature("Title", "Artist", "Album", 1000, Some(&uri));
        assert_ne!(
            after, after_mtime,
            "mtime change must invalidate the fingerprint"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_signature_handles_missing_file() {
        let missing = temp_path("missing.mp3");
        let uri = missing.to_string_lossy().to_string();
        let first = compute_track_signature("Title", "Artist", "Album", 1000, Some(&uri));
        let second = compute_track_signature("Title", "Artist", "Album", 1000, Some(&uri));
        assert_eq!(first, second);
    }

    #[test]
    fn test_signature_handles_file_uri_prefix() {
        let path = temp_path("uri.mp3");
        fs::write(&path, vec![0u8; 16]).unwrap();
        let raw = path.to_string_lossy().to_string();
        let prefixed = format!("file:///{}", raw.replace('\\', "/"));
        assert_eq!(
            compute_track_signature("T", "A", "B", 1, Some(&raw)),
            compute_track_signature("T", "A", "B", 1, Some(&prefixed))
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_local_path_from_uri_forms() {
        assert_eq!(
            local_path_from_uri("file:///C:/Music/song.mp3"),
            Path::new("C:/Music/song.mp3")
        );
        assert_eq!(
            local_path_from_uri("C:/Music/song.mp3"),
            Path::new("C:/Music/song.mp3")
        );
        assert_eq!(
            local_path_from_uri("content://media/song.mp3"),
            Path::new("content://media/song.mp3")
        );
    }
}
