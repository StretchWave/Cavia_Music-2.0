use tauri::State;

use crate::domain::Track;
use crate::services::ytmusic::YtMusicTrack;
use crate::state::AppState;

#[tauri::command]
pub async fn ytmusic_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<YtMusicTrack>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.chars().count() > 200 {
        return Err("Search query is too long".into());
    }
    state.ytmusic.search(trimmed).await
}

/// Tracks YouTube Music would play after `video_id`. The UI uses this as a
/// radio when the user played a single catalogue result and never built a
/// queue of their own.
#[tauri::command]
pub async fn ytmusic_related(
    state: State<'_, AppState>,
    video_id: String,
) -> Result<Vec<YtMusicTrack>, String> {
    state.ytmusic.related(&video_id).await
}

/// Resolves a YouTube Music result and plays it through the native engine.
///
/// The audio stream is fetched into the app stream cache first so playback,
/// seeking and duration use the same decoder path as local files. Nothing is
/// written to the library database: online results stay ephemeral.
#[tauri::command]
pub async fn ytmusic_play(
    state: State<'_, AppState>,
    video_id: String,
    title: String,
    artist: String,
    album: String,
    duration_ms: i64,
    artwork_url: Option<String>,
) -> Result<(), String> {
    let valid_id = video_id.len() == 11
        && video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid_id {
        return Err("Invalid YouTube Music video id".into());
    }

    if title.trim().is_empty() {
        return Err("YouTube Music returned a result without a title".into());
    }

    let resolved = state.ytmusic.resolve_audio(&video_id).await?;
    let path = state
        .ytmusic
        .ensure_cached(&video_id, &resolved.url)
        .await?;

    let track_id = format!("ytmusic:{video_id}");
    let artist_name = if artist.trim().is_empty() {
        "Unknown artist".to_string()
    } else {
        artist
    };

    let track = Track {
        id: track_id.clone(),
        title,
        artist: artist_name.clone(),
        album,
        album_artist: Some(artist_name),
        duration_ms: duration_ms.max(0),
        artwork_uri: artwork_url,
        source_kind: "provider".to_string(),
        provider_id: Some("youtube_music".to_string()),
        file_uri: Some(path.to_string_lossy().to_string()),
        release_year: None,
        track_number: None,
        disc_number: None,
        genre: None,
        is_liked: false,
        play_count: 0,
        date_added: String::new(),
    };

    let playback_state = state.playback.snapshot();
    let in_queue = playback_state.queue.iter().any(|t| t.id == track_id);
    if !in_queue {
        state.playback.add_to_queue(track).await?;
    }
    state.playback.play_track(&track_id).await?;
    state.playback.play().await?;
    Ok(())
}
