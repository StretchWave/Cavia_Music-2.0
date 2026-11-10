use sqlx::{Row, SqlitePool};

use crate::domain::{Album, Artist, Playlist, SearchResultsDTO, Track};

/// Escapes wildcard characters (% and _) and backslashes for parameterized LIKE queries.
pub fn escape_like(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len() + 4);
    for ch in input.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '%' => escaped.push_str("\\%"),
            '_' => escaped.push_str("\\_"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Safely constructs an FTS5 trigram query from raw user input.
/// Wraps phrases and individual terms in double quotes, escaping embedded quotes as `""`.
/// Combines the full phrase with individual token conjunction for cross-column matches.
pub fn build_fts5_query(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let full_phrase = format!("\"{}\"", trimmed.replace('"', "\"\""));

    if tokens.len() == 1 {
        Some(full_phrase)
    } else {
        let token_conj = tokens
            .iter()
            .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" ");

        Some(format!("({full_phrase}) OR ({token_conj})"))
    }
}

/// Bounded limits per category
struct CategoryLimits {
    tracks: i64,
    albums: i64,
    artists: i64,
    playlists: i64,
}

impl CategoryLimits {
    fn for_category(cat: &str) -> Self {
        match cat {
            "tracks" => Self {
                tracks: 50,
                albums: 0,
                artists: 0,
                playlists: 0,
            },
            "albums" => Self {
                tracks: 0,
                albums: 50,
                artists: 0,
                playlists: 0,
            },
            "artists" => Self {
                tracks: 0,
                albums: 0,
                artists: 50,
                playlists: 0,
            },
            "playlists" => Self {
                tracks: 0,
                albums: 0,
                artists: 0,
                playlists: 50,
            },
            _ => Self {
                // "all"
                tracks: 20,
                albums: 10,
                artists: 10,
                playlists: 10,
            },
        }
    }
}

/// Primary library search entrypoint.
/// Queries SQLite for matching tracks, albums, artists, and playlists based on query and category.
pub async fn search_library(
    pool: &SqlitePool,
    query: &str,
    category: &str,
) -> Result<SearchResultsDTO, String> {
    let trimmed_query = query.trim();
    let norm_category = category.trim().to_lowercase();
    let cat_ref = match norm_category.as_str() {
        "tracks" | "albums" | "artists" | "playlists" => norm_category.as_str(),
        _ => "all",
    };

    if trimmed_query.is_empty() {
        return Ok(SearchResultsDTO {
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            playlists: Vec::new(),
            query: String::new(),
            category: cat_ref.to_string(),
            total_count: 0,
        });
    }

    let limits = CategoryLimits::for_category(cat_ref);
    let char_count = trimmed_query.chars().count();

    let (tracks, albums, artists, playlists) = if char_count < 3 {
        search_like_fallback(pool, trimmed_query, &limits).await?
    } else {
        search_fts5(pool, trimmed_query, &limits).await?
    };

    let total_count = tracks.len() + albums.len() + artists.len() + playlists.len();

    Ok(SearchResultsDTO {
        tracks,
        albums,
        artists,
        playlists,
        query: trimmed_query.to_string(),
        category: cat_ref.to_string(),
        total_count,
    })
}

