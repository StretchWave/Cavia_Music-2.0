use sqlx::SqlitePool;

/// Shared application state. Long-lived services are added here as concrete
/// adapters are implemented; routes never construct their own copies.
pub struct AppState {
    pub database: SqlitePool,
}

impl AppState {
    pub fn new(database: SqlitePool) -> Self {
        Self { database }
    }
}
