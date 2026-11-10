use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::domain::{Album, Artist, Playlist, Track};

pub async fn get_all_tracks(pool: &SqlitePool) -> Result<Vec<Track>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            t.id,
            t.title,
            t.artist,
            t.album,
            t.album_artist,
            t.duration_ms,
            t.artwork_uri,
            t.source_kind,
            t.provider_id,
            t.file_uri,
            t.release_year,
            t.track_number,
            t.disc_number,
            t.genre,
            t.play_count,
            t.date_added,
            CASE WHEN l.track_id IS NOT NULL THEN 1 ELSE 0 END AS is_liked
        FROM tracks t
        LEFT JOIN liked_tracks l ON t.id = l.track_id
        WHERE t.is_deleted = 0
        ORDER BY t.title COLLATE NOCASE ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch tracks: {e}"))?;

    let tracks = rows
        .into_iter()
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
        .collect();

    Ok(tracks)
}

pub async fn get_albums(pool: &SqlitePool) -> Result<Vec<Album>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(NULLIF(album, ''), 'Unknown Album') AS title,
            COALESCE(NULLIF(album_artist, ''), NULLIF(artist, ''), 'Unknown Artist') AS artist,
            MIN(release_year) AS release_year,
            COUNT(*) AS track_count,
            MAX(artwork_uri) AS artwork_uri
        FROM tracks
        WHERE is_deleted = 0
        GROUP BY title, artist
        ORDER BY title COLLATE NOCASE ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch albums: {e}"))?;

    let albums = rows
        .into_iter()
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
        .collect();

    Ok(albums)
}

pub async fn get_artists(pool: &SqlitePool) -> Result<Vec<Artist>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(NULLIF(artist, ''), 'Unknown Artist') AS name,
            COUNT(*) AS track_count,
            COUNT(DISTINCT NULLIF(album, '')) AS album_count
        FROM tracks
        WHERE is_deleted = 0
        GROUP BY name
        ORDER BY name COLLATE NOCASE ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch artists: {e}"))?;

    let artists = rows
        .into_iter()
        .map(|row| {
            let name: String = row.get("name");
            let id = name.clone();
            Artist {
                id,
                name,
                track_count: row.get("track_count"),
                album_count: row.get("album_count"),
            }
        })
        .collect();

    Ok(artists)
}

pub async fn get_liked_tracks(pool: &SqlitePool) -> Result<Vec<Track>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            t.id,
            t.title,
            t.artist,
            t.album,
            t.album_artist,
            t.duration_ms,
            t.artwork_uri,
            t.source_kind,
            t.provider_id,
            t.file_uri,
            t.release_year,
            t.track_number,
            t.disc_number,
            t.genre,
            t.play_count,
            t.date_added,
            1 AS is_liked
        FROM tracks t
        JOIN liked_tracks l ON t.id = l.track_id
        WHERE t.is_deleted = 0
        ORDER BY l.liked_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch liked tracks: {e}"))?;

    let tracks = rows
        .into_iter()
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
            is_liked: true,
            play_count: row.get("play_count"),
            date_added: row.get("date_added"),
        })
        .collect();

    Ok(tracks)
}

