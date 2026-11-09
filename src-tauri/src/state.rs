use crate::services::playback::SharedPlaybackService;
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
    pub playback: SharedPlaybackService,
}

impl AppState {
    pub fn new(
        database: SqlitePool,
        artworks_dir: PathBuf,
        playback: SharedPlaybackService,
    ) -> Self {
        Self {
            database,
            artworks_dir,
            scan_cancelled: Arc::new(AtomicBool::new(false)),
            playback,
        }
    }
}
