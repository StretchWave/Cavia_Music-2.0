use cavia_lib::services::library::{
    get_albums, get_all_tracks, get_artists, get_liked_tracks, get_playlists,
};
use cavia_lib::services::lyrics::parse_lrc;
use cavia_lib::services::search::search_library;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::time::Instant;

struct BenchmarkResult {
    operation: String,
    dataset_size: usize,
    iterations: usize,
    min_ms: f64,
    median_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    rows_returned: usize,
}

impl BenchmarkResult {
    fn new(operation: &str, dataset_size: usize, mut durations_ms: Vec<f64>, rows: usize) -> Self {
        durations_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = durations_ms.len();
        let min_ms = durations_ms[0];
        let max_ms = durations_ms[n - 1];
        let median_ms = if n % 2 == 1 {
            durations_ms[n / 2]
        } else {
            (durations_ms[n / 2 - 1] + durations_ms[n / 2]) / 2.0
        };
        let p95_idx = ((n as f64) * 0.95).ceil() as usize - 1;
        let p95_ms = durations_ms[p95_idx.min(n - 1)];

        Self {
            operation: operation.to_string(),
            dataset_size,
            iterations: n,
            min_ms,
            median_ms,
            p95_ms,
            max_ms,
            rows_returned: rows,
        }
    }

    fn print(&self) {
        println!(
            "  [{:<28}] N={:<6} (iters: {:<2}) | Median: {:>7.2} ms | Min: {:>7.2} ms | P95: {:>7.2} ms | Max: {:>7.2} ms | Rows: {}",
            self.operation,
            self.dataset_size,
            self.iterations,
            self.median_ms,
            self.min_ms,
            self.p95_ms,
            self.max_ms,
            self.rows_returned
        );
    }
}

async fn create_benchmark_pool(db_path: Option<&PathBuf>) -> SqlitePool {
    let options = if let Some(path) = db_path {
        SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
    } else {
        SqliteConnectOptions::new()
            .filename(":memory:")
            .foreign_keys(true)
    };

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("Failed to create SQLite connection pool");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to apply migrations");

    pool
}

