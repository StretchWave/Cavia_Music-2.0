use crate::services::artwork::{
    ArtworkCacheSummary, ArtworkCleanupResult, ArtworkRecoveryResult, TrackRecoveryOutcome,
};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_artwork_cache_summary(
    state: State<'_, AppState>,
) -> Result<ArtworkCacheSummary, String> {
    state.artwork.get_summary().await
}

#[tauri::command]
pub async fn clean_orphaned_artwork(
    state: State<'_, AppState>,
) -> Result<ArtworkCleanupResult, String> {
    state.artwork.clean_orphans().await
}

#[tauri::command]
pub async fn recover_missing_artwork(
    state: State<'_, AppState>,
) -> Result<ArtworkRecoveryResult, String> {
    state.artwork.recover_missing().await
}

#[tauri::command]
pub async fn recover_track_artwork(
    track_id: String,
    state: State<'_, AppState>,
) -> Result<TrackRecoveryOutcome, String> {
    state.artwork.recover_track(&track_id).await
}
