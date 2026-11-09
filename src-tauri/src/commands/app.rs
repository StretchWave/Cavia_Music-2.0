use serde::Serialize;
use sqlx::Row;
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AppHealth {
    pub app_version: &'static str,
    pub database_connected: bool,
    pub audio_backend: &'static str,
    pub platform: &'static str,
}

#[tauri::command]
pub async fn app_health(state: State<'_, AppState>) -> Result<AppHealth, String> {
    let row = sqlx::query("SELECT 1 AS ok")
        .fetch_one(&state.database)
        .await
        .map_err(|error| format!("Local database health check failed: {error}"))?;

    let database_connected = row.try_get::<i64, _>("ok").unwrap_or_default() == 1;

    let audio_backend = {
        if let Ok(svc) = state.playback.lock() {
            svc.backend_health_status()
        } else {
            "error"
        }
    };

    Ok(AppHealth {
        app_version: env!("CARGO_PKG_VERSION"),
        database_connected,
        audio_backend,
        platform: std::env::consts::OS,
    })
}
