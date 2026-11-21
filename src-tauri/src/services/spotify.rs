//! Lightweight Spotify playlist reading and background matching manager.
//!
//! Only public playlist metadata is read from the Spotify embed page.
//! Track records are immediately inserted into SQLite (fast import),
//! and matching against the local library and YouTube Music runs in a
//! manageable background worker with pause/cancel controls.

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

        parse_playlist(&html, &id)
    }
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

fn parse_playlist(html: &str, fallback_id: &str) -> Result<SpotifyPlaylist, String> {
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
    if let Some(list) = entity.get("trackList").and_then(Value::as_array) {
        for item in list {
            if let Some(track) = parse_track(item) {
                tracks.push(track);
            }
        }
    }

    if tracks.is_empty() {
        return Err("Spotify returned a playlist with no readable tracks".into());
    }

    Ok(SpotifyPlaylist {
        id: fallback_id.to_string(),
        name,
        description,
        tracks,
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
}

impl ImportManager {
    pub fn new(pool: SqlitePool, ytmusic: Arc<crate::services::ytmusic::YtMusicService>) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            controls: Arc::new(Mutex::new(HashMap::new())),
            pool,
            ytmusic,
        }
    }

    /// Fast import: fetches Spotify embed metadata and inserts all tracks into
    /// the database in a single transaction (< 1 second), then spawns the background worker.
    pub async fn start_import(&self, app: AppHandle, url: &str) -> Result<ImportJob, String> {
        self.start_import_internal(Some(app), url).await
    }

    pub async fn start_import_internal(
        &self,
        app: Option<AppHandle>,
        url: &str,
    ) -> Result<ImportJob, String> {
        let service = SpotifyImportService::new()?;
        let remote = service.fetch_playlist(url).await?;

        if remote.tracks.is_empty() {
            return Err("Spotify returned a playlist with no readable tracks".into());
        }

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

    async fn run_matching_worker(
        &self,
        app: Option<AppHandle>,
        job_id: String,
        playlist_id: String,
        paused: Arc<AtomicBool>,
        cancelled: Arc<AtomicBool>,
        notify: Arc<Notify>,
    ) {
        #[derive(sqlx::FromRow)]
        struct PendingTrack {
            id: String,
            title: String,
            artist: String,
        }

        let pending = sqlx::query_as::<_, PendingTrack>(
            r#"
            SELECT id, title, artist
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
        .unwrap_or_default();

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
                    return;
                }
                notify.notified().await;
            }

            if cancelled.load(Ordering::SeqCst) {
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
                let _ = sqlx::query(
                    "UPDATE playlist_tracks SET track_id = ?, provider_id = NULL WHERE id = ?",
                )
                .bind(&local_id)
                .bind(&track.id)
                .execute(&self.pool)
                .await;

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

            // 2. Try YouTube Music search (video_id ONLY — zero artwork download, zero lyrics overhead)
            let query = if track.artist.is_empty() {
                track.title.clone()
            } else {
                format!("{} {}", track.title, track.artist)
            };

            let candidates = self.ytmusic.search(&query).await.unwrap_or_default();
            if let Some(found) = best_match(&candidates, &track.title, &track.artist) {
                let _ = sqlx::query(
                    r#"
                    UPDATE playlist_tracks
                    SET provider_id = 'youtube_music',
                        provider_track_id = ?,
                        album = CASE WHEN album = '' OR album IS NULL THEN ? ELSE album END
                    WHERE id = ?
                    "#,
                )
                .bind(&found.video_id)
                .bind(&found.album)
                .bind(&track.id)
                .execute(&self.pool)
                .await;

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

        let playlist =
            parse_playlist(html, "37i9dQZF1DXcBWIGoYBM5M").expect("payload should parse");
        assert_eq!(playlist.name, "Night drive");
        assert_eq!(playlist.tracks.len(), 2);
        assert_eq!(playlist.tracks[0].title, "Instant Crush");
        assert_eq!(playlist.tracks[0].artist, "Daft Punk");
        assert_eq!(playlist.tracks[0].duration_ms, 337_000);
    }
}
