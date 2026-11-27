use tauri::State;

use crate::domain::Track;
use crate::services::ytmusic::YtMusicTrack;
use crate::state::AppState;

#[tauri::command]
pub async fn ytmusic_search(
    state: State<'_, AppState>,
    query: String,
    region: Option<String>,
    language: Option<String>,
    request_id: String,
) -> Result<Vec<YtMusicTrack>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.chars().count() > 200 {
        return Err("Search query is too long".into());
    }
    let region = normalize_region(region.as_deref());
    let language = normalize_locale_part(language.as_deref(), 3, "en");
    let cancellation = state.ytmusic.begin_search(&request_id)?;
    let result = tokio::select! {
        result = state.ytmusic.search_with_locale(trimmed, &region, &language) => result,
        _ = cancellation => Err("YouTube Music search cancelled".into()),
    };
    state.ytmusic.finish_search(&request_id);
    result
}

#[tauri::command]
pub fn ytmusic_cancel_search(state: State<'_, AppState>, request_id: String) -> Result<(), String> {
    state.ytmusic.cancel_search(&request_id)
}

fn normalize_locale_part(value: Option<&str>, max_len: usize, fallback: &str) -> String {
    value
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= max_len
                && value
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())
        })
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| fallback.to_string())
}

fn normalize_region(value: Option<&str>) -> String {
    normalize_locale_part(value, 2, "US").to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{normalize_locale_part, normalize_region};

    #[test]
    fn locale_parts_are_normalized_and_bounded() {
        assert_eq!(normalize_locale_part(Some("in"), 2, "US"), "in");
        assert_eq!(normalize_locale_part(Some("EN"), 3, "en"), "en");
        assert_eq!(normalize_locale_part(Some("../../"), 2, "US"), "US");
        assert_eq!(normalize_locale_part(Some("IND"), 2, "US"), "US");
        assert_eq!(normalize_locale_part(None, 3, "en"), "en");
    }

    #[test]
    fn youtube_music_country_code_is_uppercase() {
        assert_eq!(normalize_region(Some("in")), "IN");
        assert_eq!(normalize_region(Some("us")), "US");
        assert_eq!(normalize_region(None), "US");
    }
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
        .ensure_cached_with_mime(&video_id, &resolved.url, &resolved.mime_type)
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
        queue_entry_id: None,
        contributor_id: None,
        contributor_name: None,
        priority_override: None,
    };

    state
        .playback
        .set_mixed_queue(vec![track], Some(0), true)
        .await?;
    Ok(())
}
