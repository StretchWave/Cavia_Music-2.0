use tauri::State;

use crate::domain::Track;
use crate::services::library::{get_track_by_id, get_tracks_by_ids};
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
pub async fn play_queue_index(state: State<'_, AppState>, index: usize) -> Result<(), String> {
    state.playback.play_queue_index(index).await
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
    if !speed.is_finite() || !(0.25..=4.0).contains(&speed) {
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

    let ordered_tracks = get_tracks_by_ids(&state.database, &track_ids).await?;

    state
        .playback
        .set_queue(ordered_tracks, start_track_id.as_deref())
        .await
}

#[tauri::command]
pub async fn set_playback_queue(
    state: State<'_, AppState>,
    tracks: Vec<Track>,
    start_index: Option<usize>,
    enable_radio: Option<bool>,
) -> Result<(), String> {
    if tracks.is_empty() {
        return state.playback.clear_queue().await;
    }

    // Hydrate local tracks missing file_uri from SQLite without loading the full library.
    let local_ids: Vec<String> = tracks
        .iter()
        .filter(|t| t.source_kind == "local" && t.file_uri.is_none())
        .map(|t| t.id.clone())
        .collect();

    let hydrated_tracks = if local_ids.is_empty() {
        tracks
    } else {
        let fetched = get_tracks_by_ids(&state.database, &local_ids).await?;
        let by_id: std::collections::HashMap<String, Track> =
            fetched.into_iter().map(|t| (t.id.clone(), t)).collect();
        tracks
            .into_iter()
            .filter_map(|t| {
                if t.source_kind == "local" && t.file_uri.is_none() {
                    by_id.get(&t.id).cloned()
                } else {
                    Some(t)
                }
            })
            .collect()
    };

    state
        .playback
        .set_mixed_queue(hydrated_tracks, start_index, enable_radio.unwrap_or(false))
        .await
}

#[tauri::command]
pub async fn add_to_queue(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    let track = get_track_by_id(&state.database, &track_id)
        .await?
        .ok_or_else(|| format!("Track not found in database: {track_id}"))?;

    state.playback.add_to_queue(track).await
}

#[tauri::command]
pub async fn play_next(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    let track = get_track_by_id(&state.database, &track_id)
        .await?
        .ok_or_else(|| format!("Track not found in database: {track_id}"))?;

    state.playback.play_next(track).await
}

#[tauri::command]
pub async fn enqueue_track(
    state: State<'_, AppState>,
    track: Track,
    play_next: Option<bool>,
) -> Result<(), String> {
    let resolved = if track.source_kind == "local" && track.file_uri.is_none() {
        get_track_by_id(&state.database, &track.id)
            .await?
            .ok_or_else(|| format!("Track not found in database: {}", track.id))?
    } else {
        track
    };

    if play_next.unwrap_or(false) {
        state.playback.play_next(resolved).await
    } else {
        state.playback.add_to_queue(resolved).await
    }
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

#[tauri::command]
pub async fn retry_shutdown(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.playback.shutdown().await?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn force_exit(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let _ = state.playback.shutdown().await;
    app.exit(0);
    Ok(())
}
