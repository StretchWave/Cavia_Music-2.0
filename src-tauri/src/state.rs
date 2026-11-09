use crate::services::playback::PlaybackService;
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
}

impl AppState {
    pub fn new(database: SqlitePool, artworks_dir: PathBuf, playback: PlaybackService) -> Self {
        Self {
            database,
            artworks_dir,
            scan_cancelled: Arc::new(AtomicBool::new(false)),
            playback,
        }
    }
}