pub async fn toggle_track_liked(pool: &SqlitePool, track_id: &str) -> Result<bool, String> {
    let exists = sqlx::query("SELECT 1 FROM liked_tracks WHERE track_id = ?")
        .bind(track_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Database error checking liked state: {e}"))?
        .is_some();

    if exists {
        sqlx::query("DELETE FROM liked_tracks WHERE track_id = ?")
            .bind(track_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to unlike track: {e}"))?;
        Ok(false)
    } else {
        sqlx::query("INSERT INTO liked_tracks (track_id) VALUES (?)")
            .bind(track_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to like track: {e}"))?;
        Ok(true)
    }
}

pub async fn get_playlists(pool: &SqlitePool) -> Result<Vec<Playlist>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.artwork_uri,
            p.source_provider,
            p.created_at,
            (SELECT COUNT(*) FROM playlist_tracks pt JOIN tracks t ON pt.track_id = t.id WHERE pt.playlist_id = p.id AND t.is_deleted = 0) AS track_count
        FROM playlists p
        WHERE p.is_deleted = 0
        ORDER BY p.created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch playlists: {e}"))?;

    let playlists = rows
        .into_iter()
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
        .collect();

    Ok(playlists)
}

pub async fn get_playlist_tracks(
    pool: &SqlitePool,
    playlist_id: &str,
) -> Result<Vec<Track>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            t.id,
            t.title,
            t.artist,
            t.album,
            t.album_artist,
            t.duration_ms,
            t.artwork_uri,
            t.source_kind,
            t.provider_id,
            t.file_uri,
            t.release_year,
            t.track_number,
            t.disc_number,
            t.genre,
            t.play_count,
            t.date_added,
            CASE WHEN l.track_id IS NOT NULL THEN 1 ELSE 0 END AS is_liked
        FROM playlist_tracks pt
        JOIN tracks t ON pt.track_id = t.id
        LEFT JOIN liked_tracks l ON t.id = l.track_id
        WHERE pt.playlist_id = ? AND t.is_deleted = 0
        ORDER BY pt.position ASC
        "#,
    )
    .bind(playlist_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch playlist tracks: {e}"))?;

    let tracks = rows
        .into_iter()
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
        .collect();

    Ok(tracks)
}

pub async fn create_playlist(
    pool: &SqlitePool,
    name: &str,
    description: &str,
) -> Result<Playlist, String> {
    let id = format!("pl-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO playlists (id, name, description) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(description)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create playlist: {e}"))?;

    let row = sqlx::query("SELECT created_at FROM playlists WHERE id = ?")
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to retrieve created playlist: {e}"))?;

    Ok(Playlist {
        id,
        name: name.to_string(),
        description: description.to_string(),
        artwork_uri: None,
        source_provider: None,
        track_count: 0,
        created_at: row.get("created_at"),
        tracks: Some(vec![]),
    })
}

