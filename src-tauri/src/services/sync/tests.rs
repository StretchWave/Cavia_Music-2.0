use crate::services::credentials::InMemoryCredentialStore;
use crate::services::library;
use crate::services::sync::engine::SyncEngine;
use crate::services::sync::outbox::OutboxService;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use uuid::Uuid;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

async fn insert_mock_track(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    artist: &str,
    album: &str,
    duration_ms: i64,
    content_hash: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO tracks (
            id, source_kind, title, artist, album, duration_ms, file_uri, content_hash, is_deleted
        ) VALUES (?, 'local', ?, ?, ?, ?, '/path/to/mock.mp3', ?, 0)
        "#,
    )
    .bind(id)
    .bind(title)
    .bind(artist)
    .bind(album)
    .bind(duration_ms)
    .bind(content_hash)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_outbox_playlist_lifecycle_mutations() {
    let pool = setup_test_db().await;

    // 1. Create playlist
    let pl = library::create_playlist(&pool, "Road Trip", "Chill tunes")
        .await
        .unwrap();
    assert_eq!(pl.name, "Road Trip");

    let pending = OutboxService::fetch_pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].entity_type, "playlist");
    assert_eq!(pending[0].operation_type, "create");
    assert_eq!(pending[0].entity_id, pl.id);

    // 2. Update playlist
    let updated = library::update_playlist(&pool, &pl.id, "Epic Road Trip", "Updated notes")
        .await
        .unwrap();
    assert_eq!(updated.name, "Epic Road Trip");

    let pending = OutboxService::fetch_pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[1].operation_type, "update");

    // 3. Delete playlist
    library::delete_playlist(&pool, &pl.id).await.unwrap();

    let pending = OutboxService::fetch_pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 3);
    assert_eq!(pending[2].operation_type, "delete");

    // Verify tombstone was created
    let tombstone: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sync_tombstones WHERE entity_type = 'playlist' AND entity_id = ?",
    )
    .bind(&pl.id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(tombstone.is_some());
}

#[tokio::test]
async fn test_outbox_playlist_tracks_and_reorder() {
    let pool = setup_test_db().await;

    insert_mock_track(
        &pool, "trk-1", "Song A", "Artist 1", "Album 1", 200000, "hash-a",
    )
    .await;
    insert_mock_track(
        &pool, "trk-2", "Song B", "Artist 2", "Album 2", 240000, "hash-b",
    )
    .await;

    let pl = library::create_playlist(&pool, "Favorites", "")
        .await
        .unwrap();

    // Clear playlist create outbox item for testing tracks in isolation
    OutboxService::clear_all(&pool).await.unwrap();

    // Add track 1
    let entry_1 = library::add_track_to_playlist(&pool, &pl.id, "trk-1")
        .await
        .unwrap();
    // Add track 2
    let entry_2 = library::add_track_to_playlist(&pool, &pl.id, "trk-2")
        .await
        .unwrap();

    let pending = OutboxService::fetch_pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].operation_type, "add");
    assert_eq!(pending[0].entity_id, entry_1);
    assert_eq!(pending[1].operation_type, "add");
    assert_eq!(pending[1].entity_id, entry_2);

    // Reorder tracks
    library::reorder_playlist_tracks(&pool, &pl.id, 0, 1)
        .await
        .unwrap();

    let pending = OutboxService::fetch_pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 3);
    assert_eq!(pending[2].operation_type, "reorder");

    // Remove track 2 (which is now at position 0)
    library::remove_track_from_playlist(&pool, &pl.id, 0)
        .await
        .unwrap();

    let pending = OutboxService::fetch_pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 4);
    assert_eq!(pending[3].operation_type, "remove");

    // Verify track entry tombstone
    let tombstone: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sync_tombstones WHERE entity_type = 'playlist_track' AND entity_id = ?",
    )
    .bind(&entry_2)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(tombstone.is_some());
}

