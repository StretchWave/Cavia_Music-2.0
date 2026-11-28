use crate::services::artwork::ArtworkService;
use crate::services::credentials::CredentialStore;
use crate::services::lyrics::LyricsService;
use crate::services::playback::PlaybackService;
use crate::services::sync::SyncEngine;
use crate::services::ytmusic::YtMusicService;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Shared application state. Long-lived services are added here as concrete
/// adapters are implemented; routes never construct their own copies.
pub struct AppState {
    pub database: SqlitePool,
    pub artworks_dir: PathBuf,
    pub scan_cancelled: Arc<AtomicBool>,
    pub playback: PlaybackService,
    pub lyrics: LyricsService,
    pub artwork: ArtworkService,
    #[allow(dead_code)]
    pub credentials: Arc<dyn CredentialStore>,
    pub sync: Arc<SyncEngine>,
    pub ytmusic: Arc<YtMusicService>,
    pub import_manager: Arc<crate::services::spotify::ImportManager>,
    pub preload: Arc<crate::services::preload::PreloadCoordinator>,
    pub connect: Arc<crate::services::connect::ConnectService>,
    pub connect_accounts: Arc<crate::services::connect_accounts::ConnectAccountService>,
    pub offline: Arc<crate::services::offline::OfflineFallbackService>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        database: SqlitePool,
        artworks_dir: PathBuf,
        playback: PlaybackService,
        lyrics: LyricsService,
        artwork: ArtworkService,
        credentials: Arc<dyn CredentialStore>,
        sync: Arc<SyncEngine>,
        ytmusic: Arc<YtMusicService>,
        import_manager: Arc<crate::services::spotify::ImportManager>,
        preload: Arc<crate::services::preload::PreloadCoordinator>,
        connect: Arc<crate::services::connect::ConnectService>,
        connect_accounts: Arc<crate::services::connect_accounts::ConnectAccountService>,
        offline: Arc<crate::services::offline::OfflineFallbackService>,
    ) -> Self {
        Self {
            database,
            artworks_dir,
            scan_cancelled: Arc::new(AtomicBool::new(false)),
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
        }
    }
}
