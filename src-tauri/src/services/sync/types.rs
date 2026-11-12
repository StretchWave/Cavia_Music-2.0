use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Playlist,
    PlaylistTrack,
    LikedTrack,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Playlist => "playlist",
            Self::PlaylistTrack => "playlist_track",
            Self::LikedTrack => "liked_track",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "playlist" => Some(Self::Playlist),
            "playlist_track" => Some(Self::PlaylistTrack),
            "liked_track" => Some(Self::LikedTrack),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Create,
    Update,
    Delete,
    Add,
    Remove,
    Reorder,
    Like,
    Unlike,
}

impl OperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Reorder => "reorder",
            Self::Like => "like",
            Self::Unlike => "unlike",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "create" => Some(Self::Create),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            "add" => Some(Self::Add),
            "remove" => Some(Self::Remove),
            "reorder" => Some(Self::Reorder),
            "like" => Some(Self::Like),
            "unlike" => Some(Self::Unlike),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAccountStatus {
    pub authenticated: bool,
    pub account_id: Option<String>,
    pub account_email: Option<String>,
    pub device_id: String,
    pub sync_enabled: bool,
    pub last_synced_at: Option<String>,
    pub pending_outbox_count: usize,
    pub last_error: Option<String>,
    pub backend_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub pushed_count: usize,
    pub pulled_count: usize,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutboxItem {
    pub operation_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub operation_type: String,
    pub payload_json: String,
    pub created_at: String,
    pub attempt_count: i64,
    pub last_error: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistMutationPayload {
    pub id: String,
    pub name: String,
    pub description: String,
    pub client_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistTrackAddPayload {
    pub entry_id: String,
    pub playlist_id: String,
    pub cloud_track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub content_hash: Option<String>,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistTrackRemovePayload {
    pub entry_id: String,
    pub playlist_id: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistTracksReorderPayload {
    pub playlist_id: String,
    pub from_position: i64,
    pub to_position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LikedTrackPayload {
    pub cloud_track_id: String,
    pub content_hash: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub is_liked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfigDTO {
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub is_configured: bool,
}