/// Fallback for short queries (< 3 characters) using parameterized LIKE with bounded limits.
async fn search_like_fallback(
    pool: &SqlitePool,
    query: &str,
    limits: &CategoryLimits,
) -> Result<(Vec<Track>, Vec<Album>, Vec<Artist>, Vec<Playlist>), String> {
    let like_pattern = format!("%{}%", escape_like(query));

    let tracks = if limits.tracks > 0 {
        let rows = sqlx::query(
            r#"
            SELECT
                t.id, t.title, t.artist, t.album, t.album_artist, t.duration_ms,
                t.artwork_uri, t.source_kind, t.provider_id, t.file_uri,
                t.release_year, t.track_number, t.disc_number, t.genre,
                t.play_count, t.date_added,
                CASE WHEN l.track_id IS NOT NULL THEN 1 ELSE 0 END AS is_liked
            FROM tracks t
            LEFT JOIN liked_tracks l ON t.id = l.track_id
            WHERE t.is_deleted = 0
              AND (
                  t.title LIKE ?1 ESCAPE '\'
                  OR t.artist LIKE ?1 ESCAPE '\'
                  OR t.album LIKE ?1 ESCAPE '\'
                  OR COALESCE(t.album_artist, '') LIKE ?1 ESCAPE '\'
                  OR COALESCE(t.genre, '') LIKE ?1 ESCAPE '\'
              )
            ORDER BY t.title COLLATE NOCASE ASC, t.artist COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )
        .bind(&like_pattern)
        .bind(limits.tracks)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search tracks (short query): {e}"))?;

        map_tracks(rows)
    } else {
        Vec::new()
    };

    let albums = if limits.albums > 0 {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(NULLIF(t.album, ''), 'Unknown Album') AS title,
                COALESCE(NULLIF(t.album_artist, ''), NULLIF(t.artist, ''), 'Unknown Artist') AS artist,
                MIN(t.release_year) AS release_year,
                COUNT(*) AS track_count,
                MAX(t.artwork_uri) AS artwork_uri
            FROM tracks t
            WHERE t.is_deleted = 0
              AND (
                  t.album LIKE ?1 ESCAPE '\'
                  OR t.artist LIKE ?1 ESCAPE '\'
                  OR COALESCE(t.album_artist, '') LIKE ?1 ESCAPE '\'
              )
            GROUP BY t.album, COALESCE(NULLIF(t.album_artist, ''), NULLIF(t.artist, ''))
            ORDER BY t.album COLLATE NOCASE ASC, t.artist COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )
        .bind(&like_pattern)
        .bind(limits.albums)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search albums (short query): {e}"))?;

        map_albums(rows)
    } else {
        Vec::new()
    };

    let artists = if limits.artists > 0 {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(NULLIF(t.artist, ''), 'Unknown Artist') AS name,
                COUNT(*) AS track_count,
                COUNT(DISTINCT NULLIF(t.album, '')) AS album_count,
                MAX(t.artwork_uri) AS artwork_uri
            FROM tracks t
            WHERE t.is_deleted = 0
              AND (
                  t.artist LIKE ?1 ESCAPE '\'
                  OR COALESCE(t.album_artist, '') LIKE ?1 ESCAPE '\'
              )
            GROUP BY t.artist
            ORDER BY t.artist COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )
        .bind(&like_pattern)
        .bind(limits.artists)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search artists (short query): {e}"))?;

        map_artists(rows)
    } else {
        Vec::new()
    };

    let playlists = if limits.playlists > 0 {
        let rows = sqlx::query(
            r#"
            SELECT
                p.id,
                p.name,
                p.description,
                p.artwork_uri,
                p.source_provider,
                p.created_at,
                (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS track_count
            FROM playlists p
            WHERE p.is_deleted = 0
              AND (
                  p.name LIKE ?1 ESCAPE '\'
                  OR p.description LIKE ?1 ESCAPE '\'
              )
            ORDER BY p.name COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )
        .bind(&like_pattern)
        .bind(limits.playlists)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search playlists (short query): {e}"))?;

        map_playlists(rows)
    } else {
        Vec::new()
    };

    Ok((tracks, albums, artists, playlists))
}