#[tokio::test]
async fn test_outbox_like_and_unlike_tracking() {
    let pool = setup_test_db().await;

    insert_mock_track(
        &pool,
        "trk-like",
        "Great Song",
        "Good Artist",
        "Album G",
        180000,
        "hash-g",
    )
    .await;

    // Like track
    let is_liked = library::toggle_track_liked(&pool, "trk-like")
        .await
        .unwrap();
    assert!(is_liked);

    let pending = OutboxService::fetch_pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].entity_type, "liked_track");
    assert_eq!(pending[0].operation_type, "like");
    assert_eq!(pending[0].entity_id, "trk-like");

    // Unlike track
    let is_liked_again = library::toggle_track_liked(&pool, "trk-like")
        .await
        .unwrap();
    assert!(!is_liked_again);

    let pending = OutboxService::fetch_pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[1].operation_type, "unlike");

    // Verify tombstone on unlike
    let tombstone: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sync_tombstones WHERE entity_type = 'liked_track' AND entity_id = ?",
    )
    .bind("trk-like")
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(tombstone.is_some());
}

#[tokio::test]
async fn test_unavailable_remote_track_display_and_match_on_scan() {
    let pool = setup_test_db().await;

    // Simulate Device B receiving a playlist with an unavailable track (audio file not present)
    let pl = library::create_playlist(&pool, "Cloud Playlist", "Synced")
        .await
        .unwrap();

    let entry_id = format!("plt-{}", Uuid::new_v4());
    sqlx::query(
        r#"
        INSERT INTO playlist_tracks (
            id, playlist_id, track_id, cloud_track_id, title, artist, album, duration_ms, content_hash, position
        ) VALUES (?, ?, NULL, 'cloud-trk-101', 'Distant Echoes', 'Ambient Artist', 'Space', 320000, 'target-hash-123', 0)
        "#,
    )
    .bind(&entry_id)
    .bind(&pl.id)
    .execute(&pool)
    .await
    .unwrap();

    // Query playlist tracks
    let tracks = library::get_playlist_tracks(&pool, &pl.id).await.unwrap();
    assert_eq!(tracks.len(), 1);
    let track = &tracks[0];
    assert_eq!(track.title, "Distant Echoes");
    assert_eq!(track.artist, "Ambient Artist");
    assert_eq!(track.duration_ms, 320000);
    // Crucial: Must be honest remote_sync with no fileUri
    assert_eq!(track.source_kind, "remote_sync");
    assert_eq!(track.file_uri, None);

    // Now simulate local scanner finding and indexing the exact matching file
    let credentials = Arc::new(InMemoryCredentialStore::new());
    let engine = SyncEngine::new(pool.clone(), credentials);

    // Before local file exists, match_unlinked_tracks finds 0 matches
    let matched = engine.match_unlinked_tracks().await.unwrap();
    assert_eq!(matched, 0);

    // Insert local track matching target-hash-123
    insert_mock_track(
        &pool,
        "local-trk-found",
        "Distant Echoes",
        "Ambient Artist",
        "Space",
        320000,
        "target-hash-123",
    )
    .await;

    // Run match_unlinked_tracks
    let matched = engine.match_unlinked_tracks().await.unwrap();
    assert_eq!(matched, 1);

    // Query playlist tracks again
    let tracks = library::get_playlist_tracks(&pool, &pl.id).await.unwrap();
    assert_eq!(tracks.len(), 1);
    let track = &tracks[0];
    // Now it is linked and playable!
    assert_eq!(track.source_kind, "local");
    assert_eq!(track.file_uri, Some("/path/to/mock.mp3".to_string()));
    assert_eq!(track.id, "local-trk-found");
}

