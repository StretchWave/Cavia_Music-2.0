use crate::services::sync::{SyncAccountStatus, SyncConfigDTO, SyncResult};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn sync_get_status(state: State<'_, AppState>) -> Result<SyncAccountStatus, String> {
    state.sync.get_status().await
}

#[tauri::command]
pub async fn sync_signup(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<SyncAccountStatus, String> {
    state.sync.signup(&email, &password).await
}

#[tauri::command]
pub async fn sync_login(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<SyncAccountStatus, String> {
    state.sync.login(&email, &password).await
}

#[tauri::command]
pub async fn sync_logout(state: State<'_, AppState>) -> Result<(), String> {
    state.sync.logout().await
}

#[tauri::command]
pub async fn sync_toggle(state: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    state.sync.toggle_sync(enabled).await
}

#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>) -> Result<SyncResult, String> {
    state.sync.sync().await
}

#[tauri::command]
pub async fn sync_delete_cloud_data(state: State<'_, AppState>) -> Result<(), String> {
    state.sync.delete_cloud_data().await
}

#[tauri::command]
pub async fn sync_get_config(state: State<'_, AppState>) -> Result<SyncConfigDTO, String> {
    let (url, anon_key) = state.sync.get_config().await;
    let is_configured = !url.is_empty() && !anon_key.is_empty();
    Ok(SyncConfigDTO {
        supabase_url: url,
        supabase_anon_key: anon_key,
        is_configured,
    })
}

#[tauri::command]
pub async fn sync_set_config(
    state: State<'_, AppState>,
    url: String,
    anon_key: String,
) -> Result<(), String> {
    state.sync.set_config(&url, &anon_key).await
}