/// FTS5 trigram search for queries of 3 or more Unicode characters.
async fn search_fts5(
    pool: &SqlitePool,
    query: &str,
    limits: &CategoryLimits,
) -> Result<(Vec<Track>, Vec<Album>, Vec<Artist>, Vec<Playlist>), String> {
    let fts_query = match build_fts5_query(query) {
        Some(q) => q,
        None => return Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new())),
    };

    let tracks = if limits.tracks > 0 {
        let rows = sqlx::query(
            r#"
            SELECT
                t.id, t.title, t.artist, t.album, t.album_artist, t.duration_ms,
                t.artwork_uri, t.source_kind, t.provider_id, t.file_uri,
                t.release_year, t.track_number, t.disc_number, t.genre,
                t.play_count, t.date_added,
                CASE WHEN l.track_id IS NOT NULL THEN 1 ELSE 0 END AS is_liked
            FROM tracks t
            JOIN tracks_fts f ON t.rowid = f.rowid
            LEFT JOIN liked_tracks l ON t.id = l.track_id
            WHERE t.is_deleted = 0
              AND f.tracks_fts MATCH ?1
            ORDER BY t.title COLLATE NOCASE ASC, t.artist COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )
        .bind(&fts_query)
        .bind(limits.tracks)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search tracks (FTS5): {e}"))?;

        map_tracks(rows)
    } else {
        Vec::new()
    };

    let albums = if limits.albums > 0 {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(NULLIF(t.album, ''), 'Unknown Album') AS title,
                COALESCE(NULLIF(t.album_artist, ''), NULLIF(t.artist, ''), 'Unknown Artist') AS artist,
                MIN(t.release_year) AS release_year,
                COUNT(*) AS track_count,
                MAX(t.artwork_uri) AS artwork_uri
            FROM tracks t
            JOIN tracks_fts f ON t.rowid = f.rowid
            WHERE t.is_deleted = 0
              AND f.tracks_fts MATCH ?1
            GROUP BY t.album, COALESCE(NULLIF(t.album_artist, ''), NULLIF(t.artist, ''))
            ORDER BY t.album COLLATE NOCASE ASC, t.artist COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )
        .bind(&fts_query)
        .bind(limits.albums)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search albums (FTS5): {e}"))?;

        map_albums(rows)
    } else {
        Vec::new()
    };

    let artists = if limits.artists > 0 {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(NULLIF(t.artist, ''), 'Unknown Artist') AS name,
                COUNT(*) AS track_count,
                COUNT(DISTINCT NULLIF(t.album, '')) AS album_count,
                MAX(t.artwork_uri) AS artwork_uri
            FROM tracks t
            JOIN tracks_fts f ON t.rowid = f.rowid
            WHERE t.is_deleted = 0
              AND f.tracks_fts MATCH ?1
            GROUP BY t.artist
            ORDER BY t.artist COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )
        .bind(&fts_query)
        .bind(limits.artists)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search artists (FTS5): {e}"))?;

        map_artists(rows)
    } else {
        Vec::new()
    };

    let playlists = if limits.playlists > 0 {
        let rows = sqlx::query(
            r#"
            SELECT
                p.id,
                p.name,
                p.description,
                p.artwork_uri,
                p.source_provider,
                p.created_at,
                (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS track_count
            FROM playlists p
            JOIN playlists_fts f ON p.rowid = f.rowid
            WHERE p.is_deleted = 0
              AND f.playlists_fts MATCH ?1
            ORDER BY p.name COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )
        .bind(&fts_query)
        .bind(limits.playlists)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search playlists (FTS5): {e}"))?;

        map_playlists(rows)
    } else {
        Vec::new()
    };

    Ok((tracks, albums, artists, playlists))
}

