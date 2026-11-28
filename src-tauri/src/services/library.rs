use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::domain::{Album, Artist, Playlist, Track};

fn row_to_track(row: &sqlx::sqlite::SqliteRow) -> Track {
    Track {
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
        queue_entry_id: None,
        contributor_id: None,
        contributor_name: None,
        priority_override: None,
    }
}

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

    Ok(rows.iter().map(row_to_track).collect())
}

pub async fn get_track_by_id(pool: &SqlitePool, track_id: &str) -> Result<Option<Track>, String> {
    let row = sqlx::query(
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
        WHERE t.id = ? AND t.is_deleted = 0
        LIMIT 1
        "#,
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to fetch track: {e}"))?;

    Ok(row.as_ref().map(row_to_track))
}

pub async fn get_tracks_by_ids(
    pool: &SqlitePool,
    track_ids: &[String],
) -> Result<Vec<Track>, String> {
    if track_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut unique_ids: Vec<&str> = Vec::with_capacity(track_ids.len());
    let mut seen = std::collections::HashSet::with_capacity(track_ids.len());
    for id in track_ids {
        if seen.insert(id.as_str()) {
            unique_ids.push(id.as_str());
        }
    }

    let mut by_id = std::collections::HashMap::with_capacity(unique_ids.len());
    // Keep SQL bind count comfortably below SQLite's default 999 variable limit.
    for chunk in unique_ids.chunks(500) {
        let mut builder = sqlx::QueryBuilder::new(
            "SELECT t.id, t.title, t.artist, t.album, t.album_artist, t.duration_ms, \
             t.artwork_uri, t.source_kind, t.provider_id, t.file_uri, t.release_year, \
             t.track_number, t.disc_number, t.genre, t.play_count, t.date_added, \
             CASE WHEN l.track_id IS NOT NULL THEN 1 ELSE 0 END AS is_liked \
             FROM tracks t LEFT JOIN liked_tracks l ON t.id = l.track_id \
             WHERE t.is_deleted = 0 AND t.id IN (",
        );
        let mut separated = builder.separated(", ");
        for id in chunk {
            separated.push_bind(*id);
        }
        separated.push_unseparated(")");

        let rows = builder
            .build()
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to fetch requested tracks: {e}"))?;

        for row in &rows {
            let track = row_to_track(row);
            by_id.insert(track.id.clone(), track);
        }
    }

    let mut ordered = Vec::with_capacity(track_ids.len());
    for id in track_ids {
        if let Some(track) = by_id.get(id) {
            ordered.push(track.clone());
        }
    }
    Ok(ordered)
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
        GROUP BY title COLLATE NOCASE, artist COLLATE NOCASE
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
            let id = format!(
                "album:{}:{}",
                hex::encode(title.to_lowercase()),
                hex::encode(artist.to_lowercase())
            );
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
            COUNT(DISTINCT NULLIF(album, '')) AS album_count,
            MAX(artwork_uri) AS artwork_uri
        FROM tracks
        WHERE is_deleted = 0
        GROUP BY name COLLATE NOCASE
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
            let id = format!("artist:{}", hex::encode(name.to_lowercase()));
            Artist {
                id,
                name,
                track_count: row.get("track_count"),
                album_count: row.get("album_count"),
                artwork_uri: row.get("artwork_uri"),
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
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        })
        .collect();

    Ok(tracks)
}

