pub mod audio;
mod commands;
pub mod domain;
pub mod services;
mod state;

use services::playback::PlaybackService;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
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
    if is_debug && let Some(val) = env_override {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
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
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(audio::android_media3::init());
    let app = builder
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

                let migrator = sqlx::migrate!("./migrations");
                migrator.run(&pool).await?;
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
            if let Err(err) = tauri::async_runtime::block_on(playback.restore_saved_state()) {
                eprintln!("[Cavia Startup] Could not restore saved playback state: {err}");
            }

            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main")
                    && let Ok(hwnd) = window.hwnd()
                {
                    let _ = tauri::async_runtime::block_on(async {
                        playback.init_media_controls(Some(hwnd.0 as isize)).await
                    });
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

            #[cfg(target_os = "android")]
            let credential_store = services::credentials::KeyringStore::new(
                app.state::<audio::android_media3::Media3PluginHandle>()
                    .0
                    .clone(),
            );
            #[cfg(not(target_os = "android"))]
            let credential_store = services::credentials::KeyringStore::new();
            let credentials: Arc<dyn services::credentials::CredentialStore> =
                Arc::new(credential_store);

            let sync = Arc::new(services::sync::SyncEngine::new(
                pool.clone(),
                credentials.clone(),
            ));

            let ytmusic = Arc::new(
                services::ytmusic::YtMusicService::new(app_data_dir.join("stream-cache"))
                    .map_err(io::Error::other)?,
            );
            tauri::async_runtime::block_on(playback.attach_ytmusic(ytmusic.clone()))
                .map_err(io::Error::other)?;

            let import_manager = Arc::new(services::spotify::ImportManager::new(
                pool.clone(),
                ytmusic.clone(),
            ));

            let offline = services::offline::OfflineFallbackService::new(
                pool.clone(),
                ytmusic.clone(),
                Some(app.handle().clone()),
            );
            let connect = services::connect::ConnectService::new(
                pool.clone(),
                Some(app.handle().clone()),
                Some(ytmusic.clone()),
                Some(sync.clone()),
            );
            let connect_accounts = services::connect_accounts::ConnectAccountService::new(
                pool.clone(),
                sync.clone(),
                Some(app.handle().clone()),
            );

            let preload_target = tauri::async_runtime::block_on(
                services::preload::PreloadCoordinator::load_persisted_target(&pool),
            );
            let app_preload_emit = app.handle().clone();
            let connect_for_late = connect.clone();
            let playback_for_late = playback.clone();
            let offline_for_late = offline.clone();
            let offline_for_conn = offline.clone();
            let playback_for_conn = playback.clone();

            let preload = services::preload::PreloadCoordinator::new(
                ytmusic.clone(),
                Some(pool.clone()),
                preload_target,
                services::preload::PreloadCallbacks {
                    on_status_changed: Arc::new(move |snap| {
                        let _ = app_preload_emit.emit("playback:preload-changed", snap);
                    }),
                    on_late_verified: Arc::new(move |entry_id, _path| {
                        let connect_c = connect_for_late.clone();
                        let playback_c = playback_for_late.clone();
                        let offline_c = offline_for_late.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Ok(Some(_)) =
                                connect_c.promote_pending_entry_ready(&entry_id).await
                                && !offline_c.is_fallback_active()
                                && let Some((cur, up)) = connect_c.upcoming_playback_tracks()
                            {
                                let _ = playback_c
                                    .apply_collaborative_sequence(cur, up, false)
                                    .await;
                            }
                        });
                    }),
                    on_connectivity_lost: Arc::new(move |_err| {
                        let offline_c = offline_for_conn.clone();
                        let playback_c = playback_for_conn.clone();
                        tauri::async_runtime::spawn(async move {
                            let queue_tracks = playback_c.snapshot().queue;
                            offline_c.notify_connectivity_lost(&queue_tracks).await;
                        });
                    }),
                },
            );

            connect.attach_preload(preload.clone());
            tauri::async_runtime::block_on(playback.attach_coordinators(
                preload.clone(),
                connect.clone(),
                offline.clone(),
            ))
            .map_err(io::Error::other)?;

            if let Ok(Some(_)) = tauri::async_runtime::block_on(connect.restore_active_session()) {
                if let Some((cur, up)) = connect.upcoming_playback_tracks() {
                    let _ = tauri::async_runtime::block_on(
                        playback.apply_collaborative_sequence(cur, up, false),
                    );
                }
                let tag =
                    tauri::async_runtime::block_on(connect_accounts.active_metadata_tag_if_shown());
                let _ = tauri::async_runtime::block_on(playback.set_connect_metadata_tag(tag));
            }

            app.manage(AppState::new(
                pool,
                artworks_dir,
                playback,
                lyrics,
                artwork,
                credentials,
                sync,
                ytmusic,
                import_manager,
                preload,
                connect,
                connect_accounts,
                offline,
            ));
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
            commands::spotify::spotify_start_import,
            commands::spotify::ytmusic_start_import,
            commands::spotify::playlist_start_import,
            commands::spotify::import_pause,
            commands::spotify::import_resume,
            commands::spotify::import_cancel,
            commands::spotify::import_get_jobs,
            commands::spotify::import_clear_completed,
            commands::spotify::import_remove_job,
            commands::spotify::import_cancel_and_delete,
            commands::spotify::spotify_import_playlist,
            commands::ytmusic::ytmusic_search,
            commands::ytmusic::ytmusic_cancel_search,
            commands::ytmusic::ytmusic_related,
            commands::ytmusic::ytmusic_play,
            commands::library::library_summary,
            commands::library::get_tracks,
            commands::library::delete_track,
            commands::library::delete_tracks,
            commands::library::unlike_tracks,
            commands::library::suggest_next_track,
            commands::library::get_albums,
            commands::library::get_artists,
            commands::library::get_liked_tracks,
            commands::library::get_recent_plays,
            commands::library::record_recent_play,
            commands::library::clear_recent_plays,
            commands::library::delete_recent_play,
            commands::library::toggle_liked_track,
            commands::library::get_playlists,
            commands::library::get_playlist_tracks,
            commands::library::create_playlist,
            commands::library::delete_playlist,
            commands::library::clear_playlist_tracks,
            commands::library::add_track_to_playlist,
            commands::library::add_provider_track_to_playlist,
            commands::library::remove_track_from_playlist,
            commands::library::remove_tracks_from_playlist,
            commands::library::reorder_playlist_tracks,
            commands::library::update_playlist,
            commands::library::search_library,
            commands::library::scan_music_directory,
            commands::library::pick_android_music_folder,
            commands::library::cancel_scan,
            commands::library::get_library_storage_summary,
            commands::library::clear_stream_cache,
            commands::library::clear_lyrics_cache,
            commands::library::clear_artwork_cache,
            commands::library::toggle_playlist_pinned,
            commands::library::quick_add_track_to_playlist,
            commands::library::remove_playlist_entry_by_id,
            commands::library::get_recent_play_events,
            commands::library::trigger_haptic_feedback,
            commands::sync::sync_get_status,
            commands::sync::credential_storage_self_test,
            commands::sync::sync_signup,
            commands::sync::sync_login,
            commands::sync::sync_logout,
            commands::sync::sync_toggle,
            commands::sync::sync_now,
            commands::sync::sync_delete_cloud_data,
            commands::sync::sync_get_config,
            commands::sync::sync_set_config,
            commands::playback::get_playback_state,
            commands::playback::play_track,
            commands::playback::play_queue_index,
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
            commands::playback::set_playback_queue,
            commands::playback::add_to_queue,
            commands::playback::play_next,
            commands::playback::enqueue_track,
            commands::playback::remove_from_queue,
            commands::playback::remove_from_queue_by_index,
            commands::playback::reorder_queue,
            commands::playback::clear_queue,
            commands::playback::retry_audio_device,
            commands::playback::retry_shutdown,
            commands::playback::force_exit,
            commands::connect::preload_get_status,
            commands::connect::preload_set_target,
            commands::connect::preload_retry,
            commands::connect::offline_get_status,
            commands::connect::offline_choose_shuffle_cached,
            commands::connect::offline_decline_fallback,
            commands::connect::offline_exit_fallback,
            commands::connect::offline_probe_reconnect,
            commands::connect::connect_get_session,
            commands::connect::connect_create_session,
            commands::connect::connect_join_session,
            commands::connect::connect_join_approved_session,
            commands::connect::connect_set_participant_connected,
            commands::connect::connect_contribute_track,
            commands::connect::connect_contribute_ytmusic_playlist,
            commands::connect::connect_remove_entry,
            commands::connect::connect_host_reorder,
            commands::connect::connect_set_alpha,
            commands::connect::connect_sync_now,
            commands::connect::connect_end_session,
            commands::connect::connect_get_account_hub,
            commands::connect::connect_update_account_settings,
            commands::connect::connect_lookup_host,
            commands::connect::connect_send_join_request,
            commands::connect::connect_respond_request,
            commands::connect::connect_unignore_guest,
            commands::connect::connect_save_host,
            commands::connect::connect_remove_saved_host,
            commands::connect::connect_to_saved_host,
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