fn map_tracks(rows: Vec<sqlx::sqlite::SqliteRow>) -> Vec<Track> {
    rows.into_iter()
        .map(|row| Track {
            id: row.get("id"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            album_artist: row.get("album_artist"),
            duration_ms: row.get("duration_ms"),
            artwork_uri: row.get("artwork_uri"),
            source_kind: row.get("source_kind"),
            provider_id: row.get("provider_id"),
            file_uri: row.get("file_uri"),
            release_year: row.get("release_year"),
            track_number: row.get("track_number"),
            disc_number: row.get("disc_number"),
            genre: row.get("genre"),
            is_liked: row.get::<i64, _>("is_liked") == 1,
            play_count: row.get("play_count"),
            date_added: row.get("date_added"),
        })
        .collect()
}

fn map_albums(rows: Vec<sqlx::sqlite::SqliteRow>) -> Vec<Album> {
    rows.into_iter()
        .map(|row| {
            let title: String = row.get("title");
            let artist: String = row.get("artist");
            let id = format!("{title}_{artist}");
            Album {
                id,
                title,
                artist,
                year: row.get("release_year"),
                track_count: row.get("track_count"),
                artwork_uri: row.get("artwork_uri"),
            }
        })
        .collect()
}

fn map_artists(rows: Vec<sqlx::sqlite::SqliteRow>) -> Vec<Artist> {
    rows.into_iter()
        .map(|row| {
            let name: String = row.get("name");
            let id = name.clone();
            Artist {
                id,
                name,
                track_count: row.get("track_count"),
                album_count: row.get("album_count"),
                artwork_uri: row.get("artwork_uri"),
            }
        })
        .collect()
}

fn map_playlists(rows: Vec<sqlx::sqlite::SqliteRow>) -> Vec<Playlist> {
    rows.into_iter()
        .map(|row| Playlist {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            artwork_uri: row.get("artwork_uri"),
            source_provider: row.get("source_provider"),
            track_count: row.get("track_count"),
            created_at: row.get("created_at"),
            tracks: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_search_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn insert_track(
        pool: &SqlitePool,
        id: &str,
        title: &str,
        artist: &str,
        album: &str,
        genre: Option<&str>,
        is_deleted: bool,
    ) {
        sqlx::query(
            r#"
            INSERT INTO tracks (id, source_kind, title, artist, album, genre, is_deleted)
            VALUES (?1, 'local', ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(id)
        .bind(title)
        .bind(artist)
        .bind(album)
        .bind(genre)
        .bind(if is_deleted { 1 } else { 0 })
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_playlist(
        pool: &SqlitePool,
        id: &str,
        name: &str,
        description: &str,
        is_deleted: bool,
    ) {
        sqlx::query(
            r#"
            INSERT INTO playlists (id, name, description, is_deleted)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(if is_deleted { 1 } else { 0 })
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_empty_and_whitespace_queries() {
        let pool = create_search_test_db().await;
        insert_track(
            &pool,
            "t1",
            "Bohemian Rhapsody",
            "Queen",
            "A Night at the Opera",
            None,
            false,
        )
        .await;

        let res_empty = search_library(&pool, "", "all").await.unwrap();
        assert_eq!(res_empty.total_count, 0);
        assert_eq!(res_empty.tracks.len(), 0);

        let res_ws = search_library(&pool, "   \t\n  ", "all").await.unwrap();
        assert_eq!(res_ws.total_count, 0);
    }

    #[tokio::test]
    async fn test_short_query_fallback_1_and_2_chars() {
        let pool = create_search_test_db().await;
        insert_track(&pool, "t1", "Track 1", "Artist A", "Album X", None, false).await;
        insert_track(&pool, "t2", "Song 2", "Artist B", "Album Y", None, false).await;

        // 1 char query
        let res_1 = search_library(&pool, "A", "all").await.unwrap();
        assert!(res_1.total_count > 0, "1-char query failed");
        assert!(res_1.tracks.iter().any(|t| t.artist == "Artist A"));

        // 2 char query
        let res_2 = search_library(&pool, "So", "tracks").await.unwrap();
        assert_eq!(res_2.tracks.len(), 1);
        assert_eq!(res_2.tracks[0].title, "Song 2");
    }

    #[tokio::test]
    async fn test_fts5_3_char_and_longer_queries() {
        let pool = create_search_test_db().await;
        insert_track(
            &pool,
            "t1",
            "Stairway to Heaven",
            "Led Zeppelin",
            "Led Zeppelin IV",
            None,
            false,
        )
        .await;
        insert_track(
            &pool,
            "t2",
            "Kashmir",
            "Led Zeppelin",
            "Physical Graffiti",
            None,
            false,
        )
        .await;

        // Substring query (3 chars)
        let res_3 = search_library(&pool, "way", "tracks").await.unwrap();
        assert_eq!(res_3.tracks.len(), 1);
        assert_eq!(res_3.tracks[0].title, "Stairway to Heaven");

        // Longer query case-insensitive
        let res_artist = search_library(&pool, "zeppelin", "artists").await.unwrap();
        assert_eq!(res_artist.artists.len(), 1);
        assert_eq!(res_artist.artists[0].name, "Led Zeppelin");
    }

    #[tokio::test]
    async fn test_wildcard_and_special_character_escaping() {
        let pool = create_search_test_db().await;
        insert_track(
            &pool,
            "t1",
            "100% Pure Rock",
            "AC/DC",
            "Live (1992)",
            None,
            false,
        )
        .await;
        insert_track(
            &pool,
            "t2",
            "Under_Score_Track",
            "Special & Punctuation",
            "Album \"Quote\"",
            None,
            false,
        )
        .await;

        // Search for % wildcard character
        let res_percent = search_library(&pool, "100%", "tracks").await.unwrap();
        assert_eq!(res_percent.tracks.len(), 1);
        assert_eq!(res_percent.tracks[0].title, "100% Pure Rock");

        // Search for _ wildcard character
        let res_underscore = search_library(&pool, "_Score_", "tracks").await.unwrap();
        assert_eq!(res_underscore.tracks.len(), 1);
        assert_eq!(res_underscore.tracks[0].title, "Under_Score_Track");

        // Search for quotes and parentheses
        let res_quote = search_library(&pool, "\"Quote\"", "albums").await.unwrap();
        assert_eq!(res_quote.albums.len(), 1);
        assert_eq!(res_quote.albums[0].title, "Album \"Quote\"");

        // Search for slash and ampersand
        let res_slash = search_library(&pool, "AC/DC", "artists").await.unwrap();
        assert_eq!(res_slash.artists.len(), 1);
        assert_eq!(res_slash.artists[0].name, "AC/DC");
    }

    #[tokio::test]
    async fn test_unicode_search() {
        let pool = create_search_test_db().await;
        insert_track(
            &pool,
            "t1",
            "夜に駆ける",
            "YOASOBI",
            "THE BOOK",
            None,
            false,
        )
        .await;
        insert_track(
            &pool,
            "t2",
            "Café del Mar",
            "Energy 52",
            "Café Sampler",
            None,
            false,
        )
        .await;

        // Japanese kanji search
        let res_jp = search_library(&pool, "駆ける", "tracks").await.unwrap();
        assert_eq!(res_jp.tracks.len(), 1);
        assert_eq!(res_jp.tracks[0].title, "夜に駆ける");

        // Accented characters
        let res_acc = search_library(&pool, "Café", "tracks").await.unwrap();
        assert_eq!(res_acc.tracks.len(), 1);
        assert_eq!(res_acc.tracks[0].title, "Café del Mar");
    }

    #[tokio::test]
    async fn test_soft_deleted_and_sync_triggers() {
        let pool = create_search_test_db().await;
        insert_track(
            &pool,
            "t1",
            "Active Song",
            "Artist 1",
            "Album 1",
            None,
            false,
        )
        .await;
        insert_track(
            &pool,
            "t2",
            "Deleted Song",
            "Artist 1",
            "Album 1",
            None,
            true,
        )
        .await;

        // Verify deleted song is not in search results
        let res1 = search_library(&pool, "Song", "tracks").await.unwrap();
        assert_eq!(res1.tracks.len(), 1);
        assert_eq!(res1.tracks[0].id, "t1");

        // Update active song to be soft-deleted
        sqlx::query("UPDATE tracks SET is_deleted = 1 WHERE id = 't1'")
            .execute(&pool)
            .await
            .unwrap();

        let res2 = search_library(&pool, "Song", "tracks").await.unwrap();
        assert_eq!(res2.tracks.len(), 0);

        // Restore song and verify index updates immediately via trigger
        sqlx::query("UPDATE tracks SET is_deleted = 0, title = 'Restored Hit' WHERE id = 't1'")
            .execute(&pool)
            .await
            .unwrap();

        let res3 = search_library(&pool, "Restored", "tracks").await.unwrap();
        assert_eq!(res3.tracks.len(), 1);
        assert_eq!(res3.tracks[0].title, "Restored Hit");
    }

    #[tokio::test]
    async fn test_playlist_name_and_description_search() {
        let pool = create_search_test_db().await;
        insert_playlist(
            &pool,
            "p1",
            "Workout Beats",
            "High tempo EDM selection",
            false,
        )
        .await;
        insert_playlist(
            &pool,
            "p2",
            "Relaxing Rain",
            "Ambient and lo-fi sounds",
            false,
        )
        .await;
        insert_playlist(&pool, "p3", "Deleted List", "Should not show", true).await;

        // Search playlist by name
        let res_name = search_library(&pool, "Workout", "playlists").await.unwrap();
        assert_eq!(res_name.playlists.len(), 1);
        assert_eq!(res_name.playlists[0].name, "Workout Beats");

        // Search playlist by description
        let res_desc = search_library(&pool, "ambient", "playlists").await.unwrap();
        assert_eq!(res_desc.playlists.len(), 1);
        assert_eq!(res_desc.playlists[0].name, "Relaxing Rain");

        // Verify deleted playlist is excluded
        let res_del = search_library(&pool, "Deleted", "playlists").await.unwrap();
        assert_eq!(res_del.playlists.len(), 0);
    }

    #[tokio::test]
    async fn test_album_and_artist_deduplication() {
        let pool = create_search_test_db().await;
        insert_track(
            &pool,
            "t1",
            "Track 1",
            "Daft Punk",
            "Discovery",
            None,
            false,
        )
        .await;
        insert_track(
            &pool,
            "t2",
            "Track 2",
            "Daft Punk",
            "Discovery",
            None,
            false,
        )
        .await;
        insert_track(
            &pool,
            "t3",
            "Track 3",
            "Daft Punk",
            "Discovery",
            None,
            false,
        )
        .await;
        insert_track(&pool, "t4", "Track 4", "Daft Punk", "Homework", None, false).await;

        // Searching "Discovery" should return 1 unique album with track_count=3
        let res_album = search_library(&pool, "Discovery", "albums").await.unwrap();
        assert_eq!(res_album.albums.len(), 1);
        assert_eq!(res_album.albums[0].title, "Discovery");
        assert_eq!(res_album.albums[0].artist, "Daft Punk");
        assert_eq!(res_album.albums[0].track_count, 3);

        // Searching "Daft Punk" should return 1 unique artist with album_count=2, track_count=4
        let res_artist = search_library(&pool, "Daft Punk", "artists").await.unwrap();
        assert_eq!(res_artist.artists.len(), 1);
        assert_eq!(res_artist.artists[0].name, "Daft Punk");
        assert_eq!(res_artist.artists[0].album_count, 2);
        assert_eq!(res_artist.artists[0].track_count, 4);
    }

    #[tokio::test]
    async fn test_category_filtering_and_limits() {
        let pool = create_search_test_db().await;
        for i in 0..25 {
            insert_track(
                &pool,
                &format!("t{i}"),
                &format!("Synth Pop {i}"),
                "Electronic Band",
                "Synth Waves",
                None,
                false,
            )
            .await;
        }
        insert_playlist(
            &pool,
            "p1",
            "Synth Favorites",
            "Collection of synth pop tracks",
            false,
        )
        .await;

        // Category: "tracks" only returns tracks (up to 50), albums/artists/playlists are empty
        let res_tracks = search_library(&pool, "Synth", "tracks").await.unwrap();
        assert_eq!(res_tracks.tracks.len(), 25);
        assert_eq!(res_tracks.albums.len(), 0);
        assert_eq!(res_tracks.artists.len(), 0);
        assert_eq!(res_tracks.playlists.len(), 0);

        // Category: "all" bounds tracks to 20
        let res_all = search_library(&pool, "Synth", "all").await.unwrap();
        assert_eq!(
            res_all.tracks.len(),
            20,
            "Category 'all' must bound tracks to 20"
        );
        assert_eq!(res_all.albums.len(), 1);
        assert_eq!(res_all.playlists.len(), 1);

        // Category: "playlists" only returns playlists
        let res_pl = search_library(&pool, "Synth", "playlists").await.unwrap();
        assert_eq!(res_pl.tracks.len(), 0);
        assert_eq!(res_pl.playlists.len(), 1);
    }

    #[tokio::test]
    async fn test_missing_metadata_and_deterministic_ordering() {
        let pool = create_search_test_db().await;
        insert_track(&pool, "t3", "Zebra Track", "", "", None, false).await;
        insert_track(&pool, "t1", "Alpha Track", "", "", None, false).await;
        insert_track(&pool, "t2", "Beta Track", "", "", None, false).await;

        let res = search_library(&pool, "Track", "tracks").await.unwrap();
        assert_eq!(res.tracks.len(), 3);
        assert_eq!(res.tracks[0].title, "Alpha Track");
        assert_eq!(res.tracks[1].title, "Beta Track");
        assert_eq!(res.tracks[2].title, "Zebra Track");
    }

    #[tokio::test]
    async fn test_initial_migration_population_and_hard_deletes() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();

        // 1. Run migrations 1 and 2 manually first
        let m1 = include_str!("../../migrations/0001_initial.sql");
        sqlx::raw_sql(m1).execute(&pool).await.unwrap();
        let m2 = include_str!("../../migrations/0002_playback_state_volume.sql");
        sqlx::raw_sql(m2).execute(&pool).await.unwrap();

        // 2. Insert tracks and playlists before search migration runs
        insert_track(
            &pool,
            "pre-1",
            "Pre-existing Classic",
            "Legacy Artist",
            "Golden Hits",
            None,
            false,
        )
        .await;
        insert_playlist(
            &pool,
            "pre-pl-1",
            "Pre-existing Jams",
            "Created prior to search migration",
            false,
        )
        .await;

        // 3. Now run migration 3 (search FTS5 migration)
        let m3 = include_str!("../../migrations/0003_library_search_fts5.sql");
        sqlx::raw_sql(m3).execute(&pool).await.unwrap();

        // 4. Verify pre-existing records are immediately searchable via FTS5
        let res_pre_track = search_library(&pool, "Classic", "tracks").await.unwrap();
        assert_eq!(res_pre_track.tracks.len(), 1);
        assert_eq!(res_pre_track.tracks[0].title, "Pre-existing Classic");

        let res_pre_pl = search_library(&pool, "Jams", "playlists").await.unwrap();
        assert_eq!(res_pre_pl.playlists.len(), 1);
        assert_eq!(res_pre_pl.playlists[0].name, "Pre-existing Jams");

        // 5. Test hard delete via triggers
        sqlx::query("DELETE FROM tracks WHERE id = 'pre-1'")
            .execute(&pool)
            .await
            .unwrap();

        let res_after_del = search_library(&pool, "Classic", "tracks").await.unwrap();
        assert_eq!(
            res_after_del.tracks.len(),
            0,
            "Hard-deleted track must be removed from FTS5"
        );

        sqlx::query("DELETE FROM playlists WHERE id = 'pre-pl-1'")
            .execute(&pool)
            .await
            .unwrap();

        let res_after_pl_del = search_library(&pool, "Jams", "playlists").await.unwrap();
        assert_eq!(
            res_after_pl_del.playlists.len(),
            0,
            "Hard-deleted playlist must be removed from FTS5"
        );
    }
}
