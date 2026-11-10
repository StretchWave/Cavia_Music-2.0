use serde::Serialize;
use sqlx::Row;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

use crate::domain::{Album, Artist, Playlist, ScanResult, SearchResultsDTO, Track};
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

    scanner::scan_directory(app, pool, path, artworks_dir, cancel_flag).await
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