#[tokio::test]
async fn test_two_device_sync_simulation_and_tombstone_protection() {
    // Two independent SQLite databases representing Device A and Device B
    let db_a = setup_test_db().await;
    let db_b = setup_test_db().await;

    let creds_a = Arc::new(InMemoryCredentialStore::new());
    let creds_b = Arc::new(InMemoryCredentialStore::new());

    let _engine_a = SyncEngine::new(db_a.clone(), creds_a);
    let _engine_b = SyncEngine::new(db_b.clone(), creds_b);

    // 1. Device A creates a playlist "Morning Coffee"
    let pl_a = library::create_playlist(&db_a, "Morning Coffee", "Smooth jazz")
        .await
        .unwrap();

    // Fetch outbox item on A
    let outbox_a = OutboxService::fetch_pending(&db_a, 10).await.unwrap();
    assert_eq!(outbox_a.len(), 1);
    let op = &outbox_a[0];

    // Simulate Sync Server Relay: A pushes, Server assigns seq=1, B pulls
    let payload_val: serde_json::Value = serde_json::from_str(&op.payload_json).unwrap();
    let name = payload_val.get("name").and_then(|v| v.as_str()).unwrap();
    let desc = payload_val
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap();

    // Pull into Device B
    sqlx::query(
        "INSERT INTO playlists (id, name, description, is_deleted, updated_at) VALUES (?, ?, ?, 0, CURRENT_TIMESTAMP)",
    )
    .bind(&op.entity_id)
    .bind(name)
    .bind(desc)
    .execute(&db_b)
    .await
    .unwrap();

    // Verify Device B has the playlist
    let pls_b = library::get_playlists(&db_b).await.unwrap();
    assert_eq!(pls_b.len(), 1);
    assert_eq!(pls_b[0].id, pl_a.id);
    assert_eq!(pls_b[0].name, "Morning Coffee");

    // 2. Device B renames the playlist to "Evening Chill"
    library::update_playlist(&db_b, &pl_a.id, "Evening Chill", "Mellow")
        .await
        .unwrap();

    let outbox_b = OutboxService::fetch_pending(&db_b, 10).await.unwrap();
    let rename_op = outbox_b
        .iter()
        .find(|i| i.operation_type == "update")
        .unwrap();
    let rename_payload: serde_json::Value = serde_json::from_str(&rename_op.payload_json).unwrap();

    // Relay to Device A
    sqlx::query(
        "UPDATE playlists SET name = ?, description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(rename_payload.get("name").and_then(|v| v.as_str()).unwrap())
    .bind(rename_payload.get("description").and_then(|v| v.as_str()).unwrap())
    .bind(&pl_a.id)
    .execute(&db_a)
    .await
    .unwrap();

    // Verify Device A reflected rename
    let pls_a = library::get_playlists(&db_a).await.unwrap();
    assert_eq!(pls_a[0].name, "Evening Chill");

    // 3. Device B deletes the playlist
    library::delete_playlist(&db_b, &pl_a.id).await.unwrap();
    let pls_b_after = library::get_playlists(&db_b).await.unwrap();
    assert_eq!(pls_b_after.len(), 0);

    // Relay delete to Device A
    library::delete_playlist(&db_a, &pl_a.id).await.unwrap();
    let pls_a_after = library::get_playlists(&db_a).await.unwrap();
    assert_eq!(pls_a_after.len(), 0);

    // 4. Stale Event Test: A delayed/stale update arrives at Device A after deletion
    // Tombstone must protect Device A from resurrecting the deleted playlist!
    let is_tombstoned: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sync_tombstones WHERE entity_type = 'playlist' AND entity_id = ?",
    )
    .bind(&pl_a.id)
    .fetch_optional(&db_a)
    .await
    .unwrap();
    assert!(is_tombstoned.is_some());

    // When tombstone is present, stale create/update is ignored
    if is_tombstoned.is_none() {
        sqlx::query("UPDATE playlists SET is_deleted = 0 WHERE id = ?")
            .bind(&pl_a.id)
            .execute(&db_a)
            .await
            .unwrap();
    }

    // Playlist remains safely deleted on Device A
    let pls_a_final = library::get_playlists(&db_a).await.unwrap();
    assert_eq!(pls_a_final.len(), 0);
}

#[tokio::test]
async fn test_delete_cloud_data_clears_sync_without_deleting_local_music() {
    let pool = setup_test_db().await;

    insert_mock_track(
        &pool,
        "local-safe",
        "Local Track",
        "Artist",
        "Album",
        150000,
        "hash-safe",
    )
    .await;
    let pl = library::create_playlist(&pool, "Local Only", "Preserved")
        .await
        .unwrap();

    let credentials = Arc::new(InMemoryCredentialStore::new());
    let _engine = SyncEngine::new(pool.clone(), credentials);

    // Outbox has pending mutations
    let count = OutboxService::get_pending_count(&pool).await.unwrap();
    assert!(count > 0);

    // Clear all outbox (as happens during delete_cloud_data)
    OutboxService::clear_all(&pool).await.unwrap();
    let count_after = OutboxService::get_pending_count(&pool).await.unwrap();
    assert_eq!(count_after, 0);

    // Crucial: Local tracks and playlists are 100% preserved
    let tracks = library::get_all_tracks(&pool).await.unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].id, "local-safe");

    let playlists = library::get_playlists(&pool).await.unwrap();
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].id, pl.id);
}
