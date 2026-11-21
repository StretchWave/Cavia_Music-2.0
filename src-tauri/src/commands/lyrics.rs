use crate::services::lyrics::{LyricsResponseDTO, LyricsTrackMeta};
use crate::state::AppState;
use tauri::State;

/// Only build a metadata fallback when the renderer actually knows the title.
/// Otherwise the resolver falls back to the library lookup and reports an
/// honest "not found" instead of querying with empty fields.
fn meta_from_request(
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
) -> Option<LyricsTrackMeta> {
    let title = title?;
    if title.trim().is_empty() {
        return None;
    }
    Some(LyricsTrackMeta {
        title,
        artist: artist.unwrap_or_default(),
        album: album.unwrap_or_default(),
        duration_ms: duration_ms.unwrap_or(0).max(0),
    })
}

#[tauri::command]
pub async fn get_track_lyrics(
    state: State<'_, AppState>,
    track_id: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
) -> Result<LyricsResponseDTO, String> {
    match meta_from_request(title, artist, album, duration_ms) {
        Some(meta) => {
            state
                .lyrics
                .get_lyrics_with_meta(&track_id, &meta, false)
                .await
        }
        None => state.lyrics.get_lyrics(&track_id).await,
    }
}

#[tauri::command]
pub async fn refresh_track_lyrics(
    state: State<'_, AppState>,
    track_id: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
) -> Result<LyricsResponseDTO, String> {
    match meta_from_request(title, artist, album, duration_ms) {
        Some(meta) => {
            state
                .lyrics
                .get_lyrics_with_meta(&track_id, &meta, true)
                .await
        }
        None => state.lyrics.refresh_lyrics(&track_id).await,
    }
}
