use tauri::{AppHandle, State};

use crate::services::spotify::{ImportJob, ImportStatus};
use crate::state::AppState;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyImportResult {
    pub playlist_id: String,
    pub playlist_name: String,
    pub total: usize,
    pub matched_local: usize,
    pub matched_online: usize,
    pub skipped: Vec<String>,
}

/// Starts an asynchronous Spotify playlist import. Playlist metadata is read
/// first, then all track shells are inserted in one SQLite transaction; matching
/// runs in the background with live progress events.
#[tauri::command]
pub async fn spotify_start_import(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<ImportJob, String> {
    state.import_manager.start_import(app, &url).await
}

/// Starts an asynchronous YouTube Music playlist import.
#[tauri::command]
pub async fn ytmusic_start_import(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<ImportJob, String> {
    state.import_manager.start_ytmusic_import(app, &url).await
}

/// Starts an asynchronous playlist import from either Spotify or YouTube Music.
#[tauri::command]
pub async fn playlist_start_import(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<ImportJob, String> {
    state.import_manager.start_playlist_import(app, &url).await
}

#[tauri::command]
pub async fn import_pause(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    state.import_manager.pause_job(Some(&app), &job_id).await
}

#[tauri::command]
pub async fn import_resume(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    state.import_manager.resume_job(Some(&app), &job_id).await
}

#[tauri::command]
pub async fn import_cancel(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    state.import_manager.cancel_job(Some(&app), &job_id).await
}

#[tauri::command]
pub async fn import_get_jobs(state: State<'_, AppState>) -> Result<Vec<ImportJob>, String> {
    Ok(state.import_manager.get_jobs().await)
}

#[tauri::command]
pub async fn import_clear_completed(state: State<'_, AppState>) -> Result<(), String> {
    state.import_manager.clear_completed().await;
    Ok(())
}

#[tauri::command]
pub async fn import_remove_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    state.import_manager.remove_job(&job_id).await
}

#[tauri::command]
pub async fn import_cancel_and_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<String, String> {
    state
        .import_manager
        .cancel_and_delete(Some(&app), &job_id)
        .await
}

/// Backwards-compatible synchronous bridge command: starts import and waits
/// until the background matching job completes, returning the legacy result format.
#[tauri::command]
pub async fn spotify_import_playlist(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<SpotifyImportResult, String> {
    let job = state.import_manager.start_import(app, &url).await?;

    // Poll until completed, cancelled or failed
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let jobs = state.import_manager.get_jobs().await;
        if let Some(current) = jobs.iter().find(|j| j.id == job.id) {
            match current.status {
                ImportStatus::Completed | ImportStatus::Cancelled | ImportStatus::Failed => {
                    return Ok(SpotifyImportResult {
                        playlist_id: current.playlist_id.clone(),
                        playlist_name: current.playlist_name.clone(),
                        total: current.total,
                        matched_local: current.matched_local,
                        matched_online: current.matched_online,
                        skipped: Vec::new(),
                    });
                }
                _ => {}
            }
        } else {
            break;
        }
    }

    Ok(SpotifyImportResult {
        playlist_id: job.playlist_id,
        playlist_name: job.playlist_name,
        total: job.total,
        matched_local: 0,
        matched_online: 0,
        skipped: Vec::new(),
    })
}
