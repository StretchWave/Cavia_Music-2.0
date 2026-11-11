pub mod audio;
mod commands;
pub mod domain;
pub mod services;
mod state;

use services::playback::PlaybackService;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use state::AppState;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, io};
use tauri::{Emitter, Manager, RunEvent};

/// Guards against re-running shutdown when `ExitRequested` is emitted more
/// than once (for example while the explicit `exit(0)` is unwinding).
static SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, PartialEq, Eq)]
pub enum ExitDecision {
    ExitSuccess,
    BlockExitAndReport(String),
}

pub async fn handle_exit_request(playback: &PlaybackService, force_exit: bool) -> ExitDecision {
    if force_exit {
        let _ = playback.shutdown().await;
        return ExitDecision::ExitSuccess;
    }

    match playback.shutdown().await {
        Ok(()) => ExitDecision::ExitSuccess,
        Err(err) => ExitDecision::BlockExitAndReport(err),
    }
}

/// Resolves the application data directory.
///
/// In debug/test builds (`cfg(debug_assertions)`), developers and test runners may set
/// `CAVIA_APP_DATA_DIR` to redirect database and cache directories into isolated temporary paths.
/// In production/release builds, `CAVIA_APP_DATA_DIR` is strictly ignored to prevent unauthorized
/// directory redirection; the platform-resolved application data directory is always used.
pub fn resolve_app_data_dir_internal(
    platform_dir: PathBuf,
    env_override: Option<&str>,
    is_debug: bool,
) -> PathBuf {
    if is_debug {
        if let Some(val) = env_override {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
    }
    platform_dir
}

pub fn resolve_app_data_dir(platform_dir: PathBuf) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let env_val = std::env::var("CAVIA_APP_DATA_DIR").ok();
        resolve_app_data_dir_internal(platform_dir, env_val.as_deref(), true)
    }

    #[cfg(not(debug_assertions))]
    {
        resolve_app_data_dir_internal(platform_dir, None, false)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let platform_dir = app.path().app_data_dir().unwrap_or_default();
            let app_data_dir = resolve_app_data_dir(platform_dir);
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

            let lyrics = services::lyrics::LyricsService::new(pool.clone()).map_err(|e| {
                io::Error::other(format!("Lyrics engine initialization failed: {e}"))
            })?;

            let artwork =
                services::artwork::ArtworkService::new(pool.clone(), artworks_dir.clone());

            app.manage(AppState::new(pool, artworks_dir, playback, lyrics, artwork));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_health,
            commands::artwork::get_artwork_cache_summary,
            commands::artwork::clean_orphaned_artwork,
            commands::artwork::recover_missing_artwork,
            commands::artwork::recover_track_artwork,
            commands::lyrics::get_track_lyrics,
            commands::lyrics::refresh_track_lyrics,
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
            commands::library::add_track_to_playlist,
            commands::library::remove_track_from_playlist,
            commands::library::reorder_playlist_tracks,
            commands::library::update_playlist,
            commands::library::search_library,
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
            commands::playback::retry_shutdown,
            commands::playback::force_exit,
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
            let decision =
                tauri::async_runtime::block_on(handle_exit_request(&state.playback, false));
            match decision {
                ExitDecision::ExitSuccess => {
                    app_handle.exit(0);
                }
                ExitDecision::BlockExitAndReport(err) => {
                    SHUTDOWN_STARTED.store(false, Ordering::SeqCst);
                    eprintln!("[Cavia Exit] Graceful shutdown failed: {err}");
                    let payload = domain::ShutdownErrorPayload {
                        error: format!("Failed to save playback state: {err}"),
                    };
                    let _ = app_handle.emit("app:shutdown-error", &payload);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::domain::ShutdownErrorPayload;

    #[test]
    fn test_shutdown_error_payload_contract() {
        let err = "database disk image is malformed (code 11)";
        let payload = ShutdownErrorPayload {
            error: format!("Failed to save playback state: {err}"),
        };
        let json_emitted =
            serde_json::to_string(&payload).expect("Failed to serialize ShutdownErrorPayload");

        // Verify exact emitted JSON string structure
        assert_eq!(
            json_emitted,
            r#"{"error":"Failed to save playback state: database disk image is malformed (code 11)"}"#
        );

        // Verify frontend contract deserialization succeeds
        let parsed: ShutdownErrorPayload = serde_json::from_str(&json_emitted)
            .expect("Payload emitted by backend must deserialize into ShutdownErrorPayload");
        assert_eq!(
            parsed.error,
            "Failed to save playback state: database disk image is malformed (code 11)"
        );
    }

    #[test]
    fn test_resolve_app_data_dir_debug_honors_override() {
        let platform_dir = std::path::PathBuf::from(r"C:\DefaultAppData");
        let override_dir = r"C:\IsolatedTestDir";
        let resolved = crate::resolve_app_data_dir_internal(
            platform_dir.clone(),
            Some(override_dir),
            true, // is_debug = true
        );
        assert_eq!(resolved, std::path::PathBuf::from(override_dir));
    }

    #[test]
    fn test_resolve_app_data_dir_release_ignores_override() {
        let platform_dir = std::path::PathBuf::from(r"C:\DefaultAppData");
        let override_dir = r"C:\IsolatedTestDir";
        let resolved = crate::resolve_app_data_dir_internal(
            platform_dir.clone(),
            Some(override_dir),
            false, // is_debug = false (simulates release build)
        );
        assert_eq!(resolved, platform_dir);
    }

    #[test]
    fn test_resolve_app_data_dir_empty_override_ignored() {
        let platform_dir = std::path::PathBuf::from(r"C:\DefaultAppData");
        let resolved = crate::resolve_app_data_dir_internal(
            platform_dir.clone(),
            Some("   "),
            true, // is_debug = true
        );
        assert_eq!(resolved, platform_dir);
    }
}