async fn explain_query_plan(pool: &SqlitePool, label: &str, query: &str) {
    println!("\n--- EXPLAIN QUERY PLAN: {} ---", label);
    println!(
        "Query: {}",
        query
            .trim()
            .replace('\n', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    );
    let explain_sql = format!("EXPLAIN QUERY PLAN {query}");
    match sqlx::raw_sql(sqlx::AssertSqlSafe(explain_sql))
        .fetch_all(pool)
        .await
    {
        Ok(rows) => {
            for row in rows {
                let id: i64 = row.try_get("id").unwrap_or(0);
                let parent: i64 = row.try_get("parent").unwrap_or(0);
                let notused: i64 = row.try_get("notused").unwrap_or(0);
                let detail: String = row.try_get("detail").unwrap_or_default();
                println!(
                    "  id={:<2} parent={:<2} notused={:<2} | {}",
                    id, parent, notused, detail
                );
            }
        }
        Err(e) => {
            println!("  [Explain error: {e}]");
        }
    }
}

async fn seed_tracks(pool: &SqlitePool, count: usize) {
    println!(
        "Seeding {} synthetic tracks into SQLite benchmark database...",
        count
    );
    let start = Instant::now();

    let batch_size = 1000;
    let mut inserted = 0;

    while inserted < count {
        let chunk = (count - inserted).min(batch_size);
        let mut tx = pool
            .begin()
            .await
            .expect("Failed to begin seeding transaction");

        for i in 0..chunk {
            let track_idx = inserted + i + 1;
            let track_id = format!("track-bench-{track_idx:06}");
            let artist_idx = (track_idx % (count / 10).max(1)) + 1;
            let album_idx = (track_idx % (count / 8).max(1)) + 1;

            let artist = format!("Artist {artist_idx}");
            let album = format!("Album {album_idx}");
            let title = if track_idx % 7 == 0 {
                format!("Tokyo Rain Track {track_idx}")
            } else if track_idx % 11 == 0 {
                format!("Élégie Nocturne {track_idx}")
            } else if track_idx % 13 == 0 {
                format!("Vermilion Dawn {track_idx}")
            } else {
                format!("Synthetic Tone Track {track_idx}")
            };

            let duration_ms = 120_000 + (track_idx as i64 * 37) % 240_000;
            let release_year = 2000 + (track_idx as i64 % 25);
            let track_number = (track_idx as i64 % 12) + 1;
            let content_hash = format!("hash_{:016x}_{:016x}", track_idx, track_idx * 31);
            let file_uri = format!(
                "C:/Music/Synthetic/Artist_{artist_idx}/Album_{album_idx}/track_{track_idx:06}.flac"
            );

            sqlx::query(
                r#"
                INSERT INTO tracks (
                    id, source_kind, title, artist, album, duration_ms, file_uri,
                    content_hash, release_year, track_number, is_deleted
                ) VALUES (?, 'local', ?, ?, ?, ?, ?, ?, ?, ?, 0)
                "#,
            )
            .bind(&track_id)
            .bind(&title)
            .bind(&artist)
            .bind(&album)
            .bind(duration_ms)
            .bind(&file_uri)
            .bind(&content_hash)
            .bind(release_year)
            .bind(track_number)
            .execute(&mut *tx)
            .await
            .expect("Failed to insert synthetic track");

            // Like ~10% of tracks
            if track_idx % 10 == 0 {
                sqlx::query("INSERT INTO liked_tracks (track_id) VALUES (?)")
                    .bind(&track_id)
                    .execute(&mut *tx)
                    .await
                    .expect("Failed to insert liked track");
            }
        }

        tx.commit().await.expect("Failed to commit batch insert");
        inserted += chunk;
    }

    // Seed 5 playlists
    for p in 1..=5 {
        let pl_id = format!("pl-bench-{p}");
        let pl_name = format!("Benchmark Playlist {p}");
        let _ = sqlx::query("INSERT INTO playlists (id, name, description) VALUES (?, ?, 'Bench')")
            .bind(&pl_id)
            .bind(&pl_name)
            .execute(pool)
            .await;

        // Add 20 tracks to each playlist
        for pos in 0..20 {
            let t_idx = ((p * 20 + pos) % count) + 1;
            let t_id = format!("track-bench-{t_idx:06}");
            let _ = sqlx::query("INSERT INTO playlist_tracks (id, playlist_id, track_id, position) VALUES (?, ?, ?, ?)")
                .bind(format!("pt-{p}-{pos}"))
                .bind(&pl_id)
                .bind(&t_id)
                .bind(pos as i64)
                .execute(pool)
                .await;
        }
    }

    println!(
        "Seeding completed in {:.2} s ({:.0} tracks/sec)\n",
        start.elapsed().as_secs_f64(),
        count as f64 / start.elapsed().as_secs_f64()
    );
}

async fn benchmark_queries(
    pool: &SqlitePool,
    count: usize,
    iterations: usize,
) -> Vec<BenchmarkResult> {
    let mut results = Vec::new();

    // 1. get_all_tracks
    {
        // Warmup
        let _ = get_all_tracks(pool).await;
        let mut times = Vec::with_capacity(iterations);
        let mut rows = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let res = get_all_tracks(pool).await.expect("get_all_tracks failed");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            rows = res.len();
        }
        results.push(BenchmarkResult::new("get_all_tracks", count, times, rows));
    }

    // 2. get_albums
    {
        let _ = get_albums(pool).await;
        let mut times = Vec::with_capacity(iterations);
        let mut rows = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let res = get_albums(pool).await.expect("get_albums failed");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            rows = res.len();
        }
        results.push(BenchmarkResult::new("get_albums", count, times, rows));
    }

    // 3. get_artists
    {
        let _ = get_artists(pool).await;
        let mut times = Vec::with_capacity(iterations);
        let mut rows = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let res = get_artists(pool).await.expect("get_artists failed");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            rows = res.len();
        }
        results.push(BenchmarkResult::new("get_artists", count, times, rows));
    }

    // 4. get_liked_tracks
    {
        let _ = get_liked_tracks(pool).await;
        let mut times = Vec::with_capacity(iterations);
        let mut rows = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let res = get_liked_tracks(pool)
                .await
                .expect("get_liked_tracks failed");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            rows = res.len();
        }
        results.push(BenchmarkResult::new("get_liked_tracks", count, times, rows));
    }

    // 5. get_playlists
    {
        let _ = get_playlists(pool).await;
        let mut times = Vec::with_capacity(iterations);
        let mut rows = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let res = get_playlists(pool).await.expect("get_playlists failed");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            rows = res.len();
        }
        results.push(BenchmarkResult::new("get_playlists", count, times, rows));
    }

    // 6. search_library (FTS5 query: "Tokyo")
    {
        let _ = search_library(pool, "Tokyo", "all").await;
        let mut times = Vec::with_capacity(iterations);
        let mut rows = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let res = search_library(pool, "Tokyo", "all")
                .await
                .expect("search_library failed");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            rows = res.total_count;
        }
        results.push(BenchmarkResult::new(
            "search_fts5 (Tokyo)",
            count,
            times,
            rows,
        ));
    }

    // 7. search_library (FTS5 query: "Vermilion Dawn")
    {
        let _ = search_library(pool, "Vermilion Dawn", "all").await;
        let mut times = Vec::with_capacity(iterations);
        let mut rows = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let res = search_library(pool, "Vermilion Dawn", "all")
                .await
                .expect("search_library failed");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            rows = res.total_count;
        }
        results.push(BenchmarkResult::new(
            "search_fts5 (Vermilion Dawn)",
            count,
            times,
            rows,
        ));
    }

    // 8. search_library (short query fallback: "To")
    {
        let _ = search_library(pool, "To", "all").await;
        let mut times = Vec::with_capacity(iterations);
        let mut rows = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let res = search_library(pool, "To", "all")
                .await
                .expect("search_library failed");
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            rows = res.total_count;
        }
        results.push(BenchmarkResult::new(
            "search_short_like (To)",
            count,
            times,
            rows,
        ));
    }

    // 9. Scanner duplicate check by content_hash
    {
        let test_hash = format!("hash_{:016x}_{:016x}", count / 2, (count / 2) * 31);
        let mut times = Vec::with_capacity(iterations);
        let mut found = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let row = sqlx::query(
                "SELECT id, file_uri, is_deleted FROM tracks WHERE content_hash = ? LIMIT 1",
            )
            .bind(&test_hash)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            if row.is_some() {
                found = 1;
            }
        }
        results.push(BenchmarkResult::new(
            "scanner_lookup (content_hash)",
            count,
            times,
            found,
        ));
    }

    // 10. Scanner duplicate check by file_uri
    {
        let test_uri = format!(
            "C:/Music/Synthetic/Artist_1/Album_1/track_{:06}.flac",
            count / 3
        );
        let mut times = Vec::with_capacity(iterations);
        let mut found = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let row = sqlx::query("SELECT id FROM tracks WHERE file_uri = ? LIMIT 1")
                .bind(&test_uri)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            if row.is_some() {
                found = 1;
            }
        }
        results.push(BenchmarkResult::new(
            "scanner_lookup (file_uri)",
            count,
            times,
            found,
        ));
    }

    results
}

