use crate::services::library::get_all_tracks;
use crate::services::playback::PlaybackSnapshot;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_playback_state(state: State<'_, AppState>) -> Result<PlaybackSnapshot, String> {
    let svc = state.playback.lock().map_err(|e| e.to_string())?;
    Ok(svc.snapshot())
}

#[tauri::command]
pub async fn play_track(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.play_track(&track_id)
}

#[tauri::command]
pub async fn play(state: State<'_, AppState>) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.play()
}

#[tauri::command]
pub async fn pause(state: State<'_, AppState>) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.pause()
}

#[tauri::command]
pub async fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.play_pause()
}

#[tauri::command]
pub async fn stop(state: State<'_, AppState>) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.stop()
}

#[tauri::command]
pub async fn seek(state: State<'_, AppState>, position_ms: u64) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.seek(position_ms)
}

#[tauri::command]
pub async fn set_volume(state: State<'_, AppState>, volume: u32) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.set_volume(volume)
}

#[tauri::command]
pub async fn toggle_mute(state: State<'_, AppState>) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.toggle_mute()
}

#[tauri::command]
pub async fn set_playback_speed(state: State<'_, AppState>, speed: f32) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.set_speed(speed)
}

#[tauri::command]
pub async fn set_repeat_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.set_repeat_mode(&mode)
}

#[tauri::command]
pub async fn set_shuffle(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.set_shuffle(enabled)
}

#[tauri::command]
pub async fn next_track(state: State<'_, AppState>) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.next()
}

#[tauri::command]
pub async fn previous_track(state: State<'_, AppState>) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.previous()
}

#[tauri::command]
pub async fn set_queue(
    state: State<'_, AppState>,
    track_ids: Vec<String>,
    start_track_id: Option<String>,
) -> Result<(), String> {
    if track_ids.is_empty() {
        let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
        return svc.clear_queue();
    }

    // Load full Track records from database preserving requested order
    let all_tracks = get_all_tracks(&state.database).await?;
    let mut ordered_tracks = Vec::with_capacity(track_ids.len());

    for tid in &track_ids {
        if let Some(track) = all_tracks.iter().find(|t| &t.id == tid) {
            ordered_tracks.push(track.clone());
        }
    }

    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.set_queue(ordered_tracks, start_track_id.as_deref())
}

#[tauri::command]
pub async fn add_to_queue(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    let all_tracks = get_all_tracks(&state.database).await?;
    let track = all_tracks
        .into_iter()
        .find(|t| t.id == track_id)
        .ok_or_else(|| format!("Track not found in database: {track_id}"))?;

    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.add_to_queue(track)
}

#[tauri::command]
pub async fn remove_from_queue(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.remove_from_queue(&track_id)
}

#[tauri::command]
pub async fn remove_from_queue_by_index(
    state: State<'_, AppState>,
    index: usize,
) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.remove_from_queue_by_index(index)
}

#[tauri::command]
pub async fn reorder_queue(
    state: State<'_, AppState>,
    from_index: usize,
    to_index: usize,
) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.reorder_queue(from_index, to_index)
}

#[tauri::command]
pub async fn clear_queue(state: State<'_, AppState>) -> Result<(), String> {
    let mut svc = state.playback.lock().map_err(|e| e.to_string())?;
    svc.clear_queue()
}
