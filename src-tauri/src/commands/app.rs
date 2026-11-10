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
    })
}