fn benchmark_lyrics_parsing(iterations: usize) -> Vec<BenchmarkResult> {
    let mut results = Vec::new();

    // Standard 60-line LRC
    let mut standard_lrc = String::new();
    for i in 0..60 {
        let mm = i / 60;
        let ss = i % 60;
        standard_lrc.push_str(&format!(
            "[{mm:02}:{ss:02}.50] Line {i}: The autumn wind whispers in obsidian dark.\n"
        ));
    }

    // 500-line large LRC
    let mut large_lrc = String::new();
    for i in 0..500 {
        let mm = i / 60;
        let ss = i % 60;
        large_lrc.push_str(&format!("[{mm:02}:{ss:02}.25] Extended verse {i} in the grand symphony of ivory and vermilion.\n"));
    }

    // 2000-line extreme LRC
    let mut extreme_lrc = String::new();
    for i in 0..2000 {
        let mm = i / 60;
        let ss = i % 60;
        extreme_lrc.push_str(&format!("[{mm:02}:{ss:02}.10] Extreme opera line {i} with high resolution timing and metadata.\n"));
    }

    // Benchmark standard
    {
        let mut times = Vec::with_capacity(iterations);
        let mut lines_count = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let parsed = parse_lrc(&standard_lrc);
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            lines_count = parsed.lines.len();
        }
        results.push(BenchmarkResult::new(
            "parse_lrc (standard 60 lines)",
            60,
            times,
            lines_count,
        ));
    }

    // Benchmark large
    {
        let mut times = Vec::with_capacity(iterations);
        let mut lines_count = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let parsed = parse_lrc(&large_lrc);
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            lines_count = parsed.lines.len();
        }
        results.push(BenchmarkResult::new(
            "parse_lrc (large 500 lines)",
            500,
            times,
            lines_count,
        ));
    }

    // Benchmark extreme
    {
        let mut times = Vec::with_capacity(iterations);
        let mut lines_count = 0;
        for _ in 0..iterations {
            let start = Instant::now();
            let parsed = parse_lrc(&extreme_lrc);
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            lines_count = parsed.lines.len();
        }
        results.push(BenchmarkResult::new(
            "parse_lrc (extreme 2000 lines)",
            2000,
            times,
            lines_count,
        ));
    }

    results
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");
    rt.block_on(async_main());
}

