//! Lightweight Spotify playlist reading and background matching manager.
//!
//! Only public playlist metadata is read from the Spotify embed page.
//! Track records are immediately inserted into SQLite (fast import),
//! and matching against the local library and YouTube Music runs in a
//! manageable background worker with pause/cancel controls.
//!
//! The embed payload carries at most 100 tracks. For longer playlists the
//! ordered track URIs come from Spotify's anonymous spclient endpoint (using
//! the short-lived token embedded in the same page), and metadata for the
//! overflow is read from each track's public embed page. Imports stop at
//! [`MAX_IMPORT_TRACKS`] so a single link cannot open an unbounded job.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Notify, RwLock};
use uuid::Uuid;

const EMBED_URL: &str = "https://open.spotify.com/embed/playlist";
const EMBED_TRACK_URL: &str = "https://open.spotify.com/embed/track";
const SPCLIENT_URL: &str = "https://spclient.wg.spotify.com/playlist/v2/playlist";

/// The public embed payload stops at this many tracks.
const EMBED_TRACK_LIMIT: usize = 100;
/// Upper bound for one import.
const MAX_IMPORT_TRACKS: usize = 1000;
/// Parallel track-metadata requests when reading past the embed cap.
const TRACK_FETCH_CONCURRENCY: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyPlaylist {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tracks: Vec<SpotifyTrack>,
    /// How many tracks Spotify reports for the playlist before Cavia's cap.
    #[serde(skip)]
    pub provider_total: usize,
    /// Tracks listed by Spotify whose public metadata could not be read.
    #[serde(skip)]
    pub unreadable_tracks: usize,
    /// True when Spotify lists more tracks than [`MAX_IMPORT_TRACKS`].
    #[serde(skip)]
    pub truncated: bool,
    /// True when the full-list endpoint was unavailable and the embed page
    /// exposed only its first page of tracks.
    #[serde(skip)]
    pub partial_embed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJob {
    pub id: String,
    pub source: String,
    pub playlist_id: String,
    pub playlist_name: String,
    pub total: usize,
    pub processed: usize,
    pub matched_local: usize,
    pub matched_online: usize,
    pub skipped: usize,
    pub current_track: Option<String>,
    pub status: ImportStatus,
    pub error: Option<String>,
    /// Honest note about tracks the provider listed but Cavia could not import.
    pub notice: Option<String>,
    pub created_at_ms: u64,
}

struct JobControl {
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

pub struct SpotifyImportService {
    client: reqwest::Client,
}

impl SpotifyImportService {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0",
            )
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("Could not create HTTP client for Spotify: {e}"))?;
        Ok(Self { client })
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn fetch_playlist(&self, url: &str) -> Result<SpotifyPlaylist, String> {
        let id = playlist_id_from_url(url)
            .ok_or_else(|| "That does not look like a Spotify playlist link".to_string())?;

        let response = self
            .client
            .get(format!("{EMBED_URL}/{id}"))
            .header("Accept", "text/html")
            .send()
            .await
            .map_err(|e| format!("Spotify is unreachable: {e}"))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(
                "Spotify returned 404 for that playlist. It may be private or removed.".into(),
            );
        }
        if !response.status().is_success() {
            return Err(format!(
                "Spotify returned status {}",
                response.status().as_u16()
            ));
        }

        let html = response
            .text()
            .await
            .map_err(|e| format!("Could not read the Spotify page: {e}"))?;

        let mut parsed = parse_playlist(&html, &id)?;

        // The embed page stops at ~100 tracks. Read the full ordered track
        // list from spclient and fill in anything the embed omitted from
        // individual track embeds. When spclient is unavailable this quietly
        // keeps the embed tracks rather than failing the import.
        let mut provider_total = parsed.playlist.tracks.len();
        let mut unreadable_tracks = 0;
        let mut read_full_list = false;
        if let Some(token) = parsed.access_token.clone() {
            if let Ok((ids, total)) = self.fetch_playlist_track_ids(&id, &token).await {
                read_full_list = true;
                provider_total = total;
                if ids.len() > parsed.playlist.tracks.len() {
                    unreadable_tracks = self.fill_overflow_tracks(&mut parsed, ids).await;
                }
            }
        }

        parsed.playlist.partial_embed =
            !read_full_list && parsed.playlist.tracks.len() >= EMBED_TRACK_LIMIT;
        parsed.playlist.truncated = provider_total > MAX_IMPORT_TRACKS;
        parsed.playlist.provider_total = provider_total;
        parsed.playlist.unreadable_tracks = unreadable_tracks;
        parsed.playlist.tracks.truncate(MAX_IMPORT_TRACKS);
        Ok(parsed.playlist)
    }

    /// Reads the complete, ordered track list for a playlist through Spotify's
    /// anonymous spclient endpoint.
    async fn fetch_playlist_track_ids(
        &self,
        playlist_id: &str,
        token: &str,
    ) -> Result<(Vec<String>, usize), String> {
        let response = self
            .client
            .get(format!("{SPCLIENT_URL}/{playlist_id}"))
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("Spotify track list is unreachable: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Spotify track list returned status {}",
                response.status().as_u16()
            ));
        }

        let value: Value = response
            .json()
            .await
            .map_err(|e| format!("Could not read the Spotify track list: {e}"))?;

        let items = value
            .get("contents")
            .and_then(|contents| contents.get("items"))
            .and_then(Value::as_array)
            .ok_or_else(|| "Spotify track list had no items".to_string())?;

        let ids: Vec<String> = items
            .iter()
            .filter_map(|item| item.get("uri").and_then(Value::as_str))
            .filter_map(|uri| uri.strip_prefix("spotify:track:"))
            .take(MAX_IMPORT_TRACKS)
            .map(str::to_string)
            .collect();

        if ids.is_empty() {
            return Err("Spotify track list had no tracks".into());
        }

        let total = value
            .get("length")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(items.len())
            .max(ids.len());

        Ok((ids, total))
    }

    /// Rebuilds the playlist in spclient order, fetching metadata for tracks
    /// the 100-track embed payload did not include.
    async fn fill_overflow_tracks(&self, parsed: &mut ParsedPlaylist, ids: Vec<String>) -> usize {
        let known: HashMap<String, SpotifyTrack> = parsed
            .playlist
            .tracks
            .iter()
            .zip(parsed.track_ids.iter())
            .filter_map(|(track, id)| id.as_ref().map(|id| (id.clone(), track.clone())))
            .collect();

        let mut missing: Vec<String> = ids
            .iter()
            .filter(|id| !known.contains_key(*id))
            .cloned()
            .collect();
        missing.sort_unstable();
        missing.dedup();
        missing.truncate(MAX_IMPORT_TRACKS.saturating_sub(known.len()));

        let mut fetched: HashMap<String, SpotifyTrack> = HashMap::new();
        for chunk in missing.chunks(TRACK_FETCH_CONCURRENCY) {
            let mut handles = Vec::with_capacity(chunk.len());
            for id in chunk {
                let client = self.client.clone();
                let id = id.clone();
                handles.push(tokio::spawn(async move {
                    fetch_embed_track(&client, &id)
                        .await
                        .map(|track| (id, track))
                }));
            }
            for handle in handles {
                if let Ok(Some((id, track))) = handle.await {
                    fetched.insert(id, track);
                }
            }
            if chunk.len() == TRACK_FETCH_CONCURRENCY {
                // Courteous spacing so a large playlist does not burst Spotify.
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
        }

        let expected = ids.len().min(MAX_IMPORT_TRACKS);
        let mut ordered = Vec::with_capacity(expected);
        for id in ids.into_iter().take(MAX_IMPORT_TRACKS) {
            if let Some(track) = known.get(&id).or_else(|| fetched.get(&id)) {
                ordered.push(track.clone());
            }
        }

        let unreadable = expected.saturating_sub(ordered.len());
        parsed.playlist.tracks = ordered;
        parsed.track_ids.clear();
        unreadable
    }
}

