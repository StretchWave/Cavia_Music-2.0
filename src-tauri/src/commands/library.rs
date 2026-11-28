use serde::Serialize;
use sqlx::Row;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

use crate::domain::{
    Album, Artist, Playlist, QuickAddPlaylistResult, RecentPlayEvent, ScanResult,
    SearchResultsDTO, Track,
};
use crate::services::library as lib_service;
use crate::services::scanner;
use crate::services::search as search_service;
use crate::state::AppState;

#[derive(Debug, Default, Serialize)]
pub struct LibrarySummary {
    pub tracks: i64,
    pub playlists: i64,
    pub liked_tracks: i64,
}

#[tauri::command]
pub async fn pick_android_music_folder(app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let handle = app.state::<crate::audio::android_media3::Media3PluginHandle>();
        let result: serde_json::Value = handle
            .0
            .run_mobile_plugin_async("pickMusicFolder", ())
            .await
            .map_err(|error| format!("Android folder picker failed: {error}"))?;
        return Ok(result
            .get("path")
            .and_then(serde_json::Value::as_str)
            .filter(|path| !path.is_empty())
            .map(str::to_owned));
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("The Android folder picker is available only on Android.".into())
    }
}

#[tauri::command]
pub async fn library_summary(state: State<'_, AppState>) -> Result<LibrarySummary, String> {
    let row = sqlx::query(
        r#"SELECT
            (SELECT COUNT(*) FROM tracks WHERE is_deleted = 0) AS tracks,
            (SELECT COUNT(*) FROM playlists WHERE is_deleted = 0) AS playlists,
            (SELECT COUNT(*) FROM liked_tracks) AS liked_tracks"#,
    )
    .fetch_one(&state.database)
    .await
    .map_err(|error| format!("Could not read library summary: {error}"))?;

    Ok(LibrarySummary {
        tracks: row.try_get("tracks").unwrap_or_default(),
        playlists: row.try_get("playlists").unwrap_or_default(),
        liked_tracks: row.try_get("liked_tracks").unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn get_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    lib_service::get_all_tracks(&state.database).await
}

/// Next track for an unqueued song, chosen from the finished track's tags.
/// Returns `null` when the library has nothing related, so the renderer stops
/// instead of inventing a continuation.
#[tauri::command]
pub async fn suggest_next_track(
    state: State<'_, AppState>,
    track_id: String,
) -> Result<Option<Track>, String> {
    lib_service::suggest_next_track(&state.database, &track_id).await
}

#[tauri::command]
pub async fn get_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    lib_service::get_albums(&state.database).await
}

#[tauri::command]
pub async fn get_artists(state: State<'_, AppState>) -> Result<Vec<Artist>, String> {
    lib_service::get_artists(&state.database).await
}

#[tauri::command]
pub async fn get_liked_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    lib_service::get_liked_tracks(&state.database).await
}

#[tauri::command]
pub async fn toggle_liked_track(
    state: State<'_, AppState>,
    track_id: String,
) -> Result<bool, String> {
    lib_service::toggle_track_liked(&state.database, &track_id).await
}

#[tauri::command]
pub async fn get_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    lib_service::get_playlists(&state.database).await
}

#[tauri::command]
pub async fn get_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: String,
) -> Result<Vec<Track>, String> {
    lib_service::get_playlist_tracks(&state.database, &playlist_id).await
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> Result<Playlist, String> {
    lib_service::create_playlist(&state.database, &name, &description).await
}

#[tauri::command]
pub async fn delete_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
) -> Result<(), String> {
    lib_service::delete_playlist(&state.database, &playlist_id).await
}

#[tauri::command]
pub async fn add_track_to_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
    track_id: String,
) -> Result<String, String> {
    lib_service::add_track_to_playlist(&state.database, &playlist_id, &track_id).await
}

#[tauri::command]
pub async fn add_provider_track_to_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
    provider_id: String,
    provider_track_id: String,
    title: String,
    artist: String,
    album: String,
    duration_ms: i64,
    artwork_uri: Option<String>,
) -> Result<(), String> {
    lib_service::add_provider_track_to_playlist(
        &state.database,
        &playlist_id,
        &provider_id,
        &provider_track_id,
        &title,
        &artist,
        &album,
        duration_ms,
        artwork_uri.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn remove_track_from_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
    position: i64,
) -> Result<(), String> {
    lib_service::remove_track_from_playlist(&state.database, &playlist_id, position).await
}

#[tauri::command]
pub async fn reorder_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: String,
    from_pos: i64,
    to_pos: i64,
) -> Result<(), String> {
    lib_service::reorder_playlist_tracks(&state.database, &playlist_id, from_pos, to_pos).await
}

#[tauri::command]
pub async fn update_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
    name: String,
    description: String,
) -> Result<Playlist, String> {
    lib_service::update_playlist(&state.database, &playlist_id, &name, &description).await
}

