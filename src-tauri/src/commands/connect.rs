use tauri::State;

use crate::domain::Track;
use crate::services::connect::{
    ConnectContributionResult, ConnectPlaylistContributionResult, ConnectSessionSnapshot,
    ConnectSyncResult,
};
use crate::services::fair_queue::ParticipantRole;
use crate::services::offline::{OfflineFallbackStatus, probe_internet_connectivity};
use crate::services::playback::{handle_connectivity_restored, restore_original_queue_now};
use crate::services::preload::PreloadStatusSnapshot;
use crate::state::AppState;

#[tauri::command]
pub async fn preload_get_status(
    state: State<'_, AppState>,
) -> Result<PreloadStatusSnapshot, String> {
    Ok(state.preload.snapshot())
}

#[tauri::command]
pub async fn preload_set_target(
    target: usize,
    state: State<'_, AppState>,
) -> Result<PreloadStatusSnapshot, String> {
    state.preload.set_target(target).await
}

#[tauri::command]
pub async fn preload_retry(state: State<'_, AppState>) -> Result<PreloadStatusSnapshot, String> {
    state.preload.retry_failed();
    Ok(state.preload.snapshot())
}

#[tauri::command]
pub async fn offline_get_status(
    state: State<'_, AppState>,
) -> Result<OfflineFallbackStatus, String> {
    Ok(state.offline.snapshot())
}

#[tauri::command]
pub async fn offline_choose_shuffle_cached(
    state: State<'_, AppState>,
) -> Result<OfflineFallbackStatus, String> {
    let saved = state.playback.queue_snapshot().await?;
    let current_id = state.playback.snapshot().current_track.map(|t| t.id);
    let pool = state
        .offline
        .activate_fallback(saved, current_id.as_deref())
        .await?;
    state.playback.enter_offline_fallback(pool).await?;
    Ok(state.offline.snapshot())
}

#[tauri::command]
pub async fn offline_decline_fallback(
    state: State<'_, AppState>,
) -> Result<OfflineFallbackStatus, String> {
    Ok(state.offline.decline_fallback())
}

#[tauri::command]
pub async fn offline_exit_fallback(
    state: State<'_, AppState>,
) -> Result<OfflineFallbackStatus, String> {
    restore_original_queue_now(&state.playback, &state.connect, &state.offline, false).await;
    Ok(state.offline.snapshot())
}

#[tauri::command]
pub async fn offline_probe_reconnect(
    state: State<'_, AppState>,
) -> Result<OfflineFallbackStatus, String> {
    if probe_internet_connectivity().await {
        handle_connectivity_restored(
            &state.playback,
            &state.preload,
            &state.connect,
            &state.offline,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .await;
    }
    Ok(state.offline.snapshot())
}

#[tauri::command]
pub async fn connect_get_session(
    state: State<'_, AppState>,
) -> Result<Option<ConnectSessionSnapshot>, String> {
    state.connect.get_snapshot().await
}

#[tauri::command]
pub async fn connect_create_session(
    name: String,
    host_name: String,
    alpha: Option<f64>,
    state: State<'_, AppState>,
) -> Result<ConnectSessionSnapshot, String> {
    let snap = state
        .connect
        .create_session(&name, &host_name, alpha)
        .await?;
    sync_connect_into_playback(&state).await;
    Ok(snap)
}

#[tauri::command]
pub async fn connect_join_session(
    join_code: String,
    display_name: String,
    participant_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ConnectSessionSnapshot, String> {
    let snap = state
        .connect
        .join_session(&join_code, participant_id.as_deref(), &display_name)
        .await?;
    sync_connect_into_playback(&state).await;
    Ok(snap)
}

#[tauri::command]
pub async fn connect_set_participant_connected(
    participant_id: String,
    connected: bool,
    state: State<'_, AppState>,
) -> Result<ConnectSessionSnapshot, String> {
    state
        .connect
        .set_participant_connected(&participant_id, connected)
        .await
}

#[tauri::command]
pub async fn connect_contribute_track(
    track: Track,
    contributor_id: Option<String>,
    contributor_name: Option<String>,
    role: Option<ParticipantRole>,
    priority_override: Option<bool>,
    state: State<'_, AppState>,
) -> Result<ConnectContributionResult, String> {
    let res = state
        .connect
        .contribute_track(
            contributor_id.as_deref(),
            contributor_name.as_deref(),
            role,
            track,
            priority_override.unwrap_or(false),
        )
        .await?;
    sync_connect_into_playback(&state).await;
    Ok(res)
}

#[tauri::command]
pub async fn connect_contribute_ytmusic_playlist(
    playlist_url_or_id: String,
    contributor_id: Option<String>,
    contributor_name: Option<String>,
    role: Option<ParticipantRole>,
    state: State<'_, AppState>,
) -> Result<ConnectPlaylistContributionResult, String> {
    let playlist = state.ytmusic.fetch_playlist(&playlist_url_or_id).await?;
    let tracks: Vec<Track> = playlist
        .tracks
        .into_iter()
        .map(|item| Track {
            id: format!("ytmusic:{}", item.video_id),
            title: item.title,
            artist: if item.artist.trim().is_empty() {
                "Unknown artist".to_string()
            } else {
                item.artist
            },
            album: item.album,
            album_artist: None,
            duration_ms: item.duration_ms.max(0),
            artwork_uri: item.artwork_url,
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
        })
        .collect();

    let res = state
        .connect
        .contribute_playlist_tracks(
            contributor_id.as_deref(),
            contributor_name.as_deref(),
            role,
            tracks,
        )
        .await?;
    sync_connect_into_playback(&state).await;
    Ok(res)
}

#[tauri::command]
pub async fn connect_remove_entry(
    queue_entry_id: String,
    apply_penalty: Option<bool>,
    state: State<'_, AppState>,
) -> Result<ConnectSessionSnapshot, String> {
    let snap = state
        .connect
        .remove_entry(&queue_entry_id, apply_penalty.unwrap_or(true))
        .await?;
    sync_connect_into_playback(&state).await;
    Ok(snap)
}

#[tauri::command]
pub async fn connect_host_reorder(
    from_index: usize,
    to_index: usize,
    state: State<'_, AppState>,
) -> Result<ConnectSessionSnapshot, String> {
    let snap = state
        .connect
        .host_reorder_upcoming(from_index, to_index)
        .await?;
    sync_connect_into_playback(&state).await;
    Ok(snap)
}

#[tauri::command]
pub async fn connect_set_alpha(
    alpha: f64,
    state: State<'_, AppState>,
) -> Result<ConnectSessionSnapshot, String> {
    let snap = state.connect.set_fairness_alpha(alpha).await?;
    sync_connect_into_playback(&state).await;
    Ok(snap)
}

#[tauri::command]
pub async fn connect_sync_now(state: State<'_, AppState>) -> Result<ConnectSyncResult, String> {
    let res = state.connect.sync_pending_events().await?;
    sync_connect_into_playback(&state).await;
    Ok(res)
}

#[tauri::command]
pub async fn connect_end_session(state: State<'_, AppState>) -> Result<(), String> {
    state.connect.end_session().await
}

async fn sync_connect_into_playback(state: &AppState) {
    if state.offline.is_fallback_active() {
        return;
    }
    if let Some((current, upcoming)) = state.connect.upcoming_playback_tracks() {
        let _ = state
            .playback
            .apply_collaborative_sequence(current, upcoming, false)
            .await;
    }
}