/// Metadata for one track read from its public embed page.
async fn fetch_embed_track(client: &reqwest::Client, track_id: &str) -> Option<SpotifyTrack> {
    let response = client
        .get(format!("{EMBED_TRACK_URL}/{track_id}"))
        .header("Accept", "text/html")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let html = response.text().await.ok()?;
    let json_text =
        extract_next_data(&html).or_else(|| extract_script_json(&html, "application/json"))?;
    let value: Value = serde_json::from_str(&json_text).ok()?;
    let entity = find_track_entity(&value)?;
    parse_track_entity(entity)
}

/// Accepts `open.spotify.com/playlist/<id>`, `spotify:playlist:<id>` and bare
/// ids. Query strings and locale prefixes are ignored.
pub fn playlist_id_from_url(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = if let Some(rest) = trimmed.strip_prefix("spotify:playlist:") {
        rest.split('?').next().unwrap_or(rest)
    } else if let Some(index) = trimmed.find("/playlist/") {
        let rest = &trimmed[index + "/playlist/".len()..];
        rest.split(['?', '/', '#']).next().unwrap_or(rest)
    } else if trimmed.starts_with("http") {
        return None;
    } else {
        trimmed.split('?').next().unwrap_or(trimmed)
    };

    let candidate = candidate.trim();
    let valid = candidate.len() == 22 && candidate.chars().all(|c| c.is_ascii_alphanumeric());
    valid.then(|| candidate.to_string())
}

/// One parsed embed payload: public tracks plus the internals needed to
/// paginate past the embed cap (track ids and the anonymous session token).
struct ParsedPlaylist {
    playlist: SpotifyPlaylist,
    track_ids: Vec<Option<String>>,
    access_token: Option<String>,
}