#[tauri::command]
pub async fn scan_music_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<ScanResult, String> {
    // Reset cancellation flag
    state.scan_cancelled.store(false, Ordering::Relaxed);

    let path = PathBuf::from(folder_path);
    let pool = state.database.clone();
    let artworks_dir = state.artworks_dir.clone();
    let cancel_flag = state.scan_cancelled.clone();
    let op_lock = state.artwork.op_lock();

    scanner::scan_directory(app, pool, path, artworks_dir, cancel_flag, op_lock).await
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) -> Result<(), String> {
    state.scan_cancelled.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn search_library(
    state: State<'_, AppState>,
    query: String,
    category: String,
) -> Result<SearchResultsDTO, String> {
    search_service::search_library(&state.database, &query, &category).await
}

#[tauri::command]
pub async fn record_recent_play(state: State<'_, AppState>, track: Track) -> Result<(), String> {
    lib_service::record_recent_play(&state.database, &track).await
}

/// Playback history shown on the Search page when no query is active. Search
/// queries themselves are never stored, only tracks that actually played.
#[tauri::command]
pub async fn get_recent_plays(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    lib_service::get_recent_plays(&state.database, 60).await
}

#[tauri::command]
pub async fn delete_track(state: State<'_, AppState>, track_id: String) -> Result<(), String> {
    lib_service::delete_track(&state.database, &track_id).await
}

#[tauri::command]
pub async fn delete_tracks(
    state: State<'_, AppState>,
    track_ids: Vec<String>,
) -> Result<usize, String> {
    lib_service::delete_tracks(&state.database, &track_ids).await
}

#[tauri::command]
pub async fn unlike_tracks(
    state: State<'_, AppState>,
    track_ids: Vec<String>,
) -> Result<usize, String> {
    lib_service::unlike_tracks(&state.database, &track_ids).await
}

#[tauri::command]
pub async fn remove_tracks_from_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
    positions: Vec<i64>,
) -> Result<(), String> {
    lib_service::remove_tracks_from_playlist(&state.database, &playlist_id, &positions).await
}

#[tauri::command]
pub async fn clear_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: String,
) -> Result<(), String> {
    lib_service::clear_playlist_tracks(&state.database, &playlist_id).await
}

#[tauri::command]
pub async fn clear_recent_plays(state: State<'_, AppState>) -> Result<(), String> {
    lib_service::clear_recent_plays(&state.database).await
}

#[tauri::command]
pub async fn delete_recent_play(
    state: State<'_, AppState>,
    track_id: String,
) -> Result<(), String> {
    lib_service::delete_recent_play(&state.database, &track_id).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStorageSummary {
    pub stream_cache_bytes: u64,
    pub stream_cache_files: usize,
    pub stream_protected_files: usize,
    pub stream_cache_limit_bytes: u64,
    pub lyrics_cache_entries: i64,
    pub recent_plays_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCacheCleanupResult {
    pub deleted_files: usize,
    pub reclaimed_bytes: u64,
    pub skipped_active_track: bool,
}

#[tauri::command]
pub async fn get_library_storage_summary(
    state: State<'_, AppState>,
) -> Result<LibraryStorageSummary, String> {
    let (stream_cache_bytes, stream_cache_files, stream_protected_files) =
        state.ytmusic.cache_summary();
    let (_, stream_cache_limit_bytes) = state.ytmusic.cache_usage_bytes();
    let lyrics_cache_entries = crate::services::lyrics::cache::count_cached_lyrics(&state.database)
        .await
        .unwrap_or(0);
    let recent_plays_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recent_plays")
        .fetch_one(&state.database)
        .await
        .unwrap_or(0);
    Ok(LibraryStorageSummary {
        stream_cache_bytes,
        stream_cache_files,
        stream_protected_files,
        stream_cache_limit_bytes,
        lyrics_cache_entries,
        recent_plays_count,
    })
}

#[tauri::command]
pub async fn clear_stream_cache(
    include_protected: Option<bool>,
    state: State<'_, AppState>,
) -> Result<StreamCacheCleanupResult, String> {
    let active_vid = state
        .playback
        .snapshot()
        .current_track
        .as_ref()
        .and_then(crate::services::preload::extract_ytmusic_video_id);
    let (deleted_files, reclaimed_bytes) = state
        .ytmusic
        .clear_cache(active_vid.as_deref(), include_protected.unwrap_or(false))
        .map_err(|e| format!("Failed to clear cached audio streams: {e}"))?;
    state.preload.refresh_snapshot();
    Ok(StreamCacheCleanupResult {
        deleted_files,
        reclaimed_bytes,
        skipped_active_track: active_vid.is_some(),
    })
}

#[tauri::command]
pub async fn clear_lyrics_cache(state: State<'_, AppState>) -> Result<u64, String> {
    crate::services::lyrics::cache::clear_all_cached_lyrics(&state.database)
        .await
        .map_err(|e| format!("Failed to clear cached lyrics: {e}"))
}

#[tauri::command]
pub async fn clear_artwork_cache(
    state: State<'_, AppState>,
) -> Result<crate::services::artwork::ArtworkCleanupResult, String> {
    state.artwork.clear_all_cache().await
}

#[tauri::command]
pub async fn toggle_playlist_pinned(
    state: State<'_, AppState>,
    playlist_id: String,
    pinned: bool,
) -> Result<Playlist, String> {
    lib_service::toggle_playlist_pinned(&state.database, &playlist_id, pinned).await
}

#[tauri::command]
pub async fn quick_add_track_to_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
    track: Track,
) -> Result<QuickAddPlaylistResult, String> {
    lib_service::quick_add_track_to_playlist(&state.database, &playlist_id, &track).await
}

#[tauri::command]
pub async fn remove_playlist_entry_by_id(
    state: State<'_, AppState>,
    playlist_id: String,
    entry_id: String,
) -> Result<(), String> {
    lib_service::remove_playlist_entry_by_id(&state.database, &playlist_id, &entry_id).await
}

#[tauri::command]
pub async fn get_recent_play_events(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<RecentPlayEvent>, String> {
    lib_service::get_recent_play_events(&state.database, limit.unwrap_or(100)).await
}

#[tauri::command]
pub async fn trigger_haptic_feedback(app: AppHandle, kind: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let handle = app.state::<crate::audio::android_media3::Media3PluginHandle>();
        let payload = serde_json::json!({ "kind": kind.unwrap_or_else(|| "enter".to_string()) });
        let _ = handle
            .0
            .run_mobile_plugin_async::<serde_json::Value>("triggerHaptic", payload)
            .await;
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, kind);
    Ok(())
}
