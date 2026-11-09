use std::collections::HashMap;
use tauri::State;

use crate::domain::Track;
use crate::services::library::get_all_tracks;
use crate::services::playback::PlaybackSnapshot;
use crate::state::AppState;

#[tauri::command]
pub async fn get_playback_state(state: State<'_, AppState>) -> Result<PlaybackSnapshot, String> {
    Ok(state.playback.snapshot())
}

#[tauri::command]
pub async fn play_track(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    state.playback.play_track(&track_id).await
}

#[tauri::command]
pub async fn play(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.play().await
}

#[tauri::command]
pub async fn pause(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.pause().await
}

#[tauri::command]
pub async fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.play_pause().await
}

#[tauri::command]
pub async fn stop(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.stop().await
}

#[tauri::command]
pub async fn seek(
    state: State<'_, AppState>,
    position_ms: u64,
    generation: Option<u64>,
) -> Result<(), String> {
    state.playback.seek(position_ms, generation).await
}

#[tauri::command]
pub async fn set_volume(state: State<'_, AppState>, volume: u32) -> Result<(), String> {
    if volume > 100 {
        return Err("Volume must be between 0 and 100".into());
    }
    state.playback.set_volume(volume).await
}

#[tauri::command]
pub async fn toggle_mute(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.toggle_mute().await
}

#[tauri::command]
pub async fn set_playback_speed(state: State<'_, AppState>, speed: f32) -> Result<(), String> {
    if !speed.is_finite() || speed < 0.25 || speed > 4.0 {
        return Err("Playback speed must be a finite number between 0.25 and 4.0".into());
    }
    state.playback.set_speed(speed).await
}

#[tauri::command]
pub async fn set_repeat_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    state.playback.set_repeat_mode(&mode).await
}

#[tauri::command]
pub async fn set_shuffle(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state.playback.set_shuffle(enabled).await
}

#[tauri::command]
pub async fn next_track(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.next().await
}

#[tauri::command]
pub async fn previous_track(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.previous().await
}

#[tauri::command]
pub async fn set_queue(
    state: State<'_, AppState>,
    track_ids: Vec<String>,
    start_track_id: Option<String>,
) -> Result<(), String> {
    if track_ids.is_empty() {
        return state.playback.clear_queue().await;
    }

    // Load full Track records from database preserving requested order
    // Optimized to O(N + M) using HashMap lookup
    let all_tracks = get_all_tracks(&state.database).await?;
    let mut track_map: HashMap<String, Track> = HashMap::with_capacity(all_tracks.len());
    for t in all_tracks {
        track_map.insert(t.id.clone(), t);
    }

    let mut ordered_tracks = Vec::with_capacity(track_ids.len());
    for tid in &track_ids {
        if let Some(track) = track_map.get(tid) {
            ordered_tracks.push(track.clone());
        }
    }

    state
        .playback
        .set_queue(ordered_tracks, start_track_id.as_deref())
        .await
}

#[tauri::command]
pub async fn add_to_queue(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    let all_tracks = get_all_tracks(&state.database).await?;
    let track = all_tracks
        .into_iter()
        .find(|t| t.id == track_id)
        .ok_or_else(|| format!("Track not found in database: {track_id}"))?;

    state.playback.add_to_queue(track).await
}

#[tauri::command]
pub async fn remove_from_queue(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    state.playback.remove_from_queue(&track_id).await
}

#[tauri::command]
pub async fn remove_from_queue_by_index(
    state: State<'_, AppState>,
    index: usize,
) -> Result<(), String> {
    state.playback.remove_from_queue_by_index(index).await
}

#[tauri::command]
pub async fn reorder_queue(
    state: State<'_, AppState>,
    from_index: usize,
    to_index: usize,
) -> Result<(), String> {
    state.playback.reorder_queue(from_index, to_index).await
}

#[tauri::command]
pub async fn clear_queue(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.clear_queue().await
}

#[tauri::command]
pub async fn retry_audio_device(state: State<'_, AppState>) -> Result<(), String> {
    state.playback.retry_device().await
}