fn parse_playlist(html: &str, fallback_id: &str) -> Result<ParsedPlaylist, String> {
    let json_text = extract_next_data(html)
        .or_else(|| extract_script_json(html, "application/json"))
        .ok_or_else(|| "Spotify returned a page without playlist data".to_string())?;

    let value: Value = serde_json::from_str(&json_text)
        .map_err(|e| format!("Could not read the Spotify playlist data: {e}"))?;

    let entity = find_entity(&value)
        .ok_or_else(|| "Spotify returned no playlist for that link".to_string())?;

    let name = entity
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Spotify playlist")
        .to_string();
    let description = entity
        .get("subtitle")
        .and_then(Value::as_str)
        .or_else(|| entity.get("description").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();

    let mut tracks = Vec::new();
    let mut track_ids = Vec::new();
    if let Some(list) = entity.get("trackList").and_then(Value::as_array) {
        for item in list {
            if let Some(track) = parse_track(item) {
                track_ids.push(track_id_from_value(item));
                tracks.push(track);
            }
        }
    }

    if tracks.is_empty() {
        return Err("Spotify returned a playlist with no readable tracks".into());
    }

    let provider_total = tracks.len();
    Ok(ParsedPlaylist {
        playlist: SpotifyPlaylist {
            id: fallback_id.to_string(),
            name,
            description,
            tracks,
            provider_total,
            unreadable_tracks: 0,
            truncated: false,
            partial_embed: false,
        },
        track_ids,
        access_token: extract_access_token(&value),
    })
}

fn track_id_from_value(item: &Value) -> Option<String> {
    item.get("uri")
        .and_then(Value::as_str)
        .and_then(|uri| uri.strip_prefix("spotify:track:"))
        .map(str::to_string)
}

/// The embed page carries a short-lived anonymous token used by its own
/// player. Spotify has shipped three shapes for this path over time.
fn extract_access_token(value: &Value) -> Option<String> {
    const PATHS: [&[&str]; 3] = [
        &["props", "pageProps", "state", "settings", "session"],
        &["props", "pageProps", "settings", "session"],
        &["props", "pageProps", "session"],
    ];

    for path in PATHS {
        let mut current = value;
        let mut found = true;
        for key in path {
            match current.get(key) {
                Some(next) => current = next,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if !found {
            continue;
        }
        if let Some(token) = current.get("accessToken").and_then(Value::as_str) {
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    None
}

fn find_track_entity(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("track") && map.contains_key("name")
            {
                return Some(value);
            }
            for child in map.values() {
                if let Some(found) = find_track_entity(child) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_track_entity),
        _ => None,
    }
}

/// Parses the single-track entity shape used by `/embed/track/{id}`.
fn parse_track_entity(entity: &Value) -> Option<SpotifyTrack> {
    let title = entity
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let artist = entity
        .get("artists")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|entry| entry.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|value| !value.is_empty())
        .or_else(|| {
            entity
                .get("subtitle")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();

    let duration_ms = entity.get("duration").and_then(Value::as_i64).unwrap_or(0);

    Some(SpotifyTrack {
        title: title.to_string(),
        artist,
        album: String::new(),
        duration_ms,
    })
}

fn parse_track(item: &Value) -> Option<SpotifyTrack> {
    let title = item
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let artist = item
        .get("subtitle")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .or_else(|| {
            item.get("subtitleItems")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|entry| entry.get("title").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
        })
        .unwrap_or_default();

    let duration_ms = item.get("duration").and_then(Value::as_i64).unwrap_or(0);

    Some(SpotifyTrack {
        title: title.to_string(),
        artist,
        album: String::new(),
        duration_ms,
    })
}

fn extract_next_data(html: &str) -> Option<String> {
    extract_script_json(html, "__NEXT_DATA__")
}

fn extract_script_json(html: &str, marker: &str) -> Option<String> {
    let marker_index = html.find(marker)?;
    let open = html[marker_index..].find('>')? + marker_index + 1;
    let close = html[open..].find("</script>")? + open;
    let candidate = html[open..close].trim();
    if candidate.starts_with('{') || candidate.starts_with('[') {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn find_entity(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            if map.contains_key("trackList") && map.contains_key("name") {
                return Some(value);
            }
            for child in map.values() {
                if let Some(found) = find_entity(child) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_entity),
        _ => None,
    }
}

pub fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn artist_tokens(artist: &str) -> Vec<String> {
    artist
        .to_lowercase()
        .split([',', '&', ';', '/', ' '])
        .map(str::trim)
        .filter(|token| token.chars().count() > 1)
        .map(|token| token.to_string())
        .collect()
}

pub fn best_match<'a>(
    candidates: &'a [crate::services::ytmusic::YtMusicTrack],
    title: &str,
    artist: &str,
) -> Option<&'a crate::services::ytmusic::YtMusicTrack> {
    let wanted_title = normalize(title);
    let tokens = artist_tokens(artist);

    let artist_matches = |candidate: &crate::services::ytmusic::YtMusicTrack| {
        if tokens.is_empty() {
            return true;
        }
        let candidate_artist = candidate.artist.to_lowercase();
        tokens.iter().any(|token| candidate_artist.contains(token))
    };

    if let Some(exact) = candidates
        .iter()
        .find(|candidate| normalize(&candidate.title) == wanted_title && artist_matches(candidate))
    {
        return Some(exact);
    }

    candidates.iter().find(|candidate| {
        let candidate_title = normalize(&candidate.title);
        (candidate_title.starts_with(&wanted_title) || wanted_title.starts_with(&candidate_title))
            && artist_matches(candidate)
    })
}

// =========================================================================
// ImportManager: Multi-job background queue with Pause / Resume / Cancel
// =========================================================================

#[derive(Clone)]
pub struct ImportManager {
    jobs: Arc<RwLock<HashMap<String, ImportJob>>>,
    controls: Arc<Mutex<HashMap<String, JobControl>>>,
    pool: SqlitePool,
    ytmusic: Arc<crate::services::ytmusic::YtMusicService>,
    client: reqwest::Client,
}

impl ImportManager {
    pub fn new(pool: SqlitePool, ytmusic: Arc<crate::services::ytmusic::YtMusicService>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0",
            )
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            controls: Arc::new(Mutex::new(HashMap::new())),
            pool,
            ytmusic,
            client,
        }
    }

    async fn cleanup_job_control(&self, job_id: &str) {
        let mut controls_guard = self.controls.lock().await;
        controls_guard.remove(job_id);
    }

    /// Auto-detects Spotify vs YouTube Music playlist URLs and starts the import.
    pub async fn start_playlist_import(
        &self,
        app: AppHandle,
        url: &str,
    ) -> Result<ImportJob, String> {
        self.start_playlist_import_internal(Some(app), url).await
    }

    pub async fn start_playlist_import_internal(
        &self,
        app: Option<AppHandle>,
        url: &str,
    ) -> Result<ImportJob, String> {
        if playlist_id_from_url(url).is_some() {
            self.start_import_internal(app, url).await
        } else if crate::services::ytmusic::playlist_id_from_url(url).is_some() {
            self.start_ytmusic_import_internal(app, url).await
        } else {
            Err("Paste a public Spotify or YouTube Music playlist link.".into())
        }
    }

    /// Starts a YouTube Music playlist import. All readable tracks carry exact video ids,
    /// so they are immediately playable while a background worker matches local library copies
    /// and queues cloud sync entries.
    pub async fn start_ytmusic_import(
        &self,
        app: AppHandle,
        url: &str,
    ) -> Result<ImportJob, String> {
        self.start_ytmusic_import_internal(Some(app), url).await
    }

    pub async fn start_ytmusic_import_internal(
        &self,
        app: Option<AppHandle>,
        url: &str,
    ) -> Result<ImportJob, String> {
        let remote = self.ytmusic.fetch_playlist(url).await?;
        if remote.tracks.is_empty() {
            return Err("YouTube Music returned a playlist with no readable tracks".into());
        }

        let notice = if remote.truncated {
            Some(format!(
                "YouTube Music lists more than {} tracks. Cavia imported the first {}.",
                MAX_IMPORT_TRACKS,
                remote.tracks.len()
            ))
        } else if remote.unreadable_tracks > 0 {
            Some(format!(
                "{} YouTube Music tracks could not be read and were left out of this import.",
                remote.unreadable_tracks
            ))
        } else {
            None
        };

        let desc = if remote.description.is_empty() {
            "Imported from YouTube Music".to_string()
        } else {
            format!("{} (Imported from YouTube Music)", remote.description)
        };

        let playlist =
            crate::services::library::create_playlist(&self.pool, &remote.title, &desc).await?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Failed to begin transaction: {e}"))?;
        for (pos, track) in remote.tracks.iter().enumerate() {
            let entry_id = format!("plt-{}", Uuid::new_v4());
            let cloud_track_id = format!("youtube_music:{}", track.video_id);
            sqlx::query(
                r#"
                INSERT INTO playlist_tracks (
                    id, playlist_id, track_id, cloud_track_id, title, artist, album, duration_ms,
                    position, provider_id, provider_track_id, artwork_uri
                ) VALUES (?, ?, NULL, ?, ?, ?, ?, ?, ?, 'youtube_music', ?, ?)
                "#,
            )
            .bind(&entry_id)
            .bind(&playlist.id)
            .bind(&cloud_track_id)
            .bind(&track.title)
            .bind(&track.artist)
            .bind(&track.album)
            .bind(track.duration_ms)
            .bind(pos as i64)
            .bind(&track.video_id)
            .bind(track.artwork_url.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to insert track: {e}"))?;
        }
        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit tracks: {e}"))?;

        if let Some(ref a) = app {
            let _ = a.emit("playlist://created", &playlist.id);
        }

        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let job_id = format!("imp-{}", Uuid::new_v4());
        let job = ImportJob {
            id: job_id.clone(),
            source: "youtube_music".to_string(),
            playlist_id: playlist.id.clone(),
            playlist_name: remote.title.clone(),
            total: remote.tracks.len(),
            processed: 0,
            matched_local: 0,
            matched_online: 0,
            skipped: 0,
            current_track: None,
            status: ImportStatus::Running,
            error: None,
            notice,
            created_at_ms,
        };

        let paused = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());

        {
            let mut jobs_guard = self.jobs.write().await;
            jobs_guard.insert(job_id.clone(), job.clone());
        }
        {
            let mut controls_guard = self.controls.lock().await;
            controls_guard.insert(
                job_id.clone(),
                JobControl {
                    paused: paused.clone(),
                    cancelled: cancelled.clone(),
                    notify: notify.clone(),
                },
            );
        }

        if let Some(ref a) = app {
            let _ = a.emit("import://progress", &job);
        }

        let manager_clone = self.clone();
        let job_id_clone = job_id.clone();
        let playlist_id_clone = playlist.id.clone();

        tokio::spawn(async move {
            manager_clone
                .run_matching_worker(
                    app,
                    job_id_clone,
                    playlist_id_clone,
                    true,
                    paused,
                    cancelled,
                    notify,
                )
                .await;
        });

        Ok(job)
    }

    /// Fetches playlist metadata, inserts every readable track into the
    /// database in one transaction, then spawns the background matching worker.
    /// Most playlists resolve in about a second; playlists larger than the
    /// embed payload take longer while the remaining public track pages are read.
    pub async fn start_import(&self, app: AppHandle, url: &str) -> Result<ImportJob, String> {
        self.start_import_internal(Some(app), url).await
    }

    pub async fn start_import_internal(
        &self,
        app: Option<AppHandle>,
        url: &str,
    ) -> Result<ImportJob, String> {
        let service = SpotifyImportService::with_client(self.client.clone());
        let remote = service.fetch_playlist(url).await?;

        if remote.tracks.is_empty() {
            return Err("Spotify returned a playlist with no readable tracks".into());
        }

        let notice = if remote.truncated {
            Some(format!(
                "Spotify lists {} tracks. Cavia imported the first {} because one import is capped at {}.",
                remote.provider_total,
                remote.tracks.len(),
                MAX_IMPORT_TRACKS
            ))
        } else if remote.partial_embed {
            Some(
                "Spotify's public page exposed only its first 100 tracks, so this import may be \
                 incomplete. Try again later for the full playlist."
                    .to_string(),
            )
        } else if remote.unreadable_tracks > 0 {
            Some(format!(
                "{} Spotify tracks could not be read and were left out of this import.",
                remote.unreadable_tracks
            ))
        } else {
            None
        };

        let desc = if remote.description.is_empty() {
            "Imported from Spotify".to_string()
        } else {
            format!("{} (Imported from Spotify)", remote.description)
        };

        let playlist =
            crate::services::library::create_playlist(&self.pool, &remote.name, &desc).await?;

        // Fast bulk insert in one transaction
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Failed to begin transaction: {e}"))?;
        for (pos, track) in remote.tracks.iter().enumerate() {
            let entry_id = format!("plt-{}", Uuid::new_v4());
            sqlx::query(
                r#"
                INSERT INTO playlist_tracks (
                    id, playlist_id, track_id, cloud_track_id, title, artist, album, duration_ms,
                    position, provider_id, provider_track_id, artwork_uri
                ) VALUES (?, ?, NULL, NULL, ?, ?, ?, ?, ?, 'pending', NULL, NULL)
                "#,
            )
            .bind(&entry_id)
            .bind(&playlist.id)
            .bind(&track.title)
            .bind(&track.artist)
            .bind(&track.album)
            .bind(track.duration_ms)
            .bind(pos as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to insert track: {e}"))?;
        }
        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit tracks: {e}"))?;

        if let Some(ref a) = app {
            let _ = a.emit("playlist://created", &playlist.id);
        }

        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let job_id = format!("imp-{}", Uuid::new_v4());
        let job = ImportJob {
            id: job_id.clone(),
            source: "spotify".to_string(),
            playlist_id: playlist.id.clone(),
            playlist_name: remote.name.clone(),
            total: remote.tracks.len(),
            processed: 0,
            matched_local: 0,
            matched_online: 0,
            skipped: 0,
            current_track: None,
            status: ImportStatus::Running,
            error: None,
            notice,
            created_at_ms,
        };

        let paused = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());

        {
            let mut jobs_guard = self.jobs.write().await;
            jobs_guard.insert(job_id.clone(), job.clone());
        }
        {
            let mut controls_guard = self.controls.lock().await;
            controls_guard.insert(
                job_id.clone(),
                JobControl {
                    paused: paused.clone(),
                    cancelled: cancelled.clone(),
                    notify: notify.clone(),
                },
            );
        }

        if let Some(ref a) = app {
            let _ = a.emit("import://progress", &job);
        }

        // Spawn background worker
        let manager_clone = self.clone();
        let job_id_clone = job_id.clone();
        let playlist_id_clone = playlist.id.clone();

        tokio::spawn(async move {
            manager_clone
                .run_matching_worker(
                    app,
                    job_id_clone,
                    playlist_id_clone,
                    false,
                    paused,
                    cancelled,
                    notify,
                )
                .await;
        });

        Ok(job)
    }

    pub async fn pause_job(&self, app: Option<&AppHandle>, job_id: &str) -> Result<(), String> {
        let controls_guard = self.controls.lock().await;
        if let Some(ctrl) = controls_guard.get(job_id) {
            ctrl.paused.store(true, Ordering::SeqCst);
            let mut jobs_guard = self.jobs.write().await;
            if let Some(job) = jobs_guard.get_mut(job_id) {
                if job.status == ImportStatus::Running {
                    job.status = ImportStatus::Paused;
                    if let Some(a) = app {
                        let _ = a.emit("import://progress", &*job);
                    }
                }
            }
            Ok(())
        } else {
            Err("Job not found or already completed".to_string())
        }
    }

    pub async fn resume_job(&self, app: Option<&AppHandle>, job_id: &str) -> Result<(), String> {
        let controls_guard = self.controls.lock().await;
        if let Some(ctrl) = controls_guard.get(job_id) {
            ctrl.paused.store(false, Ordering::SeqCst);
            ctrl.notify.notify_one();
            let mut jobs_guard = self.jobs.write().await;
            if let Some(job) = jobs_guard.get_mut(job_id) {
                if job.status == ImportStatus::Paused {
                    job.status = ImportStatus::Running;
                    if let Some(a) = app {
                        let _ = a.emit("import://progress", &*job);
                    }
                }
            }
            Ok(())
        } else {
            Err("Job not found or already completed".to_string())
        }
    }

    pub async fn cancel_job(&self, app: Option<&AppHandle>, job_id: &str) -> Result<(), String> {
        let controls_guard = self.controls.lock().await;
        if let Some(ctrl) = controls_guard.get(job_id) {
            ctrl.cancelled.store(true, Ordering::SeqCst);
            ctrl.notify.notify_one();
            let mut jobs_guard = self.jobs.write().await;
            if let Some(job) = jobs_guard.get_mut(job_id) {
                job.status = ImportStatus::Cancelled;
                job.current_track = None;
                if let Some(a) = app {
                    let _ = a.emit("import://progress", &*job);
                }
            }
            Ok(())
        } else {
            Err("Job not found or already completed".to_string())
        }
    }

    pub async fn get_jobs(&self) -> Vec<ImportJob> {
        let jobs_guard = self.jobs.read().await;
        let mut list: Vec<ImportJob> = jobs_guard.values().cloned().collect();
        list.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
        list
    }

    pub async fn clear_completed(&self) {
        let mut jobs_guard = self.jobs.write().await;
        jobs_guard.retain(|_, job| {
            job.status == ImportStatus::Running || job.status == ImportStatus::Paused
        });
        let mut controls_guard = self.controls.lock().await;
        controls_guard.retain(|id, _| jobs_guard.contains_key(id));
    }

    pub async fn remove_job(&self, job_id: &str) -> Result<(), String> {
        let mut jobs_guard = self.jobs.write().await;
        if let Some(job) = jobs_guard.get(job_id) {
            if job.status == ImportStatus::Running {
                return Err("Cannot remove a running import job. Cancel it first.".to_string());
            }
        } else {
            return Err("Import job not found".to_string());
        }
        jobs_guard.remove(job_id);
        let mut controls_guard = self.controls.lock().await;
        controls_guard.remove(job_id);
        Ok(())
    }

    pub async fn cancel_and_delete(
        &self,
        app: Option<&AppHandle>,
        job_id: &str,
    ) -> Result<String, String> {
        // Cancel the job if running or paused
        let _ = self.cancel_job(app, job_id).await;

        let playlist_id = {
            let jobs_guard = self.jobs.read().await;
            jobs_guard.get(job_id).map(|j| j.playlist_id.clone())
        };

        if let Some(pid) = playlist_id {
            crate::services::library::delete_playlist(&self.pool, &pid).await?;
            let mut jobs_guard = self.jobs.write().await;
            jobs_guard.remove(job_id);
            let mut controls_guard = self.controls.lock().await;
            controls_guard.remove(job_id);
            Ok(pid)
        } else {
            Err("Job not found".to_string())
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_matching_worker(
        &self,
        app: Option<AppHandle>,
        job_id: String,
        playlist_id: String,
        pre_resolved_ytmusic: bool,
        paused: Arc<AtomicBool>,
        cancelled: Arc<AtomicBool>,
        notify: Arc<Notify>,
    ) {
        #[derive(sqlx::FromRow)]
        struct PendingTrack {
            id: String,
            title: String,
            artist: String,
            album: String,
            duration_ms: i64,
            position: i64,
            provider_track_id: Option<String>,
        }

        let pending = if pre_resolved_ytmusic {
            sqlx::query_as::<_, PendingTrack>(
                r#"
                SELECT id, title, artist, album, duration_ms, position, provider_track_id
                FROM playlist_tracks
                WHERE playlist_id = ?
                  AND track_id IS NULL
                  AND provider_id = 'youtube_music'
                ORDER BY position ASC
                "#,
            )
            .bind(&playlist_id)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query_as::<_, PendingTrack>(
                r#"
                SELECT id, title, artist, album, duration_ms, position, provider_track_id
                FROM playlist_tracks
                WHERE playlist_id = ?
                  AND track_id IS NULL
                  AND provider_track_id IS NULL
                  AND (provider_id = 'pending' OR provider_id IS NULL)
                ORDER BY position ASC
                "#,
            )
            .bind(&playlist_id)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
        };

        for track in pending {
            if cancelled.load(Ordering::SeqCst) {
                let mut jobs_guard = self.jobs.write().await;
                if let Some(job) = jobs_guard.get_mut(&job_id) {
                    job.status = ImportStatus::Cancelled;
                    job.current_track = None;
                    if let Some(ref a) = app {
                        let _ = a.emit("import://progress", &*job);
                    }
                }
                self.cleanup_job_control(&job_id).await;
                return;
            }

            while paused.load(Ordering::SeqCst) {
                if cancelled.load(Ordering::SeqCst) {
                    let mut jobs_guard = self.jobs.write().await;
                    if let Some(job) = jobs_guard.get_mut(&job_id) {
                        job.status = ImportStatus::Cancelled;
                        job.current_track = None;
                        if let Some(ref a) = app {
                            let _ = a.emit("import://progress", &*job);
                        }
                    }
                    self.cleanup_job_control(&job_id).await;
                    return;
                }
                notify.notified().await;
            }

            if cancelled.load(Ordering::SeqCst) {
                self.cleanup_job_control(&job_id).await;
                return;
            }

            // Update live progress with the currently matching track
            {
                let mut jobs_guard = self.jobs.write().await;
                if let Some(job) = jobs_guard.get_mut(&job_id) {
                    job.current_track = Some(format!("{} — {}", track.title, track.artist));
                    if let Some(ref a) = app {
                        let _ = a.emit("import://progress", &*job);
                    }
                }
            }

            // 1. Try local match first
            let local_id = crate::services::library::find_local_track_id(
                &self.pool,
                &track.title,
                &track.artist,
            )
            .await;

            if let Some(local_id) = local_id {
                if let Ok(mut tx) = self.pool.begin().await {
                    let local_meta: Option<(String, String, String, i64, Option<String>)> =
                        sqlx::query_as(
                            "SELECT title, artist, album, duration_ms, content_hash FROM tracks WHERE id = ? AND is_deleted = 0",
                        )
                        .bind(&local_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .ok()
                        .flatten();

                    if let Some((l_title, l_artist, l_album, l_duration, l_hash)) = local_meta {
                        let _ = sqlx::query(
                            r#"
                            UPDATE playlist_tracks
                            SET track_id = ?,
                                cloud_track_id = ?,
                                content_hash = ?,
                                provider_id = NULL,
                                provider_track_id = NULL
                            WHERE id = ?
                            "#,
                        )
                        .bind(&local_id)
                        .bind(&local_id)
                        .bind(&l_hash)
                        .bind(&track.id)
                        .execute(&mut *tx)
                        .await;

                        let _ = crate::services::sync::outbox::OutboxService::record_track_add(
                            &mut *tx,
                            &playlist_id,
                            &track.id,
                            &local_id,
                            &l_title,
                            &l_artist,
                            &l_album,
                            l_duration,
                            l_hash.as_deref(),
                            track.position,
                        )
                        .await;
                        let _ = tx.commit().await;
                    }
                }

                let mut jobs_guard = self.jobs.write().await;
                if let Some(job) = jobs_guard.get_mut(&job_id) {
                    job.matched_local += 1;
                    job.processed += 1;
                    if let Some(ref a) = app {
                        let _ = a.emit("import://progress", &*job);
                    }
                }
                continue;
            }

            // 2. For YouTube Music imports, the track already carries its exact video_id.
            // Just record sync outbox and update job counters without re-searching.
            if pre_resolved_ytmusic {
                if let Some(ref vid) = track.provider_track_id {
                    if let Ok(mut tx) = self.pool.begin().await {
                        let cloud_track_id = format!("youtube_music:{vid}");
                        let _ = crate::services::sync::outbox::OutboxService::record_track_add(
                            &mut *tx,
                            &playlist_id,
                            &track.id,
                            &cloud_track_id,
                            &track.title,
                            &track.artist,
                            &track.album,
                            track.duration_ms,
                            None,
                            track.position,
                        )
                        .await;
                        let _ = tx.commit().await;
                    }
                }

                let mut jobs_guard = self.jobs.write().await;
                if let Some(job) = jobs_guard.get_mut(&job_id) {
                    job.matched_online += 1;
                    job.processed += 1;
                    if let Some(ref a) = app {
                        let _ = a.emit("import://progress", &*job);
                    }
                }
                continue;
            }

            // 3. Try YouTube Music search for Spotify tracks
            let query = if track.artist.is_empty() {
                track.title.clone()
            } else {
                format!("{} {}", track.title, track.artist)
            };

            let candidates = self.ytmusic.search(&query).await.unwrap_or_default();
            if let Some(found) = best_match(&candidates, &track.title, &track.artist) {
                if let Ok(mut tx) = self.pool.begin().await {
                    let cloud_track_id = format!("youtube_music:{}", found.video_id);
                    let resolved_album = if track.album.is_empty() {
                        &found.album
                    } else {
                        &track.album
                    };
                    let _ = sqlx::query(
                        r#"
                        UPDATE playlist_tracks
                        SET provider_id = 'youtube_music',
                            provider_track_id = ?,
                            cloud_track_id = ?,
                            artwork_uri = COALESCE(artwork_uri, ?),
                            album = CASE WHEN album = '' OR album IS NULL THEN ? ELSE album END
                        WHERE id = ?
                        "#,
                    )
                    .bind(&found.video_id)
                    .bind(&cloud_track_id)
                    .bind(found.artwork_url.as_deref())
                    .bind(&found.album)
                    .bind(&track.id)
                    .execute(&mut *tx)
                    .await;

                    let _ = crate::services::sync::outbox::OutboxService::record_track_add(
                        &mut *tx,
                        &playlist_id,
                        &track.id,
                        &cloud_track_id,
                        &track.title,
                        &track.artist,
                        resolved_album,
                        track.duration_ms,
                        None,
                        track.position,
                    )
                    .await;
                    let _ = tx.commit().await;
                }

                let mut jobs_guard = self.jobs.write().await;
                if let Some(job) = jobs_guard.get_mut(&job_id) {
                    job.matched_online += 1;
                    job.processed += 1;
                    if let Some(ref a) = app {
                        let _ = a.emit("import://progress", &*job);
                    }
                }
            } else {
                let _ = sqlx::query(
                    "UPDATE playlist_tracks SET provider_id = 'unmatched' WHERE id = ?",
                )
                .bind(&track.id)
                .execute(&self.pool)
                .await;

                let mut jobs_guard = self.jobs.write().await;
                if let Some(job) = jobs_guard.get_mut(&job_id) {
                    job.skipped += 1;
                    job.processed += 1;
                    if let Some(ref a) = app {
                        let _ = a.emit("import://progress", &*job);
                    }
                }
            }

            // Courteous throttle between network queries to avoid IP rate limits
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        // Job completed
        {
            let mut jobs_guard = self.jobs.write().await;
            if let Some(job) = jobs_guard.get_mut(&job_id) {
                if job.status == ImportStatus::Running {
                    job.status = ImportStatus::Completed;
                    job.current_track = None;
                    if let Some(ref a) = app {
                        let _ = a.emit("import://progress", &*job);
                        let _ = a.emit("import://completed", &*job);
                    }
                }
            }
        }
        self.cleanup_job_control(&job_id).await;
        if let Some(ref a) = app {
            let _ = a.emit("playlist://updated", &playlist_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_playlist_links() {
        assert_eq!(
            playlist_id_from_url("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc"),
            Some("37i9dQZF1DXcBWIGoYBM5M".to_string())
        );
        assert_eq!(
            playlist_id_from_url("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M"),
            Some("37i9dQZF1DXcBWIGoYBM5M".to_string())
        );
        assert_eq!(
            playlist_id_from_url("https://open.spotify.com/track/abc"),
            None
        );
        assert_eq!(playlist_id_from_url(""), None);
        assert_eq!(playlist_id_from_url("not a link"), None);
    }

    #[tokio::test]
    #[ignore = "hits the user test playlist"]
    async fn user_test_playlist_smoke() {
        let service = SpotifyImportService::new().expect("service");
        let playlist = service
            .fetch_playlist("https://open.spotify.com/playlist/1pFaOjma3sJKAh6MbhepKi?si=oE3div2cS5CG6W_qCrmhjQ")
            .await
            .expect("fetch");
        println!(
            "Playlist: '{}' — {} tracks",
            playlist.name,
            playlist.tracks.len()
        );
        for (i, t) in playlist.tracks.iter().enumerate() {
            println!(
                "  [{}] {} — {} ({} ms)",
                i + 1,
                t.title,
                t.artist,
                t.duration_ms
            );
        }
        assert_eq!(playlist.tracks.len(), 8);
    }

    #[tokio::test]
    #[ignore = "hits the user test playlist and YouTube Music"]
    async fn test_import_manager_with_user_playlist() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate");

        let cache = std::env::temp_dir().join(format!("cavia-test-import-{}", Uuid::new_v4()));
        let ytmusic = Arc::new(
            crate::services::ytmusic::YtMusicService::new(cache.clone()).expect("ytmusic"),
        );
        let manager = ImportManager::new(pool.clone(), ytmusic.clone());

        let url =
            "https://open.spotify.com/playlist/1pFaOjma3sJKAh6MbhepKi?si=oE3div2cS5CG6W_qCrmhjQ";
        let job = manager
            .start_import_internal(None, url)
            .await
            .expect("start import");

        assert_eq!(job.total, 8);
        assert_eq!(job.status, ImportStatus::Running);

        // Wait for worker to finish matching
        let mut attempts = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            attempts += 1;
            let jobs = manager.get_jobs().await;
            let current = jobs.iter().find(|j| j.id == job.id).unwrap();
            println!(
                "Progress: {}/{} — {} online, {} skipped, status: {:?}",
                current.processed,
                current.total,
                current.matched_online,
                current.skipped,
                current.status
            );
            if current.status == ImportStatus::Completed
                || current.status == ImportStatus::Failed
                || attempts > 60
            {
                break;
            }
        }

        let tracks = crate::services::library::get_playlist_tracks(&pool, &job.playlist_id)
            .await
            .expect("get playlist tracks");
        assert_eq!(tracks.len(), 8);
        for t in &tracks {
            println!(
                "Track: {} — {} (id: {}, source: {})",
                t.title, t.artist, t.id, t.source_kind
            );
        }
        let matched = tracks
            .iter()
            .filter(|t| t.source_kind == "provider")
            .count();
        println!("Successfully matched {} out of 8 online!", matched);
        assert!(matched >= 6, "At least 6 out of 8 should match online");

        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn reads_embed_payload() {
        let html = r#"<html><body><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"name":"Night drive","subtitle":"Made by Cavia","trackList":[{"title":"Instant Crush","subtitle":"Daft Punk","duration":337000},{"title":"Get Lucky","subtitle":"Daft Punk, Pharrell Williams","duration":369000}]}}}}}}</script></body></html>"#;

        let parsed = parse_playlist(html, "37i9dQZF1DXcBWIGoYBM5M").expect("payload should parse");
        let playlist = parsed.playlist;
        assert_eq!(playlist.name, "Night drive");
        assert_eq!(playlist.tracks.len(), 2);
        assert_eq!(playlist.tracks[0].title, "Instant Crush");
        assert_eq!(playlist.tracks[0].artist, "Daft Punk");
        assert_eq!(playlist.tracks[0].duration_ms, 337_000);
        assert_eq!(playlist.provider_total, 2);
        assert_eq!(playlist.unreadable_tracks, 0);
        assert!(!playlist.truncated);
        assert!(!playlist.partial_embed);
    }

    #[test]
    fn reads_anonymous_token_and_track_ids() {
        let html = r#"<html><body><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"settings":{"session":{"accessToken":"anon-token"}},"data":{"entity":{"name":"Long list","trackList":[{"uri":"spotify:track:abc123","title":"One","subtitle":"Artist","duration":1000}]}}}}}}</script></body></html>"#;

        let parsed = parse_playlist(html, "playlist-id").expect("payload should parse");
        assert_eq!(parsed.access_token.as_deref(), Some("anon-token"));
        assert_eq!(parsed.track_ids, vec![Some("abc123".to_string())]);
    }

    #[test]
    fn reads_single_track_embed_entity() {
        let html = r#"<html><body><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"type":"track","name":"Never Tear Us Apart","duration":184586,"artists":[{"name":"INXS"}]}}}}}}</script></body></html>"#;

        let json_text = extract_next_data(html).expect("embedded json");
        let value: Value = serde_json::from_str(&json_text).expect("valid json");
        let entity = find_track_entity(&value).expect("track entity");
        let track = parse_track_entity(entity).expect("track should parse");

        assert_eq!(track.title, "Never Tear Us Apart");
        assert_eq!(track.artist, "INXS");
        assert_eq!(track.duration_ms, 184_586);
    }
}