pub async fn toggle_track_liked(pool: &SqlitePool, track_id: &str) -> Result<bool, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Database error beginning transaction: {e}"))?;

    let exists = sqlx::query("SELECT 1 FROM liked_tracks WHERE track_id = ?")
        .bind(track_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Database error checking liked state: {e}"))?
        .is_some();

    // Fetch track metadata snapshot for sync outbox
    let trk_row = sqlx::query(
        "SELECT title, artist, album, duration_ms, content_hash FROM tracks WHERE id = ?",
    )
    .bind(track_id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or(None);

    let (title, artist, album, duration_ms, content_hash) = match trk_row {
        Some(r) => (
            r.get::<String, _>("title"),
            r.get::<String, _>("artist"),
            r.get::<String, _>("album"),
            r.get::<i64, _>("duration_ms"),
            r.get::<Option<String>, _>("content_hash"),
        ),
        None => (
            "Unknown Title".to_string(),
            "".to_string(),
            "".to_string(),
            0,
            None,
        ),
    };

    let new_liked = if exists {
        sqlx::query("DELETE FROM liked_tracks WHERE track_id = ?")
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to unlike track: {e}"))?;
        false
    } else {
        sqlx::query("INSERT INTO liked_tracks (track_id) VALUES (?)")
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to like track: {e}"))?;
        true
    };

    let _ = crate::services::sync::outbox::OutboxService::record_track_like_status(
        &mut *tx,
        track_id,
        content_hash.as_deref(),
        &title,
        &artist,
        &album,
        duration_ms,
        new_liked,
    )
    .await;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit liked state transaction: {e}"))?;

    Ok(new_liked)
}

pub async fn unlike_tracks(pool: &SqlitePool, track_ids: &[String]) -> Result<usize, String> {
    if track_ids.is_empty() {
        return Ok(0);
    }
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Database error beginning transaction: {e}"))?;

    let mut count = 0;
    for tid in track_ids {
        let deleted = sqlx::query("DELETE FROM liked_tracks WHERE track_id = ?")
            .bind(tid)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to unlike track: {e}"))?;

        if deleted.rows_affected() > 0 {
            count += 1;
            let trk_row = sqlx::query(
                "SELECT title, artist, album, duration_ms, content_hash FROM tracks WHERE id = ?",
            )
            .bind(tid)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None);

            if let Some(r) = trk_row {
                let _ = crate::services::sync::outbox::OutboxService::record_track_like_status(
                    &mut *tx,
                    tid,
                    r.get::<Option<String>, _>("content_hash").as_deref(),
                    &r.get::<String, _>("title"),
                    &r.get::<String, _>("artist"),
                    &r.get::<String, _>("album"),
                    r.get::<i64, _>("duration_ms"),
                    false,
                )
                .await;
            }
        }
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit unlike tracks: {e}"))?;

    Ok(count)
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
            p.is_pinned,
            p.last_used_at,
            p.created_at,
            (SELECT COUNT(*) FROM playlist_tracks pt LEFT JOIN tracks t ON pt.track_id = t.id WHERE pt.playlist_id = p.id AND (t.is_deleted IS NULL OR t.is_deleted = 0)) AS track_count
        FROM playlists p
        WHERE p.is_deleted = 0
        ORDER BY p.is_pinned DESC, COALESCE(p.last_used_at, p.updated_at) DESC, p.created_at DESC
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
            is_pinned: row.get::<i64, _>("is_pinned") == 1,
            last_used_at: row.get("last_used_at"),
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
            CASE
                WHEN t.id IS NULL AND pt.provider_track_id IS NOT NULL
                THEN 'ytmusic:' || pt.provider_track_id
                ELSE COALESCE(t.id, pt.id)
            END AS id,
            COALESCE(t.title, pt.title, 'Unknown Title') AS title,
            COALESCE(t.artist, pt.artist, 'Unknown Artist') AS artist,
            COALESCE(t.album, pt.album, '') AS album,
            t.album_artist,
            COALESCE(t.duration_ms, pt.duration_ms, 0) AS duration_ms,
            COALESCE(t.artwork_uri, pt.artwork_uri) AS artwork_uri,
            CASE
                WHEN t.id IS NOT NULL THEN t.source_kind
                WHEN pt.provider_track_id IS NOT NULL THEN 'provider'
                ELSE 'remote_sync'
            END AS source_kind,
            COALESCE(t.provider_id, pt.provider_id) AS provider_id,
            t.file_uri,
            t.release_year,
            t.track_number,
            t.disc_number,
            t.genre,
            COALESCE(t.play_count, 0) AS play_count,
            COALESCE(t.date_added, pt.added_at) AS date_added,
            CASE WHEN l.track_id IS NOT NULL THEN 1 ELSE 0 END AS is_liked
        FROM playlist_tracks pt
        LEFT JOIN tracks t ON pt.track_id = t.id
        LEFT JOIN liked_tracks l ON t.id = l.track_id
        WHERE pt.playlist_id = ? AND (t.is_deleted IS NULL OR t.is_deleted = 0)
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
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
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
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {e}"))?;

    sqlx::query(
        "INSERT INTO playlists (id, name, description, last_used_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)",
    )
        .bind(&id)
        .bind(name)
        .bind(description)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to create playlist: {e}"))?;

    let row = sqlx::query("SELECT created_at, is_pinned, last_used_at FROM playlists WHERE id = ?")
        .bind(&id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Failed to retrieve created playlist: {e}"))?;

    let _ = crate::services::sync::outbox::OutboxService::record_playlist_create(
        &mut *tx,
        &id,
        name,
        description,
    )
    .await;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit create playlist: {e}"))?;

    Ok(Playlist {
        id,
        name: name.to_string(),
        description: description.to_string(),
        artwork_uri: None,
        source_provider: None,
        track_count: 0,
        is_pinned: row.get::<i64, _>("is_pinned") == 1,
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
        tracks: Some(vec![]),
    })
}

pub async fn delete_playlist(pool: &SqlitePool, playlist_id: &str) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {e}"))?;

    sqlx::query("UPDATE playlists SET is_deleted = 1 WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to delete playlist: {e}"))?;

    let _ =
        crate::services::sync::outbox::OutboxService::record_playlist_delete(&mut *tx, playlist_id)
            .await;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit delete playlist: {e}"))?;

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

    let track_row = sqlx::query(
        "SELECT title, artist, album, duration_ms, content_hash FROM tracks WHERE id = ? AND is_deleted = 0",
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to verify track: {e}"))?;

    let track_row = match track_row {
        Some(r) => r,
        None => return Err(format!("Track '{track_id}' not found or deleted")),
    };

    let title: String = track_row.get("title");
    let artist: String = track_row.get("artist");
    let album: String = track_row.get("album");
    let duration_ms: i64 = track_row.get("duration_ms");
    let content_hash: Option<String> = track_row.get("content_hash");

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
        r#"
        INSERT INTO playlist_tracks (
            id, playlist_id, track_id, cloud_track_id, title, artist, album, duration_ms, content_hash, position
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&entry_id)
    .bind(playlist_id)
    .bind(track_id)
    .bind(track_id)
    .bind(&title)
    .bind(&artist)
    .bind(&album)
    .bind(duration_ms)
    .bind(&content_hash)
    .bind(next_pos)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to insert track into playlist: {e}"))?;

    let _ = crate::services::sync::outbox::OutboxService::record_track_add(
        &mut *tx,
        playlist_id,
        &entry_id,
        track_id,
        &title,
        &artist,
        &album,
        duration_ms,
        content_hash.as_deref(),
        next_pos,
    )
    .await;

    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP, last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {e}"))?;

    Ok(entry_id)
}

/// Appends a catalogue entry (no local audio file) to a playlist. The entry
/// keeps its own snapshot so the playlist renders even offline, and carries
/// the provider id the player needs to resolve a stream.
#[allow(clippy::too_many_arguments)]
pub async fn add_provider_track_to_playlist(
    pool: &SqlitePool,
    playlist_id: &str,
    provider_id: &str,
    provider_track_id: &str,
    title: &str,
    artist: &str,
    album: &str,
    duration_ms: i64,
    artwork_uri: Option<&str>,
) -> Result<(), String> {
    add_provider_track_to_playlist_with_entry_id(
        pool,
        playlist_id,
        provider_id,
        provider_track_id,
        title,
        artist,
        album,
        duration_ms,
        artwork_uri,
    )
    .await
    .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
async fn add_provider_track_to_playlist_with_entry_id(
    pool: &SqlitePool,
    playlist_id: &str,
    provider_id: &str,
    provider_track_id: &str,
    title: &str,
    artist: &str,
    album: &str,
    duration_ms: i64,
    artwork_uri: Option<&str>,
) -> Result<String, String> {
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
    let cloud_track_id = format!("{provider_id}:{provider_track_id}");
    sqlx::query(
        r#"
        INSERT INTO playlist_tracks (
            id, playlist_id, track_id, cloud_track_id, title, artist, album, duration_ms,
            position, provider_id, provider_track_id, artwork_uri
        ) VALUES (?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&entry_id)
    .bind(playlist_id)
    .bind(&cloud_track_id)
    .bind(title)
    .bind(artist)
    .bind(album)
    .bind(duration_ms)
    .bind(next_pos)
    .bind(provider_id)
    .bind(provider_track_id)
    .bind(artwork_uri)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to add catalogue track to playlist: {e}"))?;

    let _ = crate::services::sync::outbox::OutboxService::record_track_add(
        &mut *tx,
        playlist_id,
        &entry_id,
        &cloud_track_id,
        title,
        artist,
        album,
        duration_ms,
        None,
        next_pos,
    )
    .await;

    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP, last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit provider track transaction: {e}"))?;

    Ok(entry_id)
}

pub async fn quick_add_track_to_playlist(
    pool: &SqlitePool,
    playlist_id: &str,
    track: &Track,
) -> Result<crate::domain::QuickAddPlaylistResult, String> {
    let playlist_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM playlists WHERE id = ? AND is_deleted = 0")
            .bind(playlist_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to verify playlist: {e}"))?;

    let Some(playlist_name) = playlist_name else {
        return Err(format!("Playlist '{playlist_id}' not found or deleted"));
    };

    let is_provider = track.source_kind == "provider" || track.id.starts_with("ytmusic:");
    if is_provider {
        let provider_id = track.provider_id.as_deref().unwrap_or("youtube_music");
        let provider_track_id = track.id.strip_prefix("ytmusic:").unwrap_or(&track.id);
        let already_exists: Option<String> = sqlx::query_scalar(
            r#"
            SELECT id FROM playlist_tracks
            WHERE playlist_id = ? AND provider_id = ? AND provider_track_id = ?
            LIMIT 1
            "#,
        )
        .bind(playlist_id)
        .bind(provider_id)
        .bind(provider_track_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to check duplicate playlist track: {e}"))?;

        if already_exists.is_some() {
            let _ = sqlx::query("UPDATE playlists SET last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(playlist_id)
                .execute(pool)
                .await;
            return Ok(crate::domain::QuickAddPlaylistResult {
                status: "already_present".to_string(),
                entry_id: None,
                playlist_id: playlist_id.to_string(),
                playlist_name,
            });
        }

        let entry_id = add_provider_track_to_playlist_with_entry_id(
            pool,
            playlist_id,
            provider_id,
            provider_track_id,
            &track.title,
            &track.artist,
            &track.album,
            track.duration_ms,
            track.artwork_uri.as_deref(),
        )
        .await?;

        Ok(crate::domain::QuickAddPlaylistResult {
            status: "added".to_string(),
            entry_id: Some(entry_id),
            playlist_id: playlist_id.to_string(),
            playlist_name,
        })
    } else {
        let already_exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM playlist_tracks WHERE playlist_id = ? AND track_id = ? LIMIT 1",
        )
        .bind(playlist_id)
        .bind(&track.id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to check duplicate playlist track: {e}"))?;

        if already_exists.is_some() {
            let _ = sqlx::query("UPDATE playlists SET last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(playlist_id)
                .execute(pool)
                .await;
            return Ok(crate::domain::QuickAddPlaylistResult {
                status: "already_present".to_string(),
                entry_id: None,
                playlist_id: playlist_id.to_string(),
                playlist_name,
            });
        }

        let entry_id = add_track_to_playlist(pool, playlist_id, &track.id).await?;
        Ok(crate::domain::QuickAddPlaylistResult {
            status: "added".to_string(),
            entry_id: Some(entry_id),
            playlist_id: playlist_id.to_string(),
            playlist_name,
        })
    }
}

pub async fn remove_playlist_entry_by_id(
    pool: &SqlitePool,
    playlist_id: &str,
    entry_id: &str,
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to start undo transaction: {e}"))?;

    sqlx::query("DELETE FROM playlist_tracks WHERE id = ? AND playlist_id = ?")
        .bind(entry_id)
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to undo playlist track addition: {e}"))?;

    recompact_playlist_positions_in_tx(&mut tx, playlist_id).await?;

    let _ =
        crate::services::sync::outbox::OutboxService::record_track_remove(&mut tx, playlist_id, entry_id)
            .await;

    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit undo playlist removal: {e}"))?;

    Ok(())
}

pub async fn toggle_playlist_pinned(
    pool: &SqlitePool,
    playlist_id: &str,
    pinned: bool,
) -> Result<Playlist, String> {
    let res = sqlx::query(
        r#"
        UPDATE playlists
        SET is_pinned = ?, updated_at = CURRENT_TIMESTAMP
        WHERE id = ? AND is_deleted = 0
        "#,
    )
    .bind(if pinned { 1 } else { 0 })
    .bind(playlist_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to update playlist pin state: {e}"))?;

    if res.rows_affected() == 0 {
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
            p.is_pinned,
            p.last_used_at,
            p.created_at,
            (SELECT COUNT(*) FROM playlist_tracks pt LEFT JOIN tracks t ON pt.track_id = t.id WHERE pt.playlist_id = p.id AND (t.is_deleted IS NULL OR t.is_deleted = 0)) AS track_count
        FROM playlists p
        WHERE p.id = ? AND p.is_deleted = 0
        "#,
    )
    .bind(playlist_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to load updated playlist: {e}"))?;

    Ok(Playlist {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        artwork_uri: row.get("artwork_uri"),
        source_provider: row.get("source_provider"),
        track_count: row.get("track_count"),
        is_pinned: row.get::<i64, _>("is_pinned") == 1,
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
        tracks: None,
    })
}

/// Finds a local track for a track that came from another service. The title
/// must match exactly (case and surrounding whitespace ignored) and, when the
/// caller knows the artist, at least one artist token must overlap. A title
/// match alone is never enough, so a different recording is never substituted.
pub async fn find_local_track_id(pool: &SqlitePool, title: &str, artist: &str) -> Option<String> {
    let rows = sqlx::query(
        "SELECT id, artist FROM tracks WHERE is_deleted = 0 AND lower(trim(title)) = lower(trim(?)) LIMIT 8",
    )
    .bind(title)
    .fetch_all(pool)
    .await
    .ok()?;

    if rows.is_empty() {
        return None;
    }

    let tokens: Vec<String> = artist
        .to_lowercase()
        .split([',', '&', ';', '/', ' '])
        .map(str::trim)
        .filter(|token| token.chars().count() > 2)
        .map(|token| token.to_string())
        .collect();

    let mut title_only: Option<String> = None;
    for row in rows {
        let id: String = row.get("id");
        let track_artist: String = row.get::<String, _>("artist").to_lowercase();
        if tokens.iter().any(|token| track_artist.contains(token)) {
            return Some(id);
        }
        title_only.get_or_insert(id);
    }

    if tokens.is_empty() { title_only } else { None }
}

/// Records intentional user-selected playback after audio actually starts.
/// Automatic progressions (queue advance, autoplay, shuffle, recommendations)
/// are filtered out centrally and never enter `recent_plays` or `recent_play_events`.
pub async fn record_intentional_play(
    pool: &SqlitePool,
    track: &crate::domain::Track,
    origin: crate::domain::PlaybackOrigin,
) -> Result<bool, String> {
    if !origin.is_intentional() {
        return Ok(false);
    }
    record_intentional_play_internal(pool, track, origin).await?;
    Ok(true)
}

/// Remembers that a track actually started playing intentionally. Repeats update
/// the summary row and append a timestamped event row cleanly.
pub async fn record_recent_play(
    pool: &SqlitePool,
    track: &crate::domain::Track,
) -> Result<(), String> {
    record_intentional_play_internal(
        pool,
        track,
        crate::domain::PlaybackOrigin::SearchSelection,
    )
    .await
}

async fn record_intentional_play_internal(
    pool: &SqlitePool,
    track: &crate::domain::Track,
    origin: crate::domain::PlaybackOrigin,
) -> Result<(), String> {
    // If local track, verify and query authoritative data from tracks table and increment play_count
    let (title, artist, album, duration_ms, artwork_uri, source_kind, provider_id) = if track
        .source_kind
        == "local"
        || !track.id.starts_with("ytmusic:")
    {
        let row = sqlx::query(
                "SELECT title, artist, album, duration_ms, artwork_uri FROM tracks WHERE id = ? AND is_deleted = 0",
            )
            .bind(&track.id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to fetch track for history: {e}"))?;

        if let Some(r) = row {
            let _ = sqlx::query("UPDATE tracks SET play_count = play_count + 1 WHERE id = ?")
                .bind(&track.id)
                .execute(pool)
                .await;

            (
                r.get::<String, _>("title"),
                r.get::<String, _>("artist"),
                r.get::<String, _>("album"),
                r.get::<i64, _>("duration_ms"),
                r.get::<Option<String>, _>("artwork_uri"),
                "local".to_string(),
                None,
            )
        } else {
            (
                track.title.clone(),
                track.artist.clone(),
                track.album.clone(),
                track.duration_ms,
                track.artwork_uri.clone(),
                track.source_kind.clone(),
                track.provider_id.clone(),
            )
        }
    } else {
        (
            track.title.clone(),
            track.artist.clone(),
            track.album.clone(),
            track.duration_ms,
            track.artwork_uri.clone(),
            track.source_kind.clone(),
            track.provider_id.clone(),
        )
    };

    let origin_str = origin.as_str();
    sqlx::query(
        r#"
        INSERT INTO recent_plays (
            track_id, source_kind, title, artist, album, duration_ms,
            artwork_uri, provider_id, play_count, last_origin, played_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(track_id) DO UPDATE SET
            source_kind = excluded.source_kind,
            title = excluded.title,
            artist = excluded.artist,
            album = excluded.album,
            duration_ms = excluded.duration_ms,
            artwork_uri = COALESCE(excluded.artwork_uri, recent_plays.artwork_uri),
            provider_id = COALESCE(excluded.provider_id, recent_plays.provider_id),
            play_count = recent_plays.play_count + 1,
            last_origin = excluded.last_origin,
            played_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(&track.id)
    .bind(&source_kind)
    .bind(&title)
    .bind(&artist)
    .bind(&album)
    .bind(duration_ms)
    .bind(&artwork_uri)
    .bind(&provider_id)
    .bind(origin_str)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to record playback history: {e}"))?;

    sqlx::query(
        r#"
        INSERT INTO recent_play_events (
            track_id, source_kind, title, artist, album, duration_ms, playback_origin, played_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&track.id)
    .bind(&source_kind)
    .bind(&title)
    .bind(&artist)
    .bind(&album)
    .bind(duration_ms)
    .bind(origin_str)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to record intentional play event: {e}"))?;

    Ok(())
}

pub async fn get_recent_play_events(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<crate::domain::RecentPlayEvent>, String> {
    let rows = sqlx::query(
        r#"
        SELECT id, track_id, source_kind, title, artist, album, duration_ms, playback_origin, played_at
        FROM recent_play_events
        ORDER BY id DESC
        LIMIT ?
        "#,
    )
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to read playback history events: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|row| crate::domain::RecentPlayEvent {
            id: row.get("id"),
            track_id: row.get("track_id"),
            source_kind: row.get("source_kind"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            duration_ms: row.get("duration_ms"),
            playback_origin: row.get("playback_origin"),
            played_at: row.get("played_at"),
        })
        .collect())
}

/// Most recently played tracks, newest first. Catalogue entries are included
/// with the snapshot they had when they played, while active local rows retain
/// their current `file_uri`, `album_artist`, and `is_liked` state.
pub async fn get_recent_plays(pool: &SqlitePool, limit: i64) -> Result<Vec<Track>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            r.track_id,
            r.source_kind,
            r.title,
            r.artist,
            r.album,
            t.album_artist,
            r.duration_ms,
            COALESCE(t.artwork_uri, r.artwork_uri) AS artwork_uri,
            r.provider_id,
            CASE WHEN t.id IS NOT NULL THEN t.file_uri ELSE NULL END AS file_uri,
            CASE WHEN l.track_id IS NOT NULL THEN 1 ELSE 0 END AS is_liked,
            r.play_count,
            r.played_at
        FROM recent_plays r
        LEFT JOIN tracks t ON r.track_id = t.id AND t.is_deleted = 0
        LEFT JOIN liked_tracks l ON r.track_id = l.track_id
        ORDER BY r.played_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to read playback history: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|row| Track {
            id: row.get("track_id"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            album_artist: row.get("album_artist"),
            duration_ms: row.get("duration_ms"),
            artwork_uri: row.get("artwork_uri"),
            source_kind: row.get("source_kind"),
            provider_id: row.get("provider_id"),
            file_uri: row.get("file_uri"),
            release_year: None,
            track_number: None,
            disc_number: None,
            genre: None,
            is_liked: row.get::<i64, _>("is_liked") == 1,
            play_count: row.get("play_count"),
            date_added: row.get("played_at"),
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        })
        .collect())
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

    let _ = crate::services::sync::outbox::OutboxService::record_track_remove(
        &mut *tx,
        playlist_id,
        &entry_id,
        position,
    )
    .await;

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

    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP, last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {e}"))?;

    Ok(())
}

pub async fn remove_tracks_from_playlist(
    pool: &SqlitePool,
    playlist_id: &str,
    positions: &[i64],
) -> Result<(), String> {
    if positions.is_empty() {
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

    for &pos in positions {
        let entry: Option<(String, i64)> = sqlx::query_as(
            "SELECT id, position FROM playlist_tracks WHERE playlist_id = ? AND position = ?",
        )
        .bind(playlist_id)
        .bind(pos)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Failed to query playlist track: {e}"))?;

        if let Some((entry_id, position)) = entry {
            sqlx::query("DELETE FROM playlist_tracks WHERE id = ?")
                .bind(&entry_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to delete playlist track: {e}"))?;

            let _ = crate::services::sync::outbox::OutboxService::record_track_remove(
                &mut *tx,
                playlist_id,
                &entry_id,
                position,
            )
            .await;
        }
    }

    // Recompact positions: 0, 1, 2...
    let remaining_ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(playlist_id)
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();

    for (new_pos, entry_id) in remaining_ids.into_iter().enumerate() {
        let _ = sqlx::query("UPDATE playlist_tracks SET position = ? WHERE id = ?")
            .bind(new_pos as i64)
            .bind(&entry_id)
            .execute(&mut *tx)
            .await;
    }

    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP, last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit batch removal from playlist: {e}"))?;

    Ok(())
}

const REORDER_ISOLATION_POS: i64 = 1_000_000_000;
const REORDER_SHIFT_OFFSET: i64 = 2_000_000_000;

/// Recompacts `playlist_tracks.position` sequentially (`0..N-1`) inside an open transaction.
pub(crate) async fn recompact_playlist_positions_in_tx(
    tx: &mut sqlx::SqliteConnection,
    playlist_id: &str,
) -> Result<(), String> {
    let remaining_ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(playlist_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("Failed to read playlist order for compaction: {e}"))?;

    if remaining_ids.is_empty() {
        return Ok(());
    }

    // Move out of conflicting index space before writing compact 0..N positions.
    sqlx::query("UPDATE playlist_tracks SET position = position + ? WHERE playlist_id = ?")
        .bind(REORDER_SHIFT_OFFSET)
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to offset positions before compaction: {e}"))?;

    for (new_pos, entry_id) in remaining_ids.into_iter().enumerate() {
        sqlx::query("UPDATE playlist_tracks SET position = ? WHERE id = ?")
            .bind(new_pos as i64)
            .bind(&entry_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to write compacted position: {e}"))?;
    }

    Ok(())
}

/// Applies a playlist track move (`from_pos -> to_pos`) using two-phase collision avoidance.
pub(crate) async fn reorder_playlist_tracks_in_tx(
    tx: &mut sqlx::SqliteConnection,
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
    sqlx::query("UPDATE playlist_tracks SET position = ? WHERE playlist_id = ? AND position = ?")
        .bind(REORDER_ISOLATION_POS)
        .bind(playlist_id)
        .bind(from_pos)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to isolate moving track: {e}"))?;

    // Step 2: Shift intervening tracks using 2-phase disjoint update
    if from_pos < to_pos {
        sqlx::query(
            "UPDATE playlist_tracks SET position = position + ? WHERE playlist_id = ? AND position > ? AND position <= ?",
        )
        .bind(REORDER_SHIFT_OFFSET)
        .bind(playlist_id)
        .bind(from_pos)
        .bind(to_pos)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to shift intervening tracks (phase 2a): {e}"))?;

        sqlx::query(
            "UPDATE playlist_tracks SET position = position - ? WHERE playlist_id = ? AND position >= ? AND position != ?",
        )
        .bind(REORDER_SHIFT_OFFSET + 1)
        .bind(playlist_id)
        .bind(REORDER_SHIFT_OFFSET)
        .bind(REORDER_ISOLATION_POS)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to shift intervening tracks (phase 2b): {e}"))?;
    } else {
        sqlx::query(
            "UPDATE playlist_tracks SET position = position + ? WHERE playlist_id = ? AND position >= ? AND position < ?",
        )
        .bind(REORDER_SHIFT_OFFSET)
        .bind(playlist_id)
        .bind(to_pos)
        .bind(from_pos)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to shift intervening tracks (phase 2a): {e}"))?;

        sqlx::query(
            "UPDATE playlist_tracks SET position = position - ? WHERE playlist_id = ? AND position >= ? AND position != ?",
        )
        .bind(REORDER_SHIFT_OFFSET - 1)
        .bind(playlist_id)
        .bind(REORDER_SHIFT_OFFSET)
        .bind(REORDER_ISOLATION_POS)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to shift intervening tracks (phase 2b): {e}"))?;
    }

    // Step 3: Move isolated track into destination position to_pos
    sqlx::query("UPDATE playlist_tracks SET position = ? WHERE playlist_id = ? AND position = ?")
        .bind(to_pos)
        .bind(playlist_id)
        .bind(REORDER_ISOLATION_POS)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to place moving track at target position: {e}"))?;

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

    reorder_playlist_tracks_in_tx(&mut tx, playlist_id, from_pos, to_pos).await?;

    let _ = crate::services::sync::outbox::OutboxService::record_tracks_reorder(
        &mut tx,
        playlist_id,
        from_pos,
        to_pos,
    )
    .await;

    // Step 4: Update playlists.updated_at
    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP, last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
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

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {e}"))?;

    let result = sqlx::query(
        "UPDATE playlists SET name = ?, description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND is_deleted = 0",
    )
    .bind(trimmed_name)
    .bind(description.trim())
    .bind(playlist_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update playlist: {e}"))?;

    if result.rows_affected() == 0 {
        return Err(format!("Playlist '{playlist_id}' not found or deleted"));
    }

    let _ = crate::services::sync::outbox::OutboxService::record_playlist_update(
        &mut *tx,
        playlist_id,
        trimmed_name,
        description.trim(),
    )
    .await;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit update playlist: {e}"))?;

    let row = sqlx::query(
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.artwork_uri,
            p.source_provider,
            p.is_pinned,
            p.last_used_at,
            p.created_at,
            (SELECT COUNT(*) FROM playlist_tracks pt LEFT JOIN tracks t ON pt.track_id = t.id WHERE pt.playlist_id = p.id AND (t.is_deleted IS NULL OR t.is_deleted = 0)) AS track_count
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
        is_pinned: row.get::<i64, _>("is_pinned") == 1,
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
        tracks: None,
    })
}

/// Lowercased genre terms ("Rock; Pop" -> ["rock", "pop"]). A single tag can
/// hold several genres, so each term counts on its own: a five-genre tag still
/// matches a plain "Pop" track.
fn genre_terms(genre: &str) -> Vec<String> {
    genre
        .split([';', '/', ','])
        .map(|term| term.trim().to_lowercase())
        .filter(|term| !term.is_empty())
        .collect()
}

/// Words that mark a later part of a title as a different version of the same
/// song rather than a different song.
const VERSION_MARKERS: &[&str] = &[
    "remix",
    "sped up",
    "spedup",
    "slowed",
    "nightcore",
    "live",
    "version",
    "edit",
    "instrumental",
    "acoustic",
    "remaster",
    "demo",
    "reverb",
];

fn is_version_marker(segment: &str) -> bool {
    VERSION_MARKERS
        .iter()
        .any(|marker| segment.contains(marker))
}

/// Reduces a title to its base name so near-identical uploads ("Song",
/// "Song (Remix)", "Song - Sped Up") count as the same song and are never
/// chosen as the next track.
pub fn base_song_title(title: &str) -> String {
    let mut text = String::with_capacity(title.len());
    let mut depth = 0i32;
    for ch in title.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => text.push(ch),
            _ => {}
        }
    }
    let without_brackets = text.to_lowercase();
    let mut segments = without_brackets.split(" - ");
    let head = segments.next().unwrap_or("").trim();
    let tail: Vec<&str> = segments.map(str::trim).collect();
    let tail_is_version = !tail.is_empty() && tail.iter().all(|seg| is_version_marker(seg));
    let chosen = if tail_is_version {
        head
    } else {
        without_brackets.as_str()
    };

    chosen
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn primary_artist_tag(artist: &str) -> String {
    artist
        .split([';', ','])
        .next()
        .unwrap_or(artist)
        .trim()
        .to_lowercase()
}

/// Scores how related `candidate` is to `current` using the tags the library
/// actually stores: shared genre terms, then the same artist, then the same
/// album. Returns 0 for the current track and for any track with the same base
/// title, so a near-duplicate upload can never be selected.
pub fn similarity_score(current: &Track, candidate: &Track) -> u32 {
    if candidate.id == current.id {
        return 0;
    }
    let current_title = base_song_title(&current.title);
    if !current_title.is_empty() && current_title == base_song_title(&candidate.title) {
        return 0;
    }

    let mut score = 0u32;

    let current_artist = primary_artist_tag(&current.artist);
    if !current_artist.is_empty() && current_artist == primary_artist_tag(&candidate.artist) {
        score += 8;
    }

    if !current.album.trim().is_empty()
        && current
            .album
            .trim()
            .eq_ignore_ascii_case(candidate.album.trim())
    {
        score += 4;
    }

    let current_terms = genre_terms(current.genre.as_deref().unwrap_or(""));
    if !current_terms.is_empty() {
        let mut genre_matches = 0u32;
        for term in genre_terms(candidate.genre.as_deref().unwrap_or("")) {
            if current_terms.contains(&term) {
                genre_matches += 1;
            }
        }
        score += (genre_matches * 3).min(6);
    }

    score
}

/// Picks the next library track to play after `current_id` ends, using the tags
/// of the finished song. Returns `None` when nothing in the library shares a
/// genre, artist or album with it: stopping is more honest than playing an
/// unrelated song.
pub async fn suggest_next_track(
    pool: &SqlitePool,
    current_id: &str,
) -> Result<Option<Track>, String> {
    let current_row = sqlx::query(
        "SELECT id, title, artist, album, genre, play_count FROM tracks WHERE id = ? AND is_deleted = 0"
    )
    .bind(current_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Database error fetching current track: {e}"))?;

    let Some(cur_row) = current_row else {
        return Ok(None);
    };

    let current = Track {
        id: cur_row.get("id"),
        title: cur_row.get("title"),
        artist: cur_row.get("artist"),
        album: cur_row.get("album"),
        album_artist: None,
        duration_ms: 0,
        artwork_uri: None,
        source_kind: "local".to_string(),
        provider_id: None,
        file_uri: None,
        release_year: None,
        track_number: None,
        disc_number: None,
        genre: cur_row.get("genre"),
        is_liked: false,
        play_count: cur_row.get("play_count"),
        date_added: String::new(),
        queue_entry_id: None,
        contributor_id: None,
        contributor_name: None,
        priority_override: None,
    };

    let recent_ids: Vec<String> =
        sqlx::query_scalar("SELECT track_id FROM recent_plays ORDER BY played_at DESC LIMIT 10")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    let rows = sqlx::query(
        r#"
        SELECT
            id, source_kind, title, artist, album, album_artist,
            duration_ms, artwork_uri, file_uri, provider_id,
            release_year, track_number, disc_number, genre,
            play_count, date_added
        FROM tracks
        WHERE is_deleted = 0
          AND id != ?
          AND (
              artist = ?
              OR (album = ? AND album != '')
              OR (genre IS NOT NULL AND genre != '')
          )
        ORDER BY play_count ASC
        LIMIT 250
        "#,
    )
    .bind(current_id)
    .bind(&current.artist)
    .bind(&current.album)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch candidates: {e}"))?;

    let candidates: Vec<Track> = rows
        .into_iter()
        .map(|row| Track {
            id: row.get("id"),
            source_kind: row.get("source_kind"),
            title: row.get("title"),
            artist: row.get("artist"),
            album: row.get("album"),
            album_artist: row.get("album_artist"),
            duration_ms: row.get("duration_ms"),
            artwork_uri: row.get("artwork_uri"),
            file_uri: row.get("file_uri"),
            provider_id: row.get("provider_id"),
            release_year: row.get("release_year"),
            track_number: row.get("track_number"),
            disc_number: row.get("disc_number"),
            genre: row.get("genre"),
            is_liked: false,
            play_count: row.get("play_count"),
            date_added: row.get("date_added"),
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        })
        .collect();

    let mut best: Option<(&Track, u32)> = None;
    for candidate in &candidates {
        let mut score = similarity_score(&current, candidate);
        if score == 0 {
            continue;
        }

        if let Some(pos) = recent_ids.iter().position(|id| id == &candidate.id) {
            let penalty = (10 - pos as u32).min(score);
            score = score.saturating_sub(penalty);
            if score == 0 {
                continue;
            }
        }

        let better = match best {
            None => true,
            Some((best_track, best_score)) => {
                score > best_score
                    || (score == best_score
                        && (candidate.play_count < best_track.play_count
                            || (candidate.play_count == best_track.play_count
                                && candidate.title.to_lowercase()
                                    < best_track.title.to_lowercase())))
            }
        };
        if better {
            best = Some((candidate, score));
        }
    }

    Ok(best.map(|(track, _)| track.clone()))
}

/// Soft-deletes a track from the local library (`is_deleted = 1`).
/// Also cleans up any associations in `liked_tracks`, `recent_plays`, and `playlist_tracks`,
/// and compacts the playlist positions for any affected playlists.
/// Note: The actual audio file on the user's filesystem is NEVER touched or deleted.
/// Soft-deletes multiple tracks from the local library (`is_deleted = 1`) in a single transaction.
/// Also cleans up any associations in `liked_tracks`, `recent_plays`, and `playlist_tracks`,
/// and compacts the playlist positions for any affected playlists.
/// Note: The actual audio files on the user's filesystem are NEVER touched or deleted.
pub async fn delete_tracks(pool: &SqlitePool, track_ids: &[String]) -> Result<usize, String> {
    if track_ids.is_empty() {
        return Ok(0);
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {e}"))?;

    let mut deleted_count = 0;
    let mut all_affected_playlists = std::collections::HashSet::new();

    for track_id in track_ids {
        let trk = sqlx::query(
            "SELECT title, artist, album, duration_ms, content_hash FROM tracks WHERE id = ? AND is_deleted = 0",
        )
        .bind(track_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Failed to query track: {e}"))?;

        let Some(r) = trk else {
            continue;
        };

        let title = r.get::<String, _>("title");
        let artist = r.get::<String, _>("artist");
        let album = r.get::<String, _>("album");
        let duration_ms = r.get::<i64, _>("duration_ms");
        let content_hash = r.get::<Option<String>, _>("content_hash");

        // Soft delete track
        sqlx::query("UPDATE tracks SET is_deleted = 1 WHERE id = ?")
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to soft-delete track: {e}"))?;

        deleted_count += 1;

        // Check if was liked, and if so record unlike in outbox
        let was_liked = sqlx::query("SELECT 1 FROM liked_tracks WHERE track_id = ?")
            .bind(track_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Failed to check liked tracks: {e}"))?
            .is_some();

        if was_liked {
            sqlx::query("DELETE FROM liked_tracks WHERE track_id = ?")
                .bind(track_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to remove track from liked_tracks: {e}"))?;

            let _ = crate::services::sync::outbox::OutboxService::record_track_like_status(
                &mut *tx,
                track_id,
                content_hash.as_deref(),
                &title,
                &artist,
                &album,
                duration_ms,
                false,
            )
            .await;
        }

        // Remove from recent plays
        sqlx::query("DELETE FROM recent_plays WHERE track_id = ?")
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to remove from recent plays: {e}"))?;

        // Find affected playlist entries so sync outbox records every removal
        let affected_entries: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT id, playlist_id, position FROM playlist_tracks WHERE track_id = ?",
        )
        .bind(track_id)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        // Remove from playlist_tracks
        sqlx::query("DELETE FROM playlist_tracks WHERE track_id = ?")
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to remove track from playlists: {e}"))?;

        for (entry_id, pid, pos) in affected_entries {
            all_affected_playlists.insert(pid.clone());
            let _ = crate::services::sync::outbox::OutboxService::record_track_remove(
                &mut *tx, &pid, &entry_id, pos,
            )
            .await;
        }
    }

    // Recompact positions for all affected playlists once
    for pid in &all_affected_playlists {
        let entry_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
        )
        .bind(pid)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        for (new_pos, entry_id) in entry_ids.into_iter().enumerate() {
            let _ = sqlx::query("UPDATE playlist_tracks SET position = ? WHERE id = ?")
                .bind(new_pos as i64)
                .bind(&entry_id)
                .execute(&mut *tx)
                .await;
        }

        let _ = sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(pid)
            .execute(&mut *tx)
            .await;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit track deletions: {e}"))?;

    Ok(deleted_count)
}

/// Soft-deletes a single track from the local library (`is_deleted = 1`).
pub async fn delete_track(pool: &SqlitePool, track_id: &str) -> Result<(), String> {
    delete_tracks(pool, &[track_id.to_string()])
        .await
        .map(|_| ())
}

/// Removes all tracks from a playlist without deleting the playlist itself.
pub async fn clear_playlist_tracks(pool: &SqlitePool, playlist_id: &str) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {e}"))?;

    let playlist_exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM playlists WHERE id = ? AND is_deleted = 0")
            .bind(playlist_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Failed to verify playlist: {e}"))?;

    if playlist_exists.is_none() {
        return Err(format!("Playlist '{playlist_id}' not found or deleted"));
    }

    let removed_entries: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(playlist_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("Failed to list playlist tracks for clearing: {e}"))?;

    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to clear playlist tracks: {e}"))?;

    for (entry_id, position) in removed_entries {
        let _ = crate::services::sync::outbox::OutboxService::record_track_remove(
            &mut *tx,
            playlist_id,
            &entry_id,
            position,
        )
        .await;
    }

    sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP, last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update playlist timestamp: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit clear playlist: {e}"))?;

    Ok(())
}

pub async fn delete_recent_play(pool: &SqlitePool, track_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM recent_plays WHERE track_id = ?")
        .bind(track_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete recent play: {e}"))?;
    let _ = sqlx::query("DELETE FROM recent_play_events WHERE track_id = ?")
        .bind(track_id)
        .execute(pool)
        .await;
    Ok(())
}

pub async fn clear_recent_plays(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query("DELETE FROM recent_plays")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to clear recent plays: {e}"))?;
    let _ = sqlx::query("DELETE FROM recent_play_events")
        .execute(pool)
        .await;
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

    fn empty_track(id: &str, title: &str, artist: &str, album: &str, genre: Option<&str>) -> Track {
        Track {
            id: id.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            album_artist: None,
            duration_ms: 200_000,
            artwork_uri: None,
            source_kind: "local".to_string(),
            provider_id: None,
            file_uri: None,
            release_year: None,
            track_number: None,
            disc_number: None,
            genre: genre.map(str::to_string),
            is_liked: false,
            play_count: 0,
            date_added: String::new(),
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        }
    }

    #[test]
    fn similarity_prefers_shared_genre_terms() {
        let current = empty_track("a", "Instant Crush", "Daft Punk", "RAM", Some("Rock; Pop"));
        let shared_genre = empty_track("b", "Nightcall", "Kavinsky", "OutRun", Some("Pop"));
        let same_artist = empty_track("c", "Get Lucky", "Daft Punk", "RAM", None);
        let unrelated = empty_track("d", "Ambient Piece", "Someone Else", "Other", None);

        assert!(similarity_score(&current, &shared_genre) > similarity_score(&current, &unrelated));
        assert!(similarity_score(&current, &same_artist) > similarity_score(&current, &unrelated));
        assert_eq!(similarity_score(&current, &unrelated), 0);
        assert_eq!(similarity_score(&current, &current), 0);
    }

    #[test]
    fn similarity_rejects_near_identical_titles() {
        let current = empty_track("a", "Die For You", "VALORANT", "Die For You", Some("Pop"));
        let remix = empty_track(
            "b",
            "Die For You (Remix)",
            "VALORANT",
            "Die For You",
            Some("Pop"),
        );
        let sped_up = empty_track(
            "c",
            "Die For You - Sped Up",
            "VALORANT",
            "Die For You",
            Some("Pop"),
        );

        assert_eq!(similarity_score(&current, &remix), 0);
        assert_eq!(similarity_score(&current, &sped_up), 0);
    }

    #[tokio::test]
    async fn suggest_next_track_uses_tags_and_never_repeats_the_name() {
        let pool = create_test_db().await;
        for (id, title, artist, album, genre) in [
            ("seed", "Die For You", "VALORANT", "Die For You", "Pop"),
            (
                "dup",
                "Die For You (Remix)",
                "VALORANT",
                "Die For You",
                "Pop",
            ),
            ("pop", "Sugar", "Maroon 5", "V", "Pop"),
            ("rock", "Bones", "Imagine Dragons", "Mercury", "Rock"),
        ] {
            sqlx::query(
                "INSERT INTO tracks (id, source_kind, title, artist, album, genre) VALUES (?, 'local', ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(title)
            .bind(artist)
            .bind(album)
            .bind(genre)
            .execute(&pool)
            .await
            .unwrap();
        }

        let next = suggest_next_track(&pool, "seed").await.unwrap();
        assert_eq!(next.map(|track| track.id), Some("pop".to_string()));
    }

    #[tokio::test]
    async fn suggest_next_track_returns_none_without_related_tags() {
        let pool = create_test_db().await;
        for (id, title, artist, album, genre) in [
            ("seed", "Sea Song", "A", "One", "Ambient"),
            ("other", "Metal Song", "B", "Two", "Metal"),
        ] {
            sqlx::query(
                "INSERT INTO tracks (id, source_kind, title, artist, album, genre) VALUES (?, 'local', ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(title)
            .bind(artist)
            .bind(album)
            .bind(genre)
            .execute(&pool)
            .await
            .unwrap();
        }

        assert!(suggest_next_track(&pool, "seed").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_playlist_lifecycle() {
        let pool = create_test_db().await;
        let playlist = create_playlist(&pool, "Imported", "Imported from Spotify")
            .await
            .unwrap();

        add_provider_track_to_playlist(
            &pool,
            &playlist.id,
            "youtube_music",
            "dQw4w9WgXcQ",
            "Never Gonna Give You Up",
            "Rick Astley",
            "Whenever You Need Somebody",
            213_000,
            Some("https://example.test/cover.jpg"),
        )
        .await
        .unwrap();

        let tracks = get_playlist_tracks(&pool, &playlist.id).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "ytmusic:dQw4w9WgXcQ");
        assert_eq!(tracks[0].source_kind, "provider");
        assert_eq!(tracks[0].provider_id.as_deref(), Some("youtube_music"));
        assert_eq!(tracks[0].title, "Never Gonna Give You Up");
        assert_eq!(
            tracks[0].artwork_uri.as_deref(),
            Some("https://example.test/cover.jpg")
        );
    }

    #[tokio::test]
    async fn test_find_local_track_id_requires_artist_overlap() {
        let pool = create_test_db().await;
        sqlx::query(
            "INSERT INTO tracks (id, source_kind, title, artist, album) VALUES ('t1', 'local', 'Instant Crush', 'Daft Punk', 'Random Access Memories')",
        )
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            find_local_track_id(&pool, "Instant Crush", "Daft Punk, Julian Casablancas").await,
            Some("t1".to_string())
        );
        assert_eq!(
            find_local_track_id(&pool, "Instant Crush", "Some Cover Band").await,
            None
        );
    }

    #[tokio::test]
    async fn test_recent_plays_records_and_dedupes() {
        let pool = create_test_db().await;
        let track = crate::domain::Track {
            id: "ytmusic:dQw4w9WgXcQ".to_string(),
            title: "Never Gonna Give You Up".to_string(),
            artist: "Rick Astley".to_string(),
            album: String::new(),
            album_artist: None,
            duration_ms: 213_000,
            artwork_uri: Some("https://example.test/cover.jpg".to_string()),
            source_kind: "provider".to_string(),
            provider_id: Some("youtube_music".to_string()),
            file_uri: None,
            release_year: None,
            track_number: None,
            disc_number: None,
            genre: None,
            is_liked: false,
            play_count: 0,
            date_added: String::new(),
            queue_entry_id: None,
            contributor_id: None,
            contributor_name: None,
            priority_override: None,
        };

        record_recent_play(&pool, &track).await.unwrap();
        record_recent_play(&pool, &track).await.unwrap();

        let rows = get_recent_plays(&pool, 10).await.unwrap();
        assert_eq!(rows.len(), 1, "a replay must update, not duplicate");
        assert_eq!(rows[0].id, "ytmusic:dQw4w9WgXcQ");
        assert_eq!(rows[0].source_kind, "provider");
        assert_eq!(rows[0].play_count, 2);
        assert_eq!(
            rows[0].artwork_uri.as_deref(),
            Some("https://example.test/cover.jpg")
        );
    }

    #[tokio::test]
    async fn test_playlist_lifecycle_original() {
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

    #[tokio::test]
    async fn test_delete_track() {
        let pool = create_test_db().await;
        insert_dummy_track(&pool, "t1", "Track 1").await;
        insert_dummy_track(&pool, "t2", "Track 2").await;

        toggle_track_liked(&pool, "t1").await.unwrap();
        assert_eq!(get_liked_tracks(&pool).await.unwrap().len(), 1);

        let trk1 = empty_track("t1", "Track 1", "Artist", "Album", None);
        record_recent_play(&pool, &trk1).await.unwrap();
        assert_eq!(get_recent_plays(&pool, 10).await.unwrap().len(), 1);

        let pl = create_playlist(&pool, "Test PL", "Desc").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t1").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t2").await.unwrap();
        assert_eq!(get_playlist_tracks(&pool, &pl.id).await.unwrap().len(), 2);

        delete_track(&pool, "t1").await.unwrap();

        let all_tracks = get_all_tracks(&pool).await.unwrap();
        assert_eq!(all_tracks.len(), 1);
        assert_eq!(all_tracks[0].id, "t2");

        assert_eq!(get_liked_tracks(&pool).await.unwrap().len(), 0);
        assert_eq!(get_recent_plays(&pool, 10).await.unwrap().len(), 0);

        let pl_tracks = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        assert_eq!(pl_tracks.len(), 1);
        assert_eq!(pl_tracks[0].id, "t2");
    }

    #[tokio::test]
    async fn test_clear_playlist_tracks() {
        let pool = create_test_db().await;
        insert_dummy_track(&pool, "t1", "Track 1").await;
        insert_dummy_track(&pool, "t2", "Track 2").await;

        let pl = create_playlist(&pool, "My Mix", "Desc").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t1").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t2").await.unwrap();
        assert_eq!(get_playlist_tracks(&pool, &pl.id).await.unwrap().len(), 2);

        clear_playlist_tracks(&pool, &pl.id).await.unwrap();
        assert_eq!(get_playlist_tracks(&pool, &pl.id).await.unwrap().len(), 0);

        let playlists = get_playlists(&pool).await.unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].id, pl.id);
    }

    #[tokio::test]
    async fn test_recent_plays_deletion() {
        let pool = create_test_db().await;
        let trk1 = empty_track("t1", "Track 1", "Artist", "Album", None);
        let trk2 = empty_track("t2", "Track 2", "Artist", "Album", None);

        record_recent_play(&pool, &trk1).await.unwrap();
        record_recent_play(&pool, &trk2).await.unwrap();
        assert_eq!(get_recent_plays(&pool, 10).await.unwrap().len(), 2);

        delete_recent_play(&pool, "t1").await.unwrap();
        let recents = get_recent_plays(&pool, 10).await.unwrap();
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0].id, "t2");

        clear_recent_plays(&pool).await.unwrap();
        assert_eq!(get_recent_plays(&pool, 10).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_delete_tracks_batch() {
        let pool = create_test_db().await;
        insert_dummy_track(&pool, "t1", "Track 1").await;
        insert_dummy_track(&pool, "t2", "Track 2").await;
        insert_dummy_track(&pool, "t3", "Track 3").await;

        let pl = create_playlist(&pool, "Batch PL", "Desc").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t1").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t2").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t3").await.unwrap();

        let count = delete_tracks(&pool, &["t1".to_string(), "t3".to_string()])
            .await
            .unwrap();
        assert_eq!(count, 2);

        let remaining = get_all_tracks(&pool).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "t2");

        let pl_tracks = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        assert_eq!(pl_tracks.len(), 1);
        assert_eq!(pl_tracks[0].id, "t2");
    }

    #[tokio::test]
    async fn test_unlike_tracks_batch() {
        let pool = create_test_db().await;
        insert_dummy_track(&pool, "t1", "Track 1").await;
        insert_dummy_track(&pool, "t2", "Track 2").await;
        insert_dummy_track(&pool, "t3", "Track 3").await;

        toggle_track_liked(&pool, "t1").await.unwrap();
        toggle_track_liked(&pool, "t2").await.unwrap();
        toggle_track_liked(&pool, "t3").await.unwrap();

        let liked = get_liked_tracks(&pool).await.unwrap();
        assert_eq!(liked.len(), 3);

        let unliked_count = unlike_tracks(&pool, &["t1".to_string(), "t3".to_string()])
            .await
            .unwrap();
        assert_eq!(unliked_count, 2);

        let remaining_liked = get_liked_tracks(&pool).await.unwrap();
        assert_eq!(remaining_liked.len(), 1);
        assert_eq!(remaining_liked[0].id, "t2");
    }

    #[tokio::test]
    async fn test_remove_tracks_from_playlist_batch() {
        let pool = create_test_db().await;
        insert_dummy_track(&pool, "t1", "Track 1").await;
        insert_dummy_track(&pool, "t2", "Track 2").await;
        insert_dummy_track(&pool, "t3", "Track 3").await;
        insert_dummy_track(&pool, "t4", "Track 4").await;

        let pl = create_playlist(&pool, "Batch PL", "Desc").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t1").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t2").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t3").await.unwrap();
        add_track_to_playlist(&pool, &pl.id, "t4").await.unwrap();

        // Remove positions 0 (t1) and 2 (t3). Remaining should be t2 (pos 0) and t4 (pos 1).
        remove_tracks_from_playlist(&pool, &pl.id, &[0, 2])
            .await
            .unwrap();

        let pl_tracks = get_playlist_tracks(&pool, &pl.id).await.unwrap();
        assert_eq!(pl_tracks.len(), 2);
        assert_eq!(pl_tracks[0].id, "t2");
        assert_eq!(pl_tracks[1].id, "t4");
    }
}
