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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contributor_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contributor_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority_override: Option<bool>,
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
    #[serde(default)]
    pub artwork_uri: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultsDTO {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    pub playlists: Vec<Playlist>,
    pub query: String,
    pub category: String,
    pub total_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackOrigin {
    SearchSelection,
    ManualLibrarySelection,
    ManualPlaylistSelection,
    ManualQueueSelection,
    AutomaticQueueProgression,
    Autoplay,
    AutomaticShuffle,
    AutomaticRecommendation,
}

impl Default for PlaybackOrigin {
    fn default() -> Self {
        Self::AutomaticQueueProgression
    }
}

impl PlaybackOrigin {
    pub fn is_intentional(self) -> bool {
        matches!(
            self,
            Self::SearchSelection
                | Self::ManualLibrarySelection
                | Self::ManualPlaylistSelection
                | Self::ManualQueueSelection
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SearchSelection => "search_selection",
            Self::ManualLibrarySelection => "manual_library_selection",
            Self::ManualPlaylistSelection => "manual_playlist_selection",
            Self::ManualQueueSelection => "manual_queue_selection",
            Self::AutomaticQueueProgression => "automatic_queue_progression",
            Self::Autoplay => "autoplay",
            Self::AutomaticShuffle => "automatic_shuffle",
            Self::AutomaticRecommendation => "automatic_recommendation",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAddPlaylistResult {
    pub status: String, // "added" | "already_present"
    pub entry_id: Option<String>,
    pub playlist_id: String,
    pub playlist_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentPlayEvent {
    pub id: i64,
    pub track_id: String,
    pub source_kind: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub playback_origin: String,
    pub played_at: String,
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
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub last_used_at: Option<String>,
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