pub async fn delete_playlist(pool: &SqlitePool, playlist_id: &str) -> Result<(), String> {
    sqlx::query("UPDATE playlists SET is_deleted = 1 WHERE id = ?")
        .bind(playlist_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete playlist: {e}"))?;

    Ok(())
}

pub async fn add_track_to_playlist(
    pool: &SqlitePool,
    playlist_id: &str,
    track_id: &str,
) -> Result<String, String> {
    let playlist_exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM playlists WHERE id = ? AND is_deleted = 0")
            .bind(playlist_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to verify playlist: {e}"))?;

    if playlist_exists.is_none() {
        return Err(format!("Playlist '{playlist_id}' not found or deleted"));
    }

    let track_exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM tracks WHERE id = ? AND is_deleted = 0")
            .bind(track_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to verify track: {e}"))?;

    if track_exists.is_none() {
        return Err(format!("Track '{track_id}' not found or deleted"));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {e}"))?;

    let next_pos: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?",
    )
    .bind(playlist_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("Failed to determine next position: {e}"))?;

    let entry_id = format!("plt-{}", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO playlist_tracks (id, playlist_id, track_id, position) VALUES (?, ?, ?, ?)",
    )
    .bind(&entry_id)
    .bind(playlist_id)
    .bind(track_id)
    .bind(next_pos)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to insert track into playlist: {e}"))?;

    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {e}"))?;

    Ok(entry_id)
}

pub async fn remove_track_from_playlist(
    pool: &SqlitePool,
    playlist_id: &str,
    position: i64,
) -> Result<(), String> {
    if position < 0 {
        return Err("Position must be non-negative".to_string());
    }

    let playlist_exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM playlists WHERE id = ? AND is_deleted = 0")
            .bind(playlist_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to verify playlist: {e}"))?;

    if playlist_exists.is_none() {
        return Err(format!("Playlist '{playlist_id}' not found or deleted"));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {e}"))?;

    let entry_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM playlist_tracks WHERE playlist_id = ? AND position = ?")
            .bind(playlist_id)
            .bind(position)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Failed to query playlist track: {e}"))?;

    let entry_id = match entry_id {
        Some(id) => id,
        None => {
            return Err(format!(
                "No track found in playlist '{playlist_id}' at position {position}"
            ));
        }
    };

    sqlx::query("DELETE FROM playlist_tracks WHERE id = ?")
        .bind(&entry_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to delete playlist track: {e}"))?;

    // Phase 1: Shift subsequent rows into large temporary space to avoid UNIQUE(playlist_id, position) collisions
    sqlx::query(
        "UPDATE playlist_tracks SET position = position + 1000000 WHERE playlist_id = ? AND position > ?",
    )
    .bind(playlist_id)
    .bind(position)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to shift positions in phase 1: {e}"))?;

    // Phase 2: Shift back down by 1000001 (net change: -1)
    sqlx::query(
        "UPDATE playlist_tracks SET position = position - 1000001 WHERE playlist_id = ? AND position >= 1000000",
    )
    .bind(playlist_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to compact positions in phase 2: {e}"))?;

    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {e}"))?;

    Ok(())
}

pub async fn reorder_playlist_tracks(
    pool: &SqlitePool,
    playlist_id: &str,
    from_pos: i64,
    to_pos: i64,
) -> Result<(), String> {
    if from_pos < 0 || to_pos < 0 {
        return Err("Positions must be non-negative".to_string());
    }
    if from_pos == to_pos {
        return Ok(());
    }

    let playlist_exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM playlists WHERE id = ? AND is_deleted = 0")
            .bind(playlist_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to verify playlist: {e}"))?;

    if playlist_exists.is_none() {
        return Err(format!("Playlist '{playlist_id}' not found or deleted"));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {e}"))?;

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(playlist_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("Failed to count playlist tracks: {e}"))?;

    if from_pos >= count || to_pos >= count {
        return Err(format!(
            "Invalid position: from_pos={from_pos}, to_pos={to_pos}, total tracks={count}"
        ));
    }

    // Step 1: Move target track to temporary out-of-range position
    sqlx::query(
        "UPDATE playlist_tracks SET position = 1000000 WHERE playlist_id = ? AND position = ?",
    )
    .bind(playlist_id)
    .bind(from_pos)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to isolate moving track: {e}"))?;

    // Step 2: Shift intervening tracks using 2-phase disjoint update
    if from_pos < to_pos {
        // Moving downwards: items in (from_pos, to_pos] shift down by 1 to [from_pos, to_pos)
        sqlx::query(
            "UPDATE playlist_tracks SET position = position + 2000000 WHERE playlist_id = ? AND position > ? AND position <= ?",
        )
        .bind(playlist_id)
        .bind(from_pos)
        .bind(to_pos)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to shift intervening tracks (phase 2a): {e}"))?;

        sqlx::query(
            "UPDATE playlist_tracks SET position = position - 2000001 WHERE playlist_id = ? AND position >= 2000000",
        )
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to shift intervening tracks (phase 2b): {e}"))?;
    } else {
        // Moving upwards: items in [to_pos, from_pos) shift up by 1 to (to_pos, from_pos]
        sqlx::query(
            "UPDATE playlist_tracks SET position = position + 2000000 WHERE playlist_id = ? AND position >= ? AND position < ?",
        )
        .bind(playlist_id)
        .bind(to_pos)
        .bind(from_pos)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to shift intervening tracks (phase 2a): {e}"))?;

        sqlx::query(
            "UPDATE playlist_tracks SET position = position - 1999999 WHERE playlist_id = ? AND position >= 2000000",
        )
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to shift intervening tracks (phase 2b): {e}"))?;
    }

    // Step 3: Move isolated track into destination position to_pos
    sqlx::query(
        "UPDATE playlist_tracks SET position = ? WHERE playlist_id = ? AND position = 1000000",
    )
    .bind(to_pos)
    .bind(playlist_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to place moving track at target position: {e}"))?;

    // Step 4: Update playlists.updated_at
    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {e}"))?;

    Ok(())
}

pub async fn update_playlist(
    pool: &SqlitePool,
    playlist_id: &str,
    name: &str,
    description: &str,
) -> Result<Playlist, String> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err("Playlist name cannot be empty".to_string());
    }

    let result = sqlx::query(
        "UPDATE playlists SET name = ?, description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND is_deleted = 0",
    )
    .bind(trimmed_name)
    .bind(description.trim())
    .bind(playlist_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to update playlist: {e}"))?;

    if result.rows_affected() == 0 {
        return Err(format!("Playlist '{playlist_id}' not found or deleted"));
    }

    let row = sqlx::query(
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.artwork_uri,
            p.source_provider,
            p.created_at,
            (SELECT COUNT(*) FROM playlist_tracks pt JOIN tracks t ON pt.track_id = t.id WHERE pt.playlist_id = p.id AND t.is_deleted = 0) AS track_count
        FROM playlists p
        WHERE p.id = ? AND p.is_deleted = 0
        "#,
    )
    .bind(playlist_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to fetch updated playlist: {e}"))?;

    Ok(Playlist {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        artwork_uri: row.get("artwork_uri"),
        source_provider: row.get("source_provider"),
        track_count: row.get("track_count"),
        created_at: row.get("created_at"),
        tracks: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn insert_dummy_track(pool: &SqlitePool, id: &str, title: &str) {
        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, artist, album) VALUES (?, 'local', ?, 'Artist', 'Album')",
        )
        .bind(id)
        .bind(title)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_playlist_lifecycle() {
        let pool = create_test_db().await;

        let created = create_playlist(&pool, "Night Jazz", "Late night curation")
            .await
            .unwrap();
        assert_eq!(created.name, "Night Jazz");
        assert_eq!(created.description, "Late night curation");

        let playlists = get_playlists(&pool).await.unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Night Jazz");

        delete_playlist(&pool, &created.id).await.unwrap();
        let remaining = get_playlists(&pool).await.unwrap();
        assert_eq!(remaining.len(), 0);
    }

    #[tokio::test]
    async fn test_toggle_liked_track() {
        let pool = create_test_db().await;

        // Insert a dummy track
        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, artist, album) VALUES ('track-1', 'local', 'Song A', 'Artist A', 'Album A')"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Initially not liked
        let tracks = get_all_tracks(&pool).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert!(!tracks[0].is_liked);

        // Toggle liked -> true
        let liked = toggle_track_liked(&pool, "track-1").await.unwrap();
        assert!(liked);

        let liked_tracks = get_liked_tracks(&pool).await.unwrap();
        assert_eq!(liked_tracks.len(), 1);
        assert_eq!(liked_tracks[0].id, "track-1");

        // Toggle liked again -> false
        let unliked = toggle_track_liked(&pool, "track-1").await.unwrap();
        assert!(!unliked);

        let liked_tracks_after = get_liked_tracks(&pool).await.unwrap();
        assert_eq!(liked_tracks_after.len(), 0);
    }

    #[tokio::test]
    async fn test_persistence_across_reconnect() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join(format!("cavia_test_persist_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let db_path = temp_dir.join("test_persistence.sqlite3");

        // 1. Initialize and migrate database
        {
            let opts = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
            sqlx::migrate!("./migrations").run(&pool).await.unwrap();

            // Create a playlist
            create_playlist(&pool, "Road Trip", "Upbeat tracks")
                .await
                .unwrap();
            // Insert a track
            sqlx::query("INSERT INTO tracks (id, source_kind, title, artist, album) VALUES ('t1', 'local', 'Song 1', 'Artist 1', 'Album 1')")
                .execute(&pool)
                .await
                .unwrap();
        }

        // 2. Re-open pool from the exact same file path (simulating app restart)
        {
            let opts = sqlx::sqlite::SqliteConnectOptions::new().filename(&db_path);
            let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();

            let tracks = get_all_tracks(&pool).await.unwrap();
            assert_eq!(tracks.len(), 1);
            assert_eq!(tracks[0].title, "Song 1");

            let playlists = get_playlists(&pool).await.unwrap();
            assert_eq!(playlists.len(), 1);
            assert_eq!(playlists[0].name, "Road Trip");
        }

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_add_track_to_playlist_success_and_duplicates() {
        let pool = create_test_db().await;

        sqlx::query("INSERT INTO tracks (id, source_kind, title, artist, album) VALUES ('t1', 'local', 'Track 1', 'Artist 1', 'Album 1')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tracks (id, source_kind, title, artist, album) VALUES ('t2', 'local', 'Track 2', 'Artist 2', 'Album 2')")
            .execute(&pool)
            .await
            .unwrap();

        let pl = create_playlist(&pool, "Mix", "Test description")
            .await
            .unwrap();

        let e1 = add_track_to_playlist(&pool, &pl.id, "t1").await.unwrap();
        assert!(e1.starts_with("plt-"));

        let e2 = add_track_to_playlist(&pool, &pl.id, "t2").await.unwrap();
        assert!(e2.starts_with("plt-"));
        assert_ne!(e1, e2);

        // Duplicates are permitted at distinct positions
        let e3 = add_track_to_playlist(&pool, &pl.id, "t1").await.unwrap();
        assert!(e3.starts_with("plt-"));
        assert_ne!(e1, e3);

        let pl_tracks = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        assert_eq!(pl_tracks.len(), 3);
        assert_eq!(pl_tracks[0].id, "t1");
        assert_eq!(pl_tracks[1].id, "t2");
        assert_eq!(pl_tracks[2].id, "t1");

        let pl_list = get_playlists(&pool).await.unwrap();
        assert_eq!(pl_list[0].track_count, 3);
    }

    #[tokio::test]
    async fn test_add_track_validation_errors() {
        let pool = create_test_db().await;

        sqlx::query("INSERT INTO tracks (id, source_kind, title, artist, album) VALUES ('t1', 'local', 'Track 1', 'Artist 1', 'Album 1')")
            .execute(&pool)
            .await
            .unwrap();

        let pl = create_playlist(&pool, "Mix", "Test").await.unwrap();

        // Non-existent track
        let err1 = add_track_to_playlist(&pool, &pl.id, "non-existent").await;
        assert!(err1.is_err());
        assert!(err1.unwrap_err().contains("not found"));

        // Non-existent playlist
        let err2 = add_track_to_playlist(&pool, "pl-none", "t1").await;
        assert!(err2.is_err());
        assert!(err2.unwrap_err().contains("not found"));

        // Deleted playlist
        delete_playlist(&pool, &pl.id).await.unwrap();
        let err3 = add_track_to_playlist(&pool, &pl.id, "t1").await;
        assert!(err3.is_err());
        assert!(err3.unwrap_err().contains("not found or deleted"));
    }

    #[tokio::test]
    async fn test_remove_track_from_playlist_positions() {
        let pool = create_test_db().await;

        for i in 0..4 {
            insert_dummy_track(&pool, &format!("t{i}"), &format!("Track {i}")).await;
        }

        let pl = create_playlist(&pool, "Four Songs", "").await.unwrap();
        for i in 0..4 {
            add_track_to_playlist(&pool, &pl.id, &format!("t{i}"))
                .await
                .unwrap();
        }

        // Remove middle track (position 1, which is t1)
        remove_track_from_playlist(&pool, &pl.id, 1).await.unwrap();
        let tracks = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].id, "t0");
        assert_eq!(tracks[1].id, "t2");
        assert_eq!(tracks[2].id, "t3");

        // Remove first track (position 0, which is t0)
        remove_track_from_playlist(&pool, &pl.id, 0).await.unwrap();
        let tracks2 = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        assert_eq!(tracks2.len(), 2);
        assert_eq!(tracks2[0].id, "t2");
        assert_eq!(tracks2[1].id, "t3");

        // Remove last track (position 1, which is t3)
        remove_track_from_playlist(&pool, &pl.id, 1).await.unwrap();
        let tracks3 = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        assert_eq!(tracks3.len(), 1);
        assert_eq!(tracks3[0].id, "t2");

        // Out of bounds position
        let err = remove_track_from_playlist(&pool, &pl.id, 5).await;
        assert!(err.is_err());

        // Negative position
        let err_neg = remove_track_from_playlist(&pool, &pl.id, -1).await;
        assert!(err_neg.is_err());
    }

    #[tokio::test]
    async fn test_reorder_playlist_tracks() {
        let pool = create_test_db().await;

        for i in 0..5 {
            insert_dummy_track(&pool, &format!("t{i}"), &format!("Track {i}")).await;
        }

        let pl = create_playlist(&pool, "Five Tracks", "").await.unwrap();
        for i in 0..5 {
            add_track_to_playlist(&pool, &pl.id, &format!("t{i}"))
                .await
                .unwrap();
        }

        // Initial order: [t0, t1, t2, t3, t4]
        // 1. Reorder identity (pos 2 -> pos 2)
        reorder_playlist_tracks(&pool, &pl.id, 2, 2).await.unwrap();
        let list1 = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        let ids1: Vec<String> = list1.into_iter().map(|t| t.id).collect();
        assert_eq!(ids1, vec!["t0", "t1", "t2", "t3", "t4"]);

        // 2. Reorder forward (move pos 0 to pos 3)
        // t0 should move after t3: [t1, t2, t3, t0, t4]
        reorder_playlist_tracks(&pool, &pl.id, 0, 3).await.unwrap();
        let list2 = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        let ids2: Vec<String> = list2.into_iter().map(|t| t.id).collect();
        assert_eq!(ids2, vec!["t1", "t2", "t3", "t0", "t4"]);

        // 3. Reorder backward (move pos 3 (t0) back to pos 0)
        // [t0, t1, t2, t3, t4]
        reorder_playlist_tracks(&pool, &pl.id, 3, 0).await.unwrap();
        let list3 = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        let ids3: Vec<String> = list3.into_iter().map(|t| t.id).collect();
        assert_eq!(ids3, vec!["t0", "t1", "t2", "t3", "t4"]);

        // 4. Reorder invalid out-of-bounds positions
        let err = reorder_playlist_tracks(&pool, &pl.id, 0, 10).await;
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("Invalid position"));

        let err2 = reorder_playlist_tracks(&pool, &pl.id, 10, 0).await;
        assert!(err2.is_err());
        assert!(err2.unwrap_err().contains("Invalid position"));
    }

    #[tokio::test]
    async fn test_update_playlist_metadata() {
        let pool = create_test_db().await;

        let pl = create_playlist(&pool, "Old Name", "Old Description")
            .await
            .unwrap();

        let updated = update_playlist(&pool, &pl.id, "New Name", "New Description")
            .await
            .unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.description, "New Description");

        let fetched = get_playlists(&pool).await.unwrap();
        assert_eq!(fetched[0].name, "New Name");
        assert_eq!(fetched[0].description, "New Description");

        // Empty name should fail
        let err = update_playlist(&pool, &pl.id, "   ", "Something").await;
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("cannot be empty"));

        // Non-existent playlist
        let err2 = update_playlist(&pool, "pl-none", "Valid", "Desc").await;
        assert!(err2.is_err());
        assert!(err2.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_playlist_management_persistence_across_reconnect() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join(format!("cavia_test_pl_mgmt_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let db_path = temp_dir.join("test_pl_mgmt.sqlite3");

        let pl_id;
        // Phase 1: Create, add tracks, reorder, remove one, update info
        {
            let opts = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
            sqlx::migrate!("./migrations").run(&pool).await.unwrap();

            for i in 0..3 {
                insert_dummy_track(&pool, &format!("t{i}"), &format!("Song {i}")).await;
            }

            let pl = create_playlist(&pool, "Initial Playlist", "Initial Desc")
                .await
                .unwrap();
            pl_id = pl.id.clone();

            add_track_to_playlist(&pool, &pl_id, "t0").await.unwrap();
            add_track_to_playlist(&pool, &pl_id, "t1").await.unwrap();
            add_track_to_playlist(&pool, &pl_id, "t2").await.unwrap();

            // Reorder: move t0 to pos 2 -> [t1, t2, t0]
            reorder_playlist_tracks(&pool, &pl_id, 0, 2).await.unwrap();

            // Remove pos 1 (t2) -> [t1, t0]
            remove_track_from_playlist(&pool, &pl_id, 1).await.unwrap();

            // Update metadata
            update_playlist(&pool, &pl_id, "Persisted Mix", "Updated Desc")
                .await
                .unwrap();
        }

        // Phase 2: Reopen pool simulating fresh app restart
        {
            let opts = sqlx::sqlite::SqliteConnectOptions::new().filename(&db_path);
            let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();

            let playlists = get_playlists(&pool).await.unwrap();
            assert_eq!(playlists.len(), 1);
            assert_eq!(playlists[0].name, "Persisted Mix");
            assert_eq!(playlists[0].description, "Updated Desc");
            assert_eq!(playlists[0].track_count, 2);

            let tracks = get_playlist_tracks(&pool, &pl_id).await.unwrap();
            assert_eq!(tracks.len(), 2);
            assert_eq!(tracks[0].id, "t1");
            assert_eq!(tracks[1].id, "t0");
        }

        let _ = fs::remove_dir_all(temp_dir);
    }
}
