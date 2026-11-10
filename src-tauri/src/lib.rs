pub mod audio;
mod commands;
pub mod domain;
pub mod services;
mod state;

use services::playback::PlaybackService;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, io};
use tauri::{Manager, RunEvent};

/// Guards against re-running shutdown when `ExitRequested` is emitted more
/// than once (for example while the explicit `exit(0)` is unwinding).
static SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
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

            let playback =
                PlaybackService::new(pool.clone(), Some(app.handle().clone())).map_err(|e| {
                    io::Error::other(format!("Playback engine initialization failed: {e}"))
                })?;

            // Restore saved playback state from SQLite. This runs on the
            // synchronous setup thread; the persistence worker owns its own
            // runtime, so no ambient Tokio context is required.
            let _ = tauri::async_runtime::block_on(playback.restore_saved_state());

            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    if let Ok(hwnd) = window.hwnd() {
                        let _ = tauri::async_runtime::block_on(async {
                            playback.init_media_controls(Some(hwnd.0 as isize)).await
                        });
                    }
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = tauri::async_runtime::block_on(async {
                    playback.init_media_controls(None).await
                });
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
            commands::playback::retry_audio_device,
        ])
        .build(tauri::generate_context!())
        .expect("Cavia Music failed to build");

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { api, .. } = event {
            if SHUTDOWN_STARTED.swap(true, Ordering::SeqCst) {
                // The shutdown below already finished and requested exit.
                return;
            }
            api.prevent_exit();

            let state = app_handle.state::<AppState>();
            let _ = tauri::async_runtime::block_on(state.playback.shutdown());
            app_handle.exit(0);
        }
    });
}
