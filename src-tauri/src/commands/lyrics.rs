use crate::services::lyrics::LyricsResponseDTO;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_track_lyrics(
    state: State<'_, AppState>,
    track_id: String,
) -> Result<LyricsResponseDTO, String> {
    state.lyrics.get_lyrics(&track_id).await
}

#[tauri::command]
pub async fn refresh_track_lyrics(
    state: State<'_, AppState>,
    track_id: String,
) -> Result<LyricsResponseDTO, String> {
    state.lyrics.refresh_lyrics(&track_id).await
}
