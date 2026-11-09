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
}