async fn async_main() {
    println!("================================================================================");
    println!("      CAVIA MUSIC 2.0 — BENCHMARK SUITE & PERFORMANCE PROFILER");
    println!("================================================================================");

    let is_quick = std::env::args().any(|a| a == "--quick");
    let target_sizes = if is_quick {
        vec![100, 1_000, 10_000]
    } else {
        vec![100, 1_000, 10_000, 50_000]
    };

    println!("Dataset sizes to benchmark: {:?}", target_sizes);

    for &size in &target_sizes {
        println!(
            "\n--------------------------------------------------------------------------------"
        );
        println!(">>> BENCHMARKING DATASET SIZE: N = {} TRACKS", size);
        println!(
            "--------------------------------------------------------------------------------"
        );

        let temp_dir = std::env::temp_dir().join(format!("cavia_bench_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_file = temp_dir.join(format!("bench_{size}.sqlite3"));

        let pool = create_benchmark_pool(Some(&db_file)).await;

        seed_tracks(&pool, size).await;

        if size == 10_000 {
            // Print EXPLAIN QUERY PLAN for core queries
            explain_query_plan(
                &pool,
                "get_all_tracks (Active tracks sorted by title)",
                "SELECT t.id, t.title, t.artist, t.album, t.album_artist, t.duration_ms, t.artwork_uri, t.source_kind, t.provider_id, t.file_uri, t.release_year, t.track_number, t.disc_number, t.genre, t.play_count, t.date_added, CASE WHEN l.track_id IS NOT NULL THEN 1 ELSE 0 END AS is_liked FROM tracks t LEFT JOIN liked_tracks l ON t.id = l.track_id WHERE t.is_deleted = 0 ORDER BY t.title COLLATE NOCASE ASC",
            ).await;

            explain_query_plan(
                &pool,
                "get_albums (Group by album, artist)",
                "SELECT COALESCE(NULLIF(album, ''), 'Unknown Album') AS title, COALESCE(NULLIF(album_artist, ''), NULLIF(artist, ''), 'Unknown Artist') AS artist, MIN(release_year) AS release_year, COUNT(*) AS track_count, MAX(artwork_uri) AS artwork_uri FROM tracks WHERE is_deleted = 0 GROUP BY title, artist ORDER BY title COLLATE NOCASE ASC",
            ).await;

            explain_query_plan(
                &pool,
                "get_artists (Group by artist)",
                "SELECT COALESCE(NULLIF(artist, ''), 'Unknown Artist') AS name, COUNT(*) AS track_count, COUNT(DISTINCT NULLIF(album, '')) AS album_count, MAX(artwork_uri) AS artwork_uri FROM tracks WHERE is_deleted = 0 GROUP BY name ORDER BY name COLLATE NOCASE ASC",
            ).await;

            explain_query_plan(
                &pool,
                "scanner_lookup by content_hash",
                "SELECT id, file_uri, is_deleted FROM tracks WHERE content_hash = 'test' LIMIT 1",
            )
            .await;

            explain_query_plan(
                &pool,
                "scanner_lookup by file_uri",
                "SELECT id FROM tracks WHERE file_uri = 'test' LIMIT 1",
            )
            .await;

            explain_query_plan(
                &pool,
                "search_fts5 (FTS5 join with tracks)",
                "SELECT t.id, t.title, t.artist, t.album FROM tracks t JOIN tracks_fts f ON t.rowid = f.rowid WHERE t.is_deleted = 0 AND f.tracks_fts MATCH 'Tokyo' ORDER BY t.title COLLATE NOCASE ASC LIMIT 20",
            ).await;
            println!();
        }

        let iterations = if size >= 50_000 { 10 } else { 20 };
        println!("Running {} iterations per query...", iterations);
        let results = benchmark_queries(&pool, size, iterations).await;

        for res in results {
            res.print();
        }

        drop(pool);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    println!("\n--------------------------------------------------------------------------------");
    println!(">>> BENCHMARKING LYRICS PARSING");
    println!("--------------------------------------------------------------------------------");
    let lyrics_results = benchmark_lyrics_parsing(100);
    for res in lyrics_results {
        res.print();
    }

    println!("\n================================================================================");
    println!("               BENCHMARK EXECUTION COMPLETE");
    println!("================================================================================");
}
