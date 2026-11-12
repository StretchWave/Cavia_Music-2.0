use super::types::{
    LikedTrackPayload, PlaylistMutationPayload, PlaylistTrackAddPayload,
    PlaylistTrackRemovePayload, PlaylistTracksReorderPayload, SyncOutboxItem,
};
use sqlx::{Row, SqliteConnection, SqlitePool};
use uuid::Uuid;

pub struct OutboxService;

impl OutboxService {
    pub async fn record_playlist_create(
        conn: &mut SqliteConnection,
        playlist_id: &str,
        name: &str,
        description: &str,
    ) -> Result<String, String> {
        let op_id = format!("op-{}", Uuid::new_v4());
        let payload = PlaylistMutationPayload {
            id: playlist_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            client_updated_at: chrono_now_iso(),
        };
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize playlist create payload: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO sync_outbox (operation_id, entity_type, entity_id, operation_type, payload_json, status)
            VALUES (?, 'playlist', ?, 'create', ?, 'pending')
            "#,
        )
        .bind(&op_id)
        .bind(playlist_id)
        .bind(&payload_json)
        .execute(conn)
        .await
        .map_err(|e| format!("Failed to insert outbox create playlist: {e}"))?;

        Ok(op_id)
    }

    pub async fn record_playlist_update(
        conn: &mut SqliteConnection,
        playlist_id: &str,
        name: &str,
        description: &str,
    ) -> Result<String, String> {
        let op_id = format!("op-{}", Uuid::new_v4());
        let payload = PlaylistMutationPayload {
            id: playlist_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            client_updated_at: chrono_now_iso(),
        };
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize playlist update payload: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO sync_outbox (operation_id, entity_type, entity_id, operation_type, payload_json, status)
            VALUES (?, 'playlist', ?, 'update', ?, 'pending')
            "#,
        )
        .bind(&op_id)
        .bind(playlist_id)
        .bind(&payload_json)
        .execute(conn)
        .await
        .map_err(|e| format!("Failed to insert outbox update playlist: {e}"))?;

        Ok(op_id)
    }

    pub async fn record_playlist_delete(
        conn: &mut SqliteConnection,
        playlist_id: &str,
    ) -> Result<String, String> {
        let op_id = format!("op-{}", Uuid::new_v4());
        let payload = serde_json::json!({
            "id": playlist_id,
            "deletedAt": chrono_now_iso()
        });
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize playlist delete payload: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO sync_outbox (operation_id, entity_type, entity_id, operation_type, payload_json, status)
            VALUES (?, 'playlist', ?, 'delete', ?, 'pending')
            "#,
        )
        .bind(&op_id)
        .bind(playlist_id)
        .bind(&payload_json)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to insert outbox delete playlist: {e}"))?;

        // Also record durable tombstone
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO sync_tombstones (entity_type, entity_id, deleted_at)
            VALUES ('playlist', ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(playlist_id)
        .execute(conn)
        .await
        .map_err(|e| format!("Failed to insert playlist tombstone: {e}"))?;

        Ok(op_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_track_add(
        conn: &mut SqliteConnection,
        playlist_id: &str,
        entry_id: &str,
        cloud_track_id: &str,
        title: &str,
        artist: &str,
        album: &str,
        duration_ms: i64,
        content_hash: Option<&str>,
        position: i64,
    ) -> Result<String, String> {
        let op_id = format!("op-{}", Uuid::new_v4());
        let payload = PlaylistTrackAddPayload {
            entry_id: entry_id.to_string(),
            playlist_id: playlist_id.to_string(),
            cloud_track_id: cloud_track_id.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            duration_ms,
            content_hash: content_hash.map(|s| s.to_string()),
            position,
        };
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize track add payload: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO sync_outbox (operation_id, entity_type, entity_id, operation_type, payload_json, status)
            VALUES (?, 'playlist_track', ?, 'add', ?, 'pending')
            "#,
        )
        .bind(&op_id)
        .bind(entry_id)
        .bind(&payload_json)
        .execute(conn)
        .await
        .map_err(|e| format!("Failed to insert outbox track add: {e}"))?;

        Ok(op_id)
    }

    pub async fn record_track_remove(
        conn: &mut SqliteConnection,
        playlist_id: &str,
        entry_id: &str,
        position: i64,
    ) -> Result<String, String> {
        let op_id = format!("op-{}", Uuid::new_v4());
        let payload = PlaylistTrackRemovePayload {
            entry_id: entry_id.to_string(),
            playlist_id: playlist_id.to_string(),
            position,
        };
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize track remove payload: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO sync_outbox (operation_id, entity_type, entity_id, operation_type, payload_json, status)
            VALUES (?, 'playlist_track', ?, 'remove', ?, 'pending')
            "#,
        )
        .bind(&op_id)
        .bind(entry_id)
        .bind(&payload_json)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to insert outbox track remove: {e}"))?;

        // Record entry tombstone
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO sync_tombstones (entity_type, entity_id, deleted_at)
            VALUES ('playlist_track', ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(entry_id)
        .execute(conn)
        .await
        .map_err(|e| format!("Failed to insert playlist track tombstone: {e}"))?;

        Ok(op_id)
    }

    pub async fn record_tracks_reorder(
        conn: &mut SqliteConnection,
        playlist_id: &str,
        from_pos: i64,
        to_pos: i64,
    ) -> Result<String, String> {
        let op_id = format!("op-{}", Uuid::new_v4());
        let payload = PlaylistTracksReorderPayload {
            playlist_id: playlist_id.to_string(),
            from_position: from_pos,
            to_position: to_pos,
        };
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize tracks reorder payload: {e}"))?;

        sqlx::query(
            r#"
            INSERT INTO sync_outbox (operation_id, entity_type, entity_id, operation_type, payload_json, status)
            VALUES (?, 'playlist', ?, 'reorder', ?, 'pending')
            "#,
        )
        .bind(&op_id)
        .bind(playlist_id)
        .bind(&payload_json)
        .execute(conn)
        .await
        .map_err(|e| format!("Failed to insert outbox tracks reorder: {e}"))?;

        Ok(op_id)
    }

    pub async fn record_track_like_status(
        conn: &mut SqliteConnection,
        cloud_track_id: &str,
        content_hash: Option<&str>,
        title: &str,
        artist: &str,
        album: &str,
        duration_ms: i64,
        is_liked: bool,
    ) -> Result<String, String> {
        let op_id = format!("op-{}", Uuid::new_v4());
        let payload = LikedTrackPayload {
            cloud_track_id: cloud_track_id.to_string(),
            content_hash: content_hash.map(|s| s.to_string()),
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            duration_ms,
            is_liked,
        };
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize like status payload: {e}"))?;

        let op_type = if is_liked { "like" } else { "unlike" };

        sqlx::query(
            r#"
            INSERT INTO sync_outbox (operation_id, entity_type, entity_id, operation_type, payload_json, status)
            VALUES (?, 'liked_track', ?, ?, ?, 'pending')
            "#,
        )
        .bind(&op_id)
        .bind(cloud_track_id)
        .bind(op_type)
        .bind(&payload_json)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to insert outbox like status: {e}"))?;

        if !is_liked {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO sync_tombstones (entity_type, entity_id, deleted_at)
                VALUES ('liked_track', ?, CURRENT_TIMESTAMP)
                "#,
            )
            .bind(cloud_track_id)
            .execute(conn)
            .await
            .map_err(|e| format!("Failed to insert liked track tombstone: {e}"))?;
        }

        Ok(op_id)
    }

    pub async fn fetch_pending(
        pool: &SqlitePool,
        limit: i64,
    ) -> Result<Vec<SyncOutboxItem>, String> {
        let rows = sqlx::query(
            r#"
            SELECT
                operation_id,
                entity_type,
                entity_id,
                operation_type,
                payload_json,
                created_at,
                attempt_count,
                last_error,
                status
            FROM sync_outbox
            WHERE status IN ('pending', 'failed')
            ORDER BY created_at ASC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch pending outbox items: {e}"))?;

        let items = rows
            .into_iter()
            .map(|r| SyncOutboxItem {
                operation_id: r.get("operation_id"),
                entity_type: r.get("entity_type"),
                entity_id: r.get("entity_id"),
                operation_type: r.get("operation_type"),
                payload_json: r.get("payload_json"),
                created_at: r.get("created_at"),
                attempt_count: r.get("attempt_count"),
                last_error: r.get("last_error"),
                status: r.get("status"),
            })
            .collect();

        Ok(items)
    }

    pub async fn mark_completed(pool: &SqlitePool, op_ids: &[String]) -> Result<(), String> {
        if op_ids.is_empty() {
            return Ok(());
        }

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("Failed to start transaction: {e}"))?;

        for op_id in op_ids {
            sqlx::query("DELETE FROM sync_outbox WHERE operation_id = ?")
                .bind(op_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to remove completed outbox item: {e}"))?;
        }

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit outbox deletion: {e}"))?;

        Ok(())
    }

    pub async fn mark_failed(
        pool: &SqlitePool,
        op_id: &str,
        error_msg: &str,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            UPDATE sync_outbox
            SET status = 'failed',
                attempt_count = attempt_count + 1,
                last_error = ?
            WHERE operation_id = ?
            "#,
        )
        .bind(error_msg)
        .bind(op_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to mark outbox item failed: {e}"))?;

        Ok(())
    }

    pub async fn get_pending_count(pool: &SqlitePool) -> Result<usize, String> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sync_outbox WHERE status IN ('pending', 'failed')",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        Ok(count as usize)
    }

    pub async fn clear_all(pool: &SqlitePool) -> Result<(), String> {
        sqlx::query("DELETE FROM sync_outbox")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to clear outbox: {e}"))?;

        Ok(())
    }
}

fn chrono_now_iso() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", duration.as_secs(), duration.subsec_millis())
}
