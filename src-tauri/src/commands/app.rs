use serde::Serialize;
use sqlx::Row;
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AppHealth {
    pub app_version: &'static str,
    pub database_connected: bool,
    pub audio_backend: String,
    pub backend_name: String,
    pub active_migration: String,
    pub platform: &'static str,
    pub system_media_controls: bool,
    pub smtc_status: String,
    pub smtc_error: Option<String>,
    pub persistence_status: String,
    pub persistence_error: Option<String>,
}

#[tauri::command]
pub async fn app_health(state: State<'_, AppState>) -> Result<AppHealth, String> {
    let row = sqlx::query("SELECT 1 AS ok")
        .fetch_one(&state.database)
        .await
        .map_err(|error| format!("Local database health check failed: {error}"))?;

    let database_connected = row.try_get::<i64, _>("ok").unwrap_or_default() == 1;
    let audio_backend = state.playback.backend_health_status();
    let backend_name = state.playback.backend_name();
    let snapshot = state.playback.snapshot();
    let system_media_controls = snapshot.system_media_controls;
    let smtc_status = snapshot.smtc_status;
    let smtc_error = snapshot.smtc_error;

    let persist_info = state.playback.persistence_health();
    let persistence_status = persist_info.status.as_str().to_string();
    let persistence_error = persist_info.last_error;

    let migration_row = sqlx::query(
        "SELECT version, description FROM _sqlx_migrations ORDER BY version DESC LIMIT 1",
    )
    .fetch_optional(&state.database)
    .await
    .ok()
    .flatten();

    let active_migration = match migration_row {
        Some(r) => {
            let ver: i64 = r.try_get("version").unwrap_or(2);
            let desc: String = r.try_get("description").unwrap_or_default();
            format!("{ver:04}_{desc}.sql (Active)")
        }
        None => "0002_playback_state_volume.sql (Active)".to_string(),
    };

    Ok(AppHealth {
        app_version: env!("CARGO_PKG_VERSION"),
        database_connected,
        audio_backend,
        backend_name,
        active_migration,
        platform: std::env::consts::OS,
        system_media_controls,
        smtc_status,
        smtc_error,
        persistence_status,
        persistence_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::playback::{
        PersistenceHealthInfo, PersistenceHealthStatus, PlaybackService,
    };
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

    async fn temp_pool(dir: &std::path::Path) -> sqlx::SqlitePool {
        let db_path = dir.join("cavia_app_health_test.sqlite3");
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[test]
    fn test_app_health_serialization_includes_persistence() {
        let health = AppHealth {
            app_version: "2.0.0",
            database_connected: true,
            audio_backend: "ready".into(),
            backend_name: "rodio_native".into(),
            active_migration: "0002_playback_state_volume.sql (Active)".into(),
            platform: "windows",
            system_media_controls: false,
            smtc_status: "unavailable".into(),
            smtc_error: None,
            persistence_status: "healthy".into(),
            persistence_error: None,
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(
            json.contains(r#""persistence_status":"healthy""#),
            "AppHealth missing persistence_status field: {json}"
        );
        assert!(
            json.contains(r#""persistence_error":null"#),
            "AppHealth missing persistence_error field: {json}"
        );

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["persistence_status"], "healthy");
        assert_eq!(parsed["persistence_error"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_app_health_initial_healthy_status() {
        let dir = std::env::temp_dir().join(format!("cavia_health_b_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool, None).unwrap();

        let health_info = service.persistence_health();
        assert_eq!(health_info.status, PersistenceHealthStatus::Healthy);
        assert_eq!(health_info.last_error, None);
        assert_eq!(health_info.status.as_str(), "healthy");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_app_health_write_failed_status() {
        let dir = std::env::temp_dir().join(format!("cavia_health_c_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = temp_pool(&dir).await;
        let service = PlaybackService::new(pool.clone(), None).unwrap();

        // Inject SQL trigger failure
        sqlx::query(
            "CREATE TRIGGER force_playback_state_failure_health \
             BEFORE INSERT ON playback_state \
             BEGIN \
                 SELECT RAISE(FAIL, 'Injected persistence health failure'); \
             END;",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Queue a state change that writes to DB
        let _ = service.set_volume(48).await;

        // Shutdown will flush and trigger the failure
        let _ = service.shutdown().await;

        let health_info = service.persistence_health();
        assert_eq!(health_info.status, PersistenceHealthStatus::WriteFailed);
        assert!(health_info.last_error.is_some());
        assert!(
            health_info
                .last_error
                .unwrap()
                .contains("Injected persistence health failure")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_app_health_write_pending_status() {
        let mut info = PersistenceHealthInfo::default();
        info.status = PersistenceHealthStatus::WritePending;
        assert_eq!(info.status.as_str(), "write_pending");

        info.status = PersistenceHealthStatus::ShuttingDown;
        assert_eq!(info.status.as_str(), "shutting_down");
    }

    #[tokio::test]
    async fn test_app_health_worker_unavailable_status() {
        let mut info = PersistenceHealthInfo::default();
        info.status = PersistenceHealthStatus::WorkerUnavailable;
        info.last_error = Some("Persistence worker channel disconnected".into());

        assert_eq!(info.status.as_str(), "worker_unavailable");
        assert_eq!(
            info.last_error.as_deref(),
            Some("Persistence worker channel disconnected")
        );
    }
}
