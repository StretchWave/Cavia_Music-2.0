use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub duration_ms: i64,
    pub artwork_uri: Option<String>,
    pub source_kind: String,
    pub provider_id: Option<String>,
    pub file_uri: Option<String>,
    pub release_year: Option<i32>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub genre: Option<String>,
    pub is_liked: bool,
    pub play_count: i64,
    pub date_added: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub track_count: i64,
    pub artwork_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub track_count: i64,
    pub album_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: String,
    pub artwork_uri: Option<String>,
    pub source_provider: Option<String>,
    pub track_count: i64,
    pub created_at: String,
    pub tracks: Option<Vec<Track>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub phase: String, // "discovering", "scanning", "completed", "cancelled", "error"
    pub current_file: String,
    pub discovered_count: usize,
    pub processed_count: usize,
    pub new_tracks_count: usize,
    pub duplicates_count: usize,
    pub errors_count: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub total_discovered: usize,
    pub new_tracks: usize,
    pub duplicates: usize,
    pub errors: usize,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownErrorPayload {
    pub error: String,
}
