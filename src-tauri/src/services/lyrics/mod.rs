pub mod cache;
pub mod embedded;
pub mod lrclib;
pub mod parser;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub use cache::{CachedLyrics, compute_track_signature, get_cached_lyrics, save_cached_lyrics};
pub use embedded::{EmbeddedLyricsResult, extract_embedded_lyrics};
pub use lrclib::{LrclibClient, LrclibError, LrclibResult};
pub use parser::{LyricsLine, ParsedLrc, is_likely_lrc, parse_lrc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResponseDTO {
    pub track_id: String,
    pub status: String, // "synced" | "plain" | "instrumental" | "not_found"
    pub source: Option<String>, // "embedded" | "lrclib"
    pub plain_lyrics: Option<String>,
    pub synced_lines: Option<Vec<LyricsLine>>,
    pub is_instrumental: bool,
}

pub struct LyricsService {
    pool: SqlitePool,
    lrclib_client: Arc<LrclibClient>,
    now_unix: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl LyricsService {
    pub fn new(pool: SqlitePool) -> Result<Self, String> {
        let client = LrclibClient::new()?;
        Ok(Self {
            pool,
            lrclib_client: Arc::new(client),
            now_unix: Arc::new(system_now_unix),
        })
    }

    pub fn with_client(pool: SqlitePool, client: LrclibClient) -> Self {
        Self::with_client_and_clock(pool, client, Arc::new(system_now_unix))
    }

    /// Test seam: inject a clock so negative-cache expiry can be verified
    /// without sleep-based tests.
    pub fn with_client_and_clock(
        pool: SqlitePool,
        client: LrclibClient,
        now_unix: Arc<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        Self {
            pool,
            lrclib_client: Arc::new(client),
            now_unix,
        }
    }

    /// Fetches lyrics for a track, checking cache, embedded tags, and LRCLIB in order.
    pub async fn get_lyrics(&self, track_id: &str) -> Result<LyricsResponseDTO, String> {
        self.resolve_lyrics(track_id, false).await
    }

    /// Forces a refresh by bypassing existing cache.
    pub async fn refresh_lyrics(&self, track_id: &str) -> Result<LyricsResponseDTO, String> {
        self.resolve_lyrics(track_id, true).await
    }

    async fn resolve_lyrics(
        &self,
        track_id: &str,
        force_refresh: bool,
    ) -> Result<LyricsResponseDTO, String> {
        // 1. Resolve track from SQLite
        let track_row: Option<(String, String, String, i64, Option<String>)> = sqlx::query_as(
            r#"
            SELECT title, artist, album, duration_ms, file_uri
            FROM tracks
            WHERE id = ? AND is_deleted = 0
            "#,
        )
        .bind(track_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error resolving track: {e}"))?;

        let (title, artist, album, duration_ms, file_uri) = match track_row {
            Some(row) => row,
            None => return Err(format!("Track not found in library: {track_id}")),
        };

        // 2. Compute signature
        let signature =
            compute_track_signature(&title, &artist, &album, duration_ms, file_uri.as_deref());

        // 3. Check SQLite cache if not forcing refresh
        if !force_refresh {
            if let Ok(Some(cached)) =
                get_cached_lyrics(&self.pool, track_id, &signature, (self.now_unix)()).await
            {
                if cached.status == "instrumental" {
                    return Ok(LyricsResponseDTO {
                        track_id: track_id.to_string(),
                        status: "instrumental".to_string(),
                        source: Some(cached.source),
                        plain_lyrics: None,
                        synced_lines: None,
                        is_instrumental: true,
                    });
                } else if cached.status == "not_found" {
                    return Ok(LyricsResponseDTO {
                        track_id: track_id.to_string(),
                        status: "not_found".to_string(),
                        source: None,
                        plain_lyrics: None,
                        synced_lines: None,
                        is_instrumental: false,
                    });
                } else if cached.status == "found" {
                    if let Some(raw_lrc) = &cached.raw_synced_lyrics {
                        let parsed = parse_lrc(raw_lrc);
                        return Ok(LyricsResponseDTO {
                            track_id: track_id.to_string(),
                            status: "synced".to_string(),
                            source: Some(cached.source),
                            plain_lyrics: cached.plain_lyrics.or(Some(parsed.plain_text)),
                            synced_lines: Some(parsed.lines),
                            is_instrumental: false,
                        });
                    } else if let Some(plain) = &cached.plain_lyrics {
                        return Ok(LyricsResponseDTO {
                            track_id: track_id.to_string(),
                            status: "plain".to_string(),
                            source: Some(cached.source),
                            plain_lyrics: Some(plain.clone()),
                            synced_lines: None,
                            is_instrumental: false,
                        });
                    }
                }
            }
        }

        // 4. Inspect embedded tags (Priority 1)
        if let Some(uri) = &file_uri {
            let path_str = if uri.starts_with("file:///") {
                &uri[8..]
            } else if uri.starts_with("file://") {
                &uri[7..]
            } else {
                uri.as_str()
            };

            let local_path = PathBuf::from(path_str);
            let embedded = extract_embedded_lyrics(&local_path);

            match embedded {
                EmbeddedLyricsResult::Synced { raw_lrc } => {
                    let parsed = parse_lrc(&raw_lrc);
                    let cached_entry = CachedLyrics {
                        track_id: track_id.to_string(),
                        source: "embedded".to_string(),
                        status: "found".to_string(),
                        plain_lyrics: Some(parsed.plain_text.clone()),
                        raw_synced_lyrics: Some(raw_lrc),
                        is_instrumental: false,
                        lrclib_id: None,
                        track_signature: signature,
                    };
                    let _ = save_cached_lyrics(&self.pool, &cached_entry).await;

                    return Ok(LyricsResponseDTO {
                        track_id: track_id.to_string(),
                        status: "synced".to_string(),
                        source: Some("embedded".to_string()),
                        plain_lyrics: Some(parsed.plain_text),
                        synced_lines: Some(parsed.lines),
                        is_instrumental: false,
                    });
                }
                EmbeddedLyricsResult::Both { raw_lrc, plain } => {
                    let parsed = parse_lrc(&raw_lrc);
                    let cached_entry = CachedLyrics {
                        track_id: track_id.to_string(),
                        source: "embedded".to_string(),
                        status: "found".to_string(),
                        plain_lyrics: Some(plain.clone()),
                        raw_synced_lyrics: Some(raw_lrc),
                        is_instrumental: false,
                        lrclib_id: None,
                        track_signature: signature,
                    };
                    let _ = save_cached_lyrics(&self.pool, &cached_entry).await;

                    return Ok(LyricsResponseDTO {
                        track_id: track_id.to_string(),
                        status: "synced".to_string(),
                        source: Some("embedded".to_string()),
                        plain_lyrics: Some(plain),
                        synced_lines: Some(parsed.lines),
                        is_instrumental: false,
                    });
                }
                EmbeddedLyricsResult::Plain { text } => {
                    let cached_entry = CachedLyrics {
                        track_id: track_id.to_string(),
                        source: "embedded".to_string(),
                        status: "found".to_string(),
                        plain_lyrics: Some(text.clone()),
                        raw_synced_lyrics: None,
                        is_instrumental: false,
                        lrclib_id: None,
                        track_signature: signature,
                    };
                    let _ = save_cached_lyrics(&self.pool, &cached_entry).await;

                    return Ok(LyricsResponseDTO {
                        track_id: track_id.to_string(),
                        status: "plain".to_string(),
                        source: Some("embedded".to_string()),
                        plain_lyrics: Some(text),
                        synced_lines: None,
                        is_instrumental: false,
                    });
                }
                EmbeddedLyricsResult::None => {}
            }
        }

        // 5. Query LRCLIB (Priority 2)
        let duration_s = if duration_ms > 0 {
            Some(duration_ms / 1000)
        } else {
            None
        };

        let lrclib_res = self
            .lrclib_client
            .get_lyrics(
                &title,
                &artist,
                if album.is_empty() { None } else { Some(&album) },
                duration_s,
            )
            .await;

        match lrclib_res {
            Ok(LrclibResult::Found {
                lrclib_id,
                plain_lyrics,
                synced_lyrics,
                is_instrumental: _,
            }) => {
                if let Some(raw_lrc) = synced_lyrics {
                    let parsed = parse_lrc(&raw_lrc);
                    let final_plain = plain_lyrics.unwrap_or(parsed.plain_text.clone());

                    let cached_entry = CachedLyrics {
                        track_id: track_id.to_string(),
                        source: "lrclib".to_string(),
                        status: "found".to_string(),
                        plain_lyrics: Some(final_plain.clone()),
                        raw_synced_lyrics: Some(raw_lrc),
                        is_instrumental: false,
                        lrclib_id,
                        track_signature: signature,
                    };
                    let _ = save_cached_lyrics(&self.pool, &cached_entry).await;

                    Ok(LyricsResponseDTO {
                        track_id: track_id.to_string(),
                        status: "synced".to_string(),
                        source: Some("lrclib".to_string()),
                        plain_lyrics: Some(final_plain),
                        synced_lines: Some(parsed.lines),
                        is_instrumental: false,
                    })
                } else if let Some(plain) = plain_lyrics {
                    let cached_entry = CachedLyrics {
                        track_id: track_id.to_string(),
                        source: "lrclib".to_string(),
                        status: "found".to_string(),
                        plain_lyrics: Some(plain.clone()),
                        raw_synced_lyrics: None,
                        is_instrumental: false,
                        lrclib_id,
                        track_signature: signature,
                    };
                    let _ = save_cached_lyrics(&self.pool, &cached_entry).await;

                    Ok(LyricsResponseDTO {
                        track_id: track_id.to_string(),
                        status: "plain".to_string(),
                        source: Some("lrclib".to_string()),
                        plain_lyrics: Some(plain),
                        synced_lines: None,
                        is_instrumental: false,
                    })
                } else {
                    let cached_entry = CachedLyrics {
                        track_id: track_id.to_string(),
                        source: "none".to_string(),
                        status: "not_found".to_string(),
                        plain_lyrics: None,
                        raw_synced_lyrics: None,
                        is_instrumental: false,
                        lrclib_id,
                        track_signature: signature,
                    };
                    let _ = save_cached_lyrics(&self.pool, &cached_entry).await;

                    Ok(LyricsResponseDTO {
                        track_id: track_id.to_string(),
                        status: "not_found".to_string(),
                        source: None,
                        plain_lyrics: None,
                        synced_lines: None,
                        is_instrumental: false,
                    })
                }
            }
            Ok(LrclibResult::Instrumental { lrclib_id }) => {
                let cached_entry = CachedLyrics {
                    track_id: track_id.to_string(),
                    source: "lrclib".to_string(),
                    status: "instrumental".to_string(),
                    plain_lyrics: None,
                    raw_synced_lyrics: None,
                    is_instrumental: true,
                    lrclib_id,
                    track_signature: signature,
                };
                let _ = save_cached_lyrics(&self.pool, &cached_entry).await;

                Ok(LyricsResponseDTO {
                    track_id: track_id.to_string(),
                    status: "instrumental".to_string(),
                    source: Some("lrclib".to_string()),
                    plain_lyrics: None,
                    synced_lines: None,
                    is_instrumental: true,
                })
            }
            Ok(LrclibResult::Mismatch { reason }) => {
                // The server returned a best-match candidate whose metadata
                // contradicts this recording. Never cache it as a positive
                // result and never present it as belonging to this track.
                // It is also not cached as a negative result: metadata may be
                // corrected locally, and a mismatch is not a definitive
                // "no lyrics exist" answer.
                let _ = reason;
                Ok(LyricsResponseDTO {
                    track_id: track_id.to_string(),
                    status: "not_found".to_string(),
                    source: None,
                    plain_lyrics: None,
                    synced_lines: None,
                    is_instrumental: false,
                })
            }
            Ok(LrclibResult::NotFound) => {
                let cached_entry = CachedLyrics {
                    track_id: track_id.to_string(),
                    source: "none".to_string(),
                    status: "not_found".to_string(),
                    plain_lyrics: None,
                    raw_synced_lyrics: None,
                    is_instrumental: false,
                    lrclib_id: None,
                    track_signature: signature,
                };
                let _ = save_cached_lyrics(&self.pool, &cached_entry).await;

                Ok(LyricsResponseDTO {
                    track_id: track_id.to_string(),
                    status: "not_found".to_string(),
                    source: None,
                    plain_lyrics: None,
                    synced_lines: None,
                    is_instrumental: false,
                })
            }
            Err(err) => {
                // Transient error: do NOT store in cache, return honest error
                Err(format!("Lyrics lookup error: {}", err))
            }
        }
    }
}

fn system_now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const TEST_NOW: i64 = 1_800_000_000;
    const DAY_SECS: i64 = 24 * 60 * 60;

    async fn create_test_db() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn insert_test_track(
        pool: &SqlitePool,
        id: &str,
        title: &str,
        artist: &str,
        album: &str,
        duration_ms: i64,
        file_uri: Option<&str>,
    ) {
        sqlx::query(
            r#"
            INSERT INTO tracks (id, source_kind, title, artist, album, duration_ms, file_uri)
            VALUES (?, 'local', ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(title)
        .bind(artist)
        .bind(album)
        .bind(duration_ms)
        .bind(file_uri)
        .execute(pool)
        .await
        .unwrap();
    }

    fn track_signature(
        title: &str,
        artist: &str,
        album: &str,
        duration_ms: i64,
        file_uri: Option<&str>,
    ) -> String {
        compute_track_signature(title, artist, album, duration_ms, file_uri)
    }

    fn fixed_clock() -> Arc<dyn Fn() -> i64 + Send + Sync> {
        Arc::new(|| TEST_NOW)
    }

    async fn set_cache_updated_at(pool: &SqlitePool, track_id: &str, unix: i64) {
        sqlx::query(
            "UPDATE lyrics_cache SET updated_at = datetime(?, 'unixepoch') WHERE track_id = ?",
        )
        .bind(unix)
        .bind(track_id)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn seed_cache(
        pool: &SqlitePool,
        track_id: &str,
        signature: &str,
        status: &str,
        source: &str,
        is_instrumental: bool,
        plain: Option<&str>,
        synced: Option<&str>,
        updated_unix: i64,
    ) {
        let entry = CachedLyrics {
            track_id: track_id.to_string(),
            source: source.to_string(),
            status: status.to_string(),
            plain_lyrics: plain.map(str::to_string),
            raw_synced_lyrics: synced.map(str::to_string),
            is_instrumental,
            lrclib_id: None,
            track_signature: signature.to_string(),
        };
        save_cached_lyrics(pool, &entry).await.unwrap();
        set_cache_updated_at(pool, track_id, updated_unix).await;
    }

    struct MockServer {
        base_url: String,
        requests: Arc<tokio::sync::Mutex<Vec<String>>>,
        _counter: Arc<AtomicUsize>,
    }

    impl MockServer {
        async fn request_count(&self) -> usize {
            self.requests.lock().await.len()
        }
    }

    async fn spawn_mock_server<F>(handler: F) -> MockServer
    where
        F: Fn(usize, String) -> (u16, Vec<(&'static str, &'static str)>, String)
            + Send
            + Sync
            + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let requests_for_task = requests.clone();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_task = counter.clone();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let requests = requests_for_task.clone();
                let handler = handler.clone();
                let index = counter_for_task.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    loop {
                        match socket.read(&mut chunk).await {
                            Ok(0) => break,
                            Ok(n) => {
                                buf.extend_from_slice(&chunk[..n]);
                                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                            Err(_) => return,
                        }
                    }
                    let req_str = String::from_utf8_lossy(&buf).to_string();
                    requests.lock().await.push(req_str.clone());
                    let (status, headers, body) = handler(index, req_str);
                    let status_line = match status {
                        200 => "HTTP/1.1 200 OK",
                        404 => "HTTP/1.1 404 Not Found",
                        429 => "HTTP/1.1 429 Too Many Requests",
                        500 => "HTTP/1.1 500 Internal Server Error",
                        503 => "HTTP/1.1 503 Service Unavailable",
                        _ => "HTTP/1.1 400 Bad Request",
                    };
                    let mut response = format!(
                        "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n",
                        body.len()
                    );
                    for (header, value) in headers {
                        response.push_str(&format!("{header}: {value}\r\n"));
                    }
                    response.push_str("\r\n");
                    response.push_str(&body);
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        MockServer {
            base_url,
            requests,
            _counter: counter,
        }
    }

    fn found_body(track: &str, artist: &str) -> String {
        format!(
            r#"{{"id":101,"trackName":"{track}","artistName":"{artist}","instrumental":false,"plainLyrics":"Plain line","syncedLyrics":"[00:10.00]Synced line"}}"#
        )
    }

    #[tokio::test]
    async fn test_lyrics_service_lrclib_lookup_and_cache_reuse() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-1",
            "Echoes",
            "Pink Floyd",
            "Meddle",
            300000,
            None,
        )
        .await;

        let server = spawn_mock_server(|_, _| {
            (
                200,
                vec![("Content-Type", "application/json")],
                found_body("Echoes", "Pink Floyd"),
            )
        })
        .await;

        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client(pool.clone(), client);

        let res1 = service.get_lyrics("track-1").await.unwrap();
        assert_eq!(res1.status, "synced");
        assert_eq!(res1.source, Some("lrclib".to_string()));
        assert_eq!(res1.synced_lines.as_ref().unwrap().len(), 1);
        assert_eq!(res1.synced_lines.as_ref().unwrap()[0].timestamp_ms, 10000);
        assert_eq!(server.request_count().await, 1);

        let cache_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_cache WHERE track_id = 'track-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(cache_count, 1);

        let res2 = service.get_lyrics("track-1").await.unwrap();
        assert_eq!(res2.status, "synced");
        assert_eq!(
            server.request_count().await,
            1,
            "cache hit must not perform another HTTP request"
        );
    }

    #[tokio::test]
    async fn test_lyrics_service_fingerprint_invalidation() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-2",
            "Original Title",
            "Artist",
            "Album",
            200000,
            None,
        )
        .await;

        let server = spawn_mock_server(|_, req| {
            if req.contains("track_name=Retagged+Title") {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    found_body("Retagged Title", "Artist"),
                )
            } else {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    found_body("Original Title", "Artist"),
                )
            }
        })
        .await;

        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client(pool.clone(), client);

        let res1 = service.get_lyrics("track-2").await.unwrap();
        assert_eq!(res1.synced_lines.as_ref().unwrap()[0].text, "Synced line");
        assert_eq!(server.request_count().await, 1);

        sqlx::query("UPDATE tracks SET title = 'Retagged Title' WHERE id = 'track-2'")
            .execute(&pool)
            .await
            .unwrap();

        let res2 = service.get_lyrics("track-2").await.unwrap();
        assert_eq!(res2.status, "synced");
        assert_eq!(
            server.request_count().await,
            2,
            "metadata change must invalidate the cache"
        );
    }

    #[tokio::test]
    async fn test_lyrics_service_instrumental_and_not_found_negative_caching() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-inst",
            "Ambient Song",
            "Ambient",
            "Album",
            100000,
            None,
        )
        .await;
        insert_test_track(
            &pool,
            "track-missing",
            "Unknown Song",
            "Unknown",
            "Album",
            100000,
            None,
        )
        .await;

        let server = spawn_mock_server(|_, req| {
            if req.contains("track_name=Ambient+Song") {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    r#"{"id":301,"trackName":"Ambient Song","artistName":"Ambient","instrumental":true}"#
                        .to_string(),
                )
            } else {
                (404, vec![], "{}".to_string())
            }
        })
        .await;

        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client(pool.clone(), client);

        let inst_res = service.get_lyrics("track-inst").await.unwrap();
        assert_eq!(inst_res.status, "instrumental");
        assert!(inst_res.is_instrumental);

        let miss_res = service.get_lyrics("track-missing").await.unwrap();
        assert_eq!(miss_res.status, "not_found");
        assert!(!miss_res.is_instrumental);

        let inst_status: String =
            sqlx::query_scalar("SELECT status FROM lyrics_cache WHERE track_id = 'track-inst'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(inst_status, "instrumental");

        let miss_status: String =
            sqlx::query_scalar("SELECT status FROM lyrics_cache WHERE track_id = 'track-missing'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(miss_status, "not_found");

        // Both negative outcomes are reused within the TTL.
        let requests_before = server.request_count().await;
        let _ = service.get_lyrics("track-inst").await.unwrap();
        let _ = service.get_lyrics("track-missing").await.unwrap();
        assert_eq!(server.request_count().await, requests_before);
    }

    #[tokio::test]
    async fn test_transient_error_does_not_poison_cache() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-err",
            "Error Song",
            "Artist",
            "Album",
            100000,
            None,
        )
        .await;

        let server = spawn_mock_server(|_, _| (500, vec![], "Server Error".to_string())).await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client(pool.clone(), client);

        let err = service.get_lyrics("track-err").await.unwrap_err();
        assert!(err.contains("Lyrics lookup error"));

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_cache WHERE track_id = 'track-err'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_identity_mismatch_is_not_cached() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-mismatch",
            "Midnight",
            "Cavia Artist",
            "Obsidian",
            180000,
            None,
        )
        .await;

        let server = spawn_mock_server(|_, _| {
            (
                200,
                vec![("Content-Type", "application/json")],
                r#"{"id":9,"trackName":"Midnight","artistName":"Different Band","duration":180,"plainLyrics":"Wrong recording"}"#.to_string(),
            )
        })
        .await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client(pool.clone(), client);

        let res = service.get_lyrics("track-mismatch").await.unwrap();
        assert_eq!(res.status, "not_found");
        assert!(res.plain_lyrics.is_none());
        assert!(res.synced_lines.is_none());

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM lyrics_cache WHERE track_id = 'track-mismatch'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "mismatched lyrics must never enter the cache");

        let _ = service.get_lyrics("track-mismatch").await.unwrap();
        assert_eq!(
            server.request_count().await,
            2,
            "mismatches are not negatively cached either"
        );
    }

    #[tokio::test]
    async fn test_negative_cache_hit_before_expiry() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-neg",
            "Negative Song",
            "Artist",
            "Album",
            120000,
            None,
        )
        .await;
        let signature = track_signature("Negative Song", "Artist", "Album", 120000, None);
        seed_cache(
            &pool,
            "track-neg",
            &signature,
            "not_found",
            "none",
            false,
            None,
            None,
            TEST_NOW - 6 * DAY_SECS,
        )
        .await;

        let server = spawn_mock_server(|_, _| {
            (
                200,
                vec![("Content-Type", "application/json")],
                found_body("Negative Song", "Artist"),
            )
        })
        .await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client_and_clock(pool.clone(), client, fixed_clock());

        let res = service.get_lyrics("track-neg").await.unwrap();
        assert_eq!(res.status, "not_found");
        assert_eq!(
            server.request_count().await,
            0,
            "unexpired negative cache must not trigger HTTP"
        );
    }

    #[tokio::test]
    async fn test_expired_not_found_cache_is_retried() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-expired",
            "Expired Song",
            "Artist",
            "Album",
            120000,
            None,
        )
        .await;
        let signature = track_signature("Expired Song", "Artist", "Album", 120000, None);
        seed_cache(
            &pool,
            "track-expired",
            &signature,
            "not_found",
            "none",
            false,
            None,
            None,
            TEST_NOW - 8 * DAY_SECS,
        )
        .await;

        let server = spawn_mock_server(|_, _| {
            (
                200,
                vec![("Content-Type", "application/json")],
                found_body("Expired Song", "Artist"),
            )
        })
        .await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client_and_clock(pool.clone(), client, fixed_clock());

        let res = service.get_lyrics("track-expired").await.unwrap();
        assert_eq!(res.status, "synced");
        assert_eq!(server.request_count().await, 1);

        let status: String =
            sqlx::query_scalar("SELECT status FROM lyrics_cache WHERE track_id = 'track-expired'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "found", "expired negative must be replaced");
    }

    #[tokio::test]
    async fn test_expired_instrumental_cache_is_retried() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-inst-expired",
            "Instrumental Song",
            "Composer",
            "Album",
            90000,
            None,
        )
        .await;
        let signature = track_signature("Instrumental Song", "Composer", "Album", 90000, None);
        seed_cache(
            &pool,
            "track-inst-expired",
            &signature,
            "instrumental",
            "lrclib",
            true,
            None,
            None,
            TEST_NOW - 30 * DAY_SECS,
        )
        .await;

        let server = spawn_mock_server(|_, _| {
            (
                200,
                vec![("Content-Type", "application/json")],
                found_body("Instrumental Song", "Composer"),
            )
        })
        .await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client_and_clock(pool.clone(), client, fixed_clock());

        let res = service.get_lyrics("track-inst-expired").await.unwrap();
        assert_eq!(res.status, "synced");
        assert_eq!(server.request_count().await, 1);

        let status: String = sqlx::query_scalar(
            "SELECT status FROM lyrics_cache WHERE track_id = 'track-inst-expired'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "found");
    }

    #[tokio::test]
    async fn test_positive_cache_never_expires() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-old",
            "Old Song",
            "Artist",
            "Album",
            120000,
            None,
        )
        .await;
        let signature = track_signature("Old Song", "Artist", "Album", 120000, None);
        seed_cache(
            &pool,
            "track-old",
            &signature,
            "found",
            "lrclib",
            false,
            Some("Plain line"),
            Some("[00:10.00]Synced line"),
            TEST_NOW - 365 * DAY_SECS,
        )
        .await;

        let server = spawn_mock_server(|_, _| (404, vec![], "{}".to_string())).await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client_and_clock(pool.clone(), client, fixed_clock());

        let res = service.get_lyrics("track-old").await.unwrap();
        assert_eq!(res.status, "synced");
        assert_eq!(
            server.request_count().await,
            0,
            "positive lyrics must not be expired"
        );
    }

    #[tokio::test]
    async fn test_refresh_failure_preserves_valid_positive_cache() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-keep",
            "Keep Song",
            "Artist",
            "Album",
            120000,
            None,
        )
        .await;
        let signature = track_signature("Keep Song", "Artist", "Album", 120000, None);
        seed_cache(
            &pool,
            "track-keep",
            &signature,
            "found",
            "lrclib",
            false,
            Some("Plain line"),
            Some("[00:10.00]Synced line"),
            TEST_NOW - 60 * DAY_SECS,
        )
        .await;

        let server = spawn_mock_server(|_, _| (500, vec![], "Server Error".to_string())).await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client(pool.clone(), client);

        let err = service.refresh_lyrics("track-keep").await.unwrap_err();
        assert!(err.contains("Lyrics lookup error"));

        let (status, synced): (String, Option<String>) = sqlx::query_as(
            "SELECT status, raw_synced_lyrics FROM lyrics_cache WHERE track_id = 'track-keep'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "found");
        assert!(synced.is_some());

        // A normal read still serves the valid cached result.
        let res = service.get_lyrics("track-keep").await.unwrap();
        assert_eq!(res.status, "synced");
    }

    #[tokio::test]
    async fn test_transient_failure_does_not_convert_expired_negative() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-neg-err",
            "Negative Error",
            "Artist",
            "Album",
            120000,
            None,
        )
        .await;
        let signature = track_signature("Negative Error", "Artist", "Album", 120000, None);
        seed_cache(
            &pool,
            "track-neg-err",
            &signature,
            "not_found",
            "none",
            false,
            None,
            None,
            TEST_NOW - 10 * DAY_SECS,
        )
        .await;

        let server = spawn_mock_server(|_, _| {
            (503, vec![("Retry-After", "120")], "Unavailable".to_string())
        })
        .await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let service = LyricsService::with_client_and_clock(pool.clone(), client, fixed_clock());

        let err = service.get_lyrics("track-neg-err").await.unwrap_err();
        assert!(err.contains("Lyrics lookup error"));

        let status: String =
            sqlx::query_scalar("SELECT status FROM lyrics_cache WHERE track_id = 'track-neg-err'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            status, "not_found",
            "transient errors must not rewrite the cached negative outcome"
        );
    }

    #[tokio::test]
    async fn test_track_deletion_cascades_lyrics_cache() {
        let pool = create_test_db().await;
        insert_test_track(
            &pool,
            "track-delete",
            "Delete Song",
            "Artist",
            "Album",
            120000,
            None,
        )
        .await;
        let signature = track_signature("Delete Song", "Artist", "Album", 120000, None);
        seed_cache(
            &pool,
            "track-delete",
            &signature,
            "found",
            "lrclib",
            false,
            Some("Plain"),
            None,
            TEST_NOW,
        )
        .await;

        sqlx::query("DELETE FROM tracks WHERE id = 'track-delete'")
            .execute(&pool)
            .await
            .unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_cache WHERE track_id = 'track-delete'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0, "ON DELETE CASCADE must remove the cache row");
    }

    #[test]
    fn test_lyrics_response_dto_serializes_camel_case() {
        let dto = LyricsResponseDTO {
            track_id: "track-1".to_string(),
            status: "synced".to_string(),
            source: Some("lrclib".to_string()),
            plain_lyrics: Some("Line".to_string()),
            synced_lines: Some(vec![LyricsLine {
                timestamp_ms: 1000,
                text: "Line".to_string(),
            }]),
            is_instrumental: false,
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["trackId"], "track-1");
        assert_eq!(json["plainLyrics"], "Line");
        assert_eq!(json["syncedLines"][0]["timestampMs"], 1000);
        assert_eq!(json["isInstrumental"], false);
        assert!(json.get("track_id").is_none());
    }
}
