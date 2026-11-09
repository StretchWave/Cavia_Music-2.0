pub mod audio;
mod commands;
pub mod domain;
pub mod services;
mod state;

use services::playback::PlaybackService;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use state::AppState;
use std::{fs, io};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("cavia-library.sqlite3");
            let artworks_dir = app_data_dir.join("artworks");
            fs::create_dir_all(&artworks_dir)?;

            let pool = tauri::async_runtime::block_on(async move {
                let options = SqliteConnectOptions::new()
                    .filename(database_path)
                    .create_if_missing(true)
                    .foreign_keys(true)
                    .journal_mode(SqliteJournalMode::Wal);

                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(options)
                    .await?;

                sqlx::migrate!("./migrations").run(&pool).await?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(pool)
            })
            .map_err(io::Error::other)?;

            let playback = PlaybackService::new(pool.clone()).map_err(|e| {
                io::Error::other(format!("Playback engine initialization failed: {e}"))
            })?;

            // Restore saved playback state from SQLite asynchronously
            let playback_for_restore = playback.clone();
            tauri::async_runtime::block_on(async move {
                if let Ok(mut svc) = playback_for_restore.lock() {
                    let _ = svc.restore_saved_state().await;
                }
            });

            {
                if let Ok(mut svc) = playback.lock() {
                    svc.set_app_handle(app.handle().clone());
                    svc.init_media_controls(None);
                }
            }

            app.manage(AppState::new(pool, artworks_dir, playback));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_health,
            commands::library::library_summary,
            commands::library::get_tracks,
            commands::library::get_albums,
            commands::library::get_artists,
            commands::library::get_liked_tracks,
            commands::library::toggle_liked_track,
            commands::library::get_playlists,
            commands::library::get_playlist_tracks,
            commands::library::create_playlist,
            commands::library::delete_playlist,
            commands::library::scan_music_directory,
            commands::library::cancel_scan,
            commands::playback::get_playback_state,
            commands::playback::play_track,
            commands::playback::play,
            commands::playback::pause,
            commands::playback::play_pause,
            commands::playback::stop,
            commands::playback::seek,
            commands::playback::set_volume,
            commands::playback::toggle_mute,
            commands::playback::set_playback_speed,
            commands::playback::set_repeat_mode,
            commands::playback::set_shuffle,
            commands::playback::next_track,
            commands::playback::previous_track,
            commands::playback::set_queue,
            commands::playback::add_to_queue,
            commands::playback::remove_from_queue,
            commands::playback::remove_from_queue_by_index,
            commands::playback::reorder_queue,
            commands::playback::clear_queue,
        ])
        .run(tauri::generate_context!())
        .expect("Cavia Music failed to start");
}
