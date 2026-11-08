use serde::Serialize;
use sqlx::Row;
use tauri::State;

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
        "SELECT\n            (SELECT COUNT(*) FROM tracks WHERE is_deleted = 0) AS tracks,\n            (SELECT COUNT(*) FROM playlists WHERE is_deleted = 0) AS playlists,\n            (SELECT COUNT(*) FROM liked_tracks) AS liked_tracks",
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
