use crate::services::sync::{SyncAccountStatus, SyncConfigDTO, SyncResult};
use crate::state::AppState;
use tauri::State;

/// Performs an ephemeral credential-store round trip without exposing its value.
/// Android uses this command for device/emulator verification of Keystore access.
#[tauri::command]
pub fn credential_storage_self_test(state: State<'_, AppState>) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let account = format!("self-test-{}", uuid::Uuid::new_v4());
        let secret = format!("cavia-credential-round-trip-{}", uuid::Uuid::new_v4());
        if let Err(error) = state
            .credentials
            .set("com.cavia.music.self-test", &account, &secret)
        {
            let _ = state
                .credentials
                .delete("com.cavia.music.self-test", &account);
            return Err(error);
        }
        let read_result = state.credentials.get("com.cavia.music.self-test", &account);
        let delete_result = state
            .credentials
            .delete("com.cavia.music.self-test", &account);
        delete_result?;
        return Ok(read_result? == Some(secret));
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = state;
        Err("Credential self-test is only available on Android".into())
    }
}

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
    let (url, anon_key) = state.sync.get_config().await?;
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
