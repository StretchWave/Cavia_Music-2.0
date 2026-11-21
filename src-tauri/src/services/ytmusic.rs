//! YouTube Music catalogue search and audio resolution.
//!
//! This follows the same approach the Harmony Music project uses: YouTube
//! Music's own InnerTube endpoints. Search uses the `WEB_REMIX` client; the
//! `player` endpoint is queried with the `VISIONOS` client, which returns
//! un-gated adaptive audio formats that are not limited to the first
//! megabyte the way iOS URLs are.
//!
//! These are undocumented endpoints and are subject to YouTube's terms of
//! service, so the integration is deliberately narrow: it searches, resolves
//! one audio stream and caches the fetched bytes. It carries no cookies, no
//! account credentials and no DRM handling. When an endpoint changes the
//! service fails with a readable error rather than guessing.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const CLIENT_VERSION: &str = "1.20241127.01.00";
const SEARCH_URL: &str = "https://music.youtube.com/youtubei/v1/search?prettyPrint=false";
const NEXT_URL: &str = "https://music.youtube.com/youtubei/v1/next?prettyPrint=false";
const PLAYER_URL: &str = "https://music.youtube.com/youtubei/v1/player?prettyPrint=false";
const GUIDE_URL: &str = "https://music.youtube.com/youtubei/v1/guide?prettyPrint=false";
const ORIGIN: &str = "https://music.youtube.com";
/// Base64 `params` selecting the Songs tab of a YouTube Music search.
const SONGS_FILTER: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA==";

const WEB_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0";
const VISIONOS_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";

/// Upper bound for a single cached audio fetch. A 10-minute Opus track is
/// well under this; anything larger is treated as a provider error.
const MAX_AUDIO_BYTES: u64 = 80 * 1024 * 1024;

/// Upper bound for total disk space consumed by cached audio streams (500 MB).
/// When this threshold is exceeded, older cached streams are evicted LRU-style.
const MAX_TOTAL_CACHE_BYTES: u64 = 500 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicTrack {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedAudio {
    pub url: String,
    pub mime_type: String,
    pub bitrate: u64,
}

pub struct YtMusicService {
    client: reqwest::Client,
    cache_dir: PathBuf,
    visitor: Mutex<Option<String>>,
}

impl YtMusicService {
    pub fn new(cache_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Could not create stream cache directory: {e}"))?;

        let client = reqwest::Client::builder()
            .user_agent(WEB_UA)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(25))
            .build()
            .map_err(|e| format!("Could not create HTTP client for YouTube Music: {e}"))?;

        Ok(Self {
            client,
            cache_dir,
            visitor: Mutex::new(None),
        })
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Cached visitor data. The VISIONOS player request is answered with
    /// `LOGIN_REQUIRED` unless `X-Goog-Visitor-Id` is present.
    async fn visitor_id(&self) -> Option<String> {
        if let Some(existing) = self
            .visitor
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .filter(|value| !value.is_empty())
        {
            return Some(existing);
        }

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": CLIENT_VERSION,
                    "gl": "US",
                    "hl": "en"
                }
            }
        });

        let response = self
            .client
            .post(GUIDE_URL)
            .header("Content-Type", "application/json")
            .header("Origin", ORIGIN)
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", CLIENT_VERSION)
            .json(&body)
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let value: Value = response.json().await.ok()?;
        let visitor = value
            .pointer("/responseContext/visitorData")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())?;

        if let Ok(mut guard) = self.visitor.lock() {
            *guard = Some(visitor.clone());
        }
        Some(visitor)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<YtMusicTrack>, String> {
        // `EgWKAQIIAWoKEAkQBRAKEAMQBA==` is the "Songs" filter. Without it the
        // default response mixes shelves whose items omit the artist, because
        // the artist is implied by the shelf they came from.
        let songs = self.search_with_params(query, Some(SONGS_FILTER)).await?;
        if !songs.is_empty() {
            return Ok(songs);
        }
        self.search_with_params(query, None).await
    }

    async fn search_with_params(
        &self,
        query: &str,
        params: Option<&str>,
    ) -> Result<Vec<YtMusicTrack>, String> {
        let mut body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": CLIENT_VERSION,
                    "gl": "US",
                    "hl": "en"
                }
            },
            "query": query
        });
        if let Some(params) = params {
            body["params"] = Value::String(params.to_string());
        }

        let response = self
            .client
            .post(SEARCH_URL)
            .header("Content-Type", "application/json")
            .header("Origin", ORIGIN)
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", CLIENT_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("YouTube Music is unreachable: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "YouTube Music search failed with status {}",
                response.status().as_u16()
            ));
        }

        let value: Value = response
            .json()
            .await
            .map_err(|e| format!("YouTube Music returned an unreadable response: {e}"))?;

        let mut items = Vec::new();
        collect_search_items(&value, &mut items);

        let mut seen = HashSet::new();
        items.retain(|track| seen.insert(track.video_id.clone()));
        items.truncate(24);
        Ok(items)
    }

    /// Tracks YouTube Music offers as a continuation of `video_id` (the same
    /// list the web player shows under "Up next"). Used as a radio when the
    /// user plays one catalogue result without building a queue.
    pub async fn related(&self, video_id: &str) -> Result<Vec<YtMusicTrack>, String> {
        if !is_valid_video_id(video_id) {
            return Err("Invalid YouTube Music video id".into());
        }

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": CLIENT_VERSION,
                    "gl": "US",
                    "hl": "en"
                }
            },
            "videoId": video_id,
            "playlistId": format!("RDAMVM{video_id}"),
            "isAudioOnly": true,
            "enablePersistentPlaylistPanel": true
        });

        let response = self
            .client
            .post(NEXT_URL)
            .header("Content-Type", "application/json")
            .header("Origin", ORIGIN)
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", CLIENT_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("YouTube Music radio is unreachable: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "YouTube Music radio failed with status {}",
                response.status().as_u16()
            ));
        }

        let value: Value = response
            .json()
            .await
            .map_err(|e| format!("YouTube Music returned an unreadable radio response: {e}"))?;

        let mut items = Vec::new();
        collect_panel_items(&value, &mut items);

        let mut seen = HashSet::new();
        items.retain(|track| track.video_id != video_id && seen.insert(track.video_id.clone()));
        items.truncate(20);
        Ok(items)
    }

    pub async fn resolve_audio(&self, video_id: &str) -> Result<ResolvedAudio, String> {
        let visitor = self.visitor_id().await;

        let mut client_context = json!({
            "clientName": "VISIONOS",
            "clientVersion": "1.02",
            "deviceMake": "Apple",
            "deviceModel": "RealityDevice17,1",
            "osName": "visionOS",
            "osVersion": "26.5.23O471",
            "gl": "US",
            "hl": "en"
        });
        if let Some(visitor) = visitor.as_ref() {
            client_context["visitorData"] = Value::String(visitor.clone());
        }

        let body = json!({
            "context": { "client": client_context },
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true
        });

        let mut request = self
            .client
            .post(PLAYER_URL)
            .header("Content-Type", "application/json")
            .header("Origin", ORIGIN)
            .header("User-Agent", VISIONOS_UA)
            .header("X-YouTube-Client-Name", "101")
            .header("X-YouTube-Client-Version", "1.02");

        if let Some(visitor) = visitor.as_ref() {
            request = request.header("X-Goog-Visitor-Id", visitor);
        }

        let response = request
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("YouTube Music is unreachable: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "YouTube Music could not prepare this track (status {})",
                response.status().as_u16()
            ));
        }

        let value: Value = response
            .json()
            .await
            .map_err(|e| format!("YouTube Music returned an unreadable response: {e}"))?;

        if let Some(status) = value
            .pointer("/playabilityStatus/status")
            .and_then(Value::as_str)
            && status != "OK"
        {
            let reason = value
                .pointer("/playabilityStatus/reason")
                .and_then(Value::as_str)
                .unwrap_or("track is not playable");
            return Err(format!("YouTube Music refused playback: {reason}"));
        }

        let formats = value
            .pointer("/streamingData/adaptiveFormats")
            .and_then(Value::as_array)
            .ok_or_else(|| "YouTube Music returned no audio formats for this track".to_string())?;

        let mut best: Option<ResolvedAudio> = None;
        for format in formats {
            let mime = format.get("mimeType").and_then(Value::as_str).unwrap_or("");
            if !mime.starts_with("audio/") {
                continue;
            }

            let url = match format.get("url").and_then(Value::as_str) {
                Some(url) if !url.is_empty() => url,
                _ => continue,
            };

            // iOS URLs only serve the first megabyte; they cannot be played.
            if url.to_ascii_lowercase().contains("c=ios") {
                continue;
            }

            let bitrate = format.get("bitrate").and_then(Value::as_u64).unwrap_or(0);
            let is_mp4 = mime.contains("mp4");

            let candidate = ResolvedAudio {
                url: url.to_string(),
                mime_type: mime.split(';').next().unwrap_or(mime).trim().to_string(),
                bitrate,
            };

            best = match best {
                None => Some(candidate),
                Some(current) => {
                    let current_mp4 = current.mime_type.contains("mp4");
                    if (is_mp4 && !current_mp4)
                        || (is_mp4 == current_mp4 && bitrate > current.bitrate)
                    {
                        Some(candidate)
                    } else {
                        Some(current)
                    }
                }
            };
        }

        best.ok_or_else(|| "No playable audio stream was available for this track".to_string())
    }

    /// Downloads the audio stream into the cache directory and returns the
    /// local path. Playback then uses the same decoder path as local files,
    /// so seeking and duration are exact.
    pub async fn ensure_cached(&self, video_id: &str, url: &str) -> Result<PathBuf, String> {
        let path = self.cache_dir.join(format!("{video_id}.m4a"));
        if path.metadata().map(|meta| meta.len() > 0).unwrap_or(false) {
            return Ok(path);
        }

        // These media URLs advertise `rqh=1`: the server only serves them as a
        // range response and drops a plain GET part-way through the body.
        let mut response = self
            .client
            .get(url)
            .header("User-Agent", VISIONOS_UA)
            .header("Range", "bytes=0-")
            .send()
            .await
            .map_err(|e| format!("Could not fetch the audio stream: {e}"))?;

        let status = response.status();
        if !(status.is_success() || status == reqwest::StatusCode::PARTIAL_CONTENT) {
            return Err(format!(
                "Could not fetch the audio stream (status {})",
                status.as_u16()
            ));
        }

        let partial = path.with_extension("part");
        let mut file = std::fs::File::create(&partial)
            .map_err(|e| format!("Could not create the stream cache file: {e}"))?;

        let mut total: u64 = 0;
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(e) => {
                    drop(file);
                    let _ = std::fs::remove_file(&partial);
                    return Err(format!("Could not read the audio stream: {e}"));
                }
            };

            total += chunk.len() as u64;
            if total > MAX_AUDIO_BYTES {
                drop(file);
                let _ = std::fs::remove_file(&partial);
                return Err("The audio stream exceeded the cache size limit".to_string());
            }

            if let Err(e) = std::io::Write::write_all(&mut file, &chunk) {
                drop(file);
                let _ = std::fs::remove_file(&partial);
                return Err(format!("Could not write the stream cache file: {e}"));
            }
        }

        drop(file);

        if total == 0 {
            let _ = std::fs::remove_file(&partial);
            return Err("YouTube Music returned an empty audio stream".to_string());
        }

        std::fs::rename(&partial, &path)
            .map_err(|e| format!("Could not finalize the stream cache file: {e}"))?;

        let _ = self.prune_cache(MAX_TOTAL_CACHE_BYTES);

        Ok(path)
    }

    /// Evicts oldest cached streams when the total stream cache size exceeds `max_bytes`.
    pub fn prune_cache(&self, max_bytes: u64) -> Result<(), std::io::Error> {
        if !self.cache_dir.exists() {
            return Ok(());
        }
        let mut entries = Vec::new();
        let mut total_size: u64 = 0;
        for entry in std::fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.is_file() {
                if let Ok(meta) = file_path.metadata() {
                    let len = meta.len();
                    total_size += len;
                    let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    entries.push((file_path, len, mtime));
                }
            }
        }
        if total_size <= max_bytes {
            return Ok(());
        }
        // Sort oldest modified first
        entries.sort_by_key(|(_, _, mtime)| *mtime);
        for (file_path, len, _) in entries {
            if total_size <= max_bytes {
                break;
            }
            if std::fs::remove_file(&file_path).is_ok() {
                total_size = total_size.saturating_sub(len);
            }
        }
        Ok(())
    }
}

fn flex_text(item: &Value, index: usize) -> String {
    let pointer = format!("/flexColumns/{index}/musicResponsiveListItemFlexColumnRenderer/text");
    item.pointer(&pointer).map(text_runs).unwrap_or_default()
}

fn text_runs(node: &Value) -> String {
    if let Some(runs) = node.get("runs").and_then(Value::as_array) {
        return runs
            .iter()
            .filter_map(|run| run.get("text").and_then(Value::as_str))
            .collect::<String>();
    }
    node.get("simpleText")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn find_string_key(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(found)) = map.get(key)
                && !found.is_empty()
            {
                return Some(found.clone());
            }
            map.values().find_map(|child| find_string_key(child, key))
        }
        Value::Array(items) => items.iter().find_map(|child| find_string_key(child, key)),
        _ => None,
    }
}

fn parse_duration_ms(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    if trimmed.is_empty() || !trimmed.contains(':') {
        return None;
    }

    let mut seconds: i64 = 0;
    let mut parts = 0;
    for chunk in trimmed.split(':') {
        let value: i64 = chunk.trim().parse().ok()?;
        seconds = seconds * 60 + value;
        parts += 1;
        if parts > 3 {
            return None;
        }
    }

    if parts == 0 || seconds <= 0 || seconds > 6 * 60 * 60 {
        return None;
    }
    Some(seconds * 1000)
}

/// Google's image proxy encodes the served size in the URL
/// (`=w120-h120-l90-rj`). Search rows hand back the small variant even though
/// a 544px image exists, which is what makes catalogue art look soft once it
/// is scaled up in the player. Ask the CDN for the largest variant instead.
fn upgrade_thumbnail_url(url: &str) -> String {
    let Some((base, params)) = url.rsplit_once('=') else {
        return url.to_string();
    };
    if !(params.starts_with('w') || params.starts_with('s') || params.starts_with("p-")) {
        return url.to_string();
    }

    let kept: Vec<&str> = params
        .split('-')
        .filter(|part| {
            !part.starts_with('w')
                && !part.starts_with('h')
                && !part.starts_with('s')
                && !part.starts_with("p-")
        })
        .collect();

    if kept.is_empty() {
        format!("{base}=w544-h544")
    } else {
        format!("{base}=w544-h544-{}", kept.join("-"))
    }
}

fn is_valid_video_id(video_id: &str) -> bool {
    video_id.len() == 11
        && video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn last_thumbnail_url(value: &Value) -> Option<String> {
    let items = value.as_array()?;
    items
        .iter()
        .rev()
        .find_map(|item| item.get("url").and_then(Value::as_str))
        .filter(|url| !url.is_empty())
        .map(|url| url.to_string())
}

fn find_video_id(item: &Value) -> Option<String> {
    // The playable id is the one on the row itself, not a menu action.
    const POINTERS: [&str; 2] = [
        "/playlistItemData/videoId",
        "/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId",
    ];
    for pointer in POINTERS {
        if let Some(id) = item.pointer(pointer).and_then(Value::as_str)
            && id.len() == 11
        {
            return Some(id.to_string());
        }
    }

    let fallback = find_string_key(item, "videoId")?;
    (fallback.len() == 11).then_some(fallback)
}

fn parse_search_item(item: &Value) -> Option<YtMusicTrack> {
    let video_id = find_video_id(item)?;

    let mut title = flex_text(item, 0);
    if title.is_empty() {
        title = item
            .pointer("/title/runs")
            .map(text_runs)
            .unwrap_or_default();
    }
    if title.is_empty() {
        return None;
    }

    let subtitle = flex_text(item, 1);
    let mut artist = String::new();
    let mut album = String::new();
    let mut duration_ms = 0i64;

    for part in subtitle
        .split(['•', '·'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if let Some(parsed) = parse_duration_ms(part) {
            duration_ms = parsed;
            continue;
        }
        if matches!(
            part,
            "Song" | "Video" | "Album" | "Single" | "EP" | "Playlist"
        ) {
            continue;
        }
        if artist.is_empty() {
            artist = part.to_string();
        } else if album.is_empty() {
            album = part.to_string();
        }
    }

    if duration_ms == 0 {
        let fixed = item
            .pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text")
            .map(text_runs)
            .unwrap_or_default();
        duration_ms = parse_duration_ms(&fixed).unwrap_or(0);
    }

    let artwork_url = item
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
        .and_then(last_thumbnail_url)
        .or_else(|| {
            item.pointer("/thumbnail/thumbnails")
                .and_then(last_thumbnail_url)
        })
        .map(|url| upgrade_thumbnail_url(&url));

    Some(YtMusicTrack {
        video_id,
        title,
        artist,
        album,
        duration_ms,
        artwork_url,
    })
}

fn parse_panel_item(item: &Value) -> Option<YtMusicTrack> {
    let video_id = item.pointer("/videoId").and_then(Value::as_str)?;
    if !is_valid_video_id(video_id) {
        return None;
    }

    let mut title = item.pointer("/title").map(text_runs).unwrap_or_default();
    if title.is_empty() {
        title = item
            .pointer("/title/runs")
            .map(text_runs)
            .unwrap_or_default();
    }
    if title.is_empty() {
        return None;
    }

    let byline = item
        .pointer("/longBylineText")
        .or_else(|| item.pointer("/shortBylineText"))
        .map(text_runs)
        .unwrap_or_default();
    let mut artist = String::new();
    let mut album = String::new();
    for part in byline
        .split(['•', '·'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if artist.is_empty() {
            artist = part.to_string();
        } else if album.is_empty() {
            album = part.to_string();
        }
    }

    let duration_ms = item
        .pointer("/lengthText")
        .map(text_runs)
        .and_then(|text| parse_duration_ms(&text))
        .unwrap_or(0);

    let artwork_url = item
        .pointer("/thumbnail/thumbnails")
        .and_then(last_thumbnail_url)
        .map(|url| upgrade_thumbnail_url(&url));

    Some(YtMusicTrack {
        video_id: video_id.to_string(),
        title,
        artist,
        album,
        duration_ms,
        artwork_url,
    })
}

fn collect_panel_items(value: &Value, out: &mut Vec<YtMusicTrack>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "playlistPanelVideoRenderer" {
                    if let Some(track) = parse_panel_item(child) {
                        out.push(track);
                    }
                    continue;
                }
                collect_panel_items(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_panel_items(child, out);
            }
        }
        _ => {}
    }
}

fn collect_search_items(value: &Value, out: &mut Vec<YtMusicTrack>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "musicResponsiveListItemRenderer" {
                    if let Some(track) = parse_search_item(child) {
                        out.push(track);
                    }
                    continue;
                }
                collect_search_items(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_search_items(child, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration_ms("3:45"), Some(225_000));
        assert_eq!(parse_duration_ms("1:02:03"), Some(3_723_000));
        assert_eq!(parse_duration_ms("Song"), None);
        assert_eq!(parse_duration_ms(""), None);
    }

    #[test]
    fn reads_runs_and_simple_text() {
        let runs = json!({ "runs": [{ "text": "Hello" }, { "text": " world" }] });
        assert_eq!(text_runs(&runs), "Hello world");
        let simple = json!({ "simpleText": "Solo" });
        assert_eq!(text_runs(&simple), "Solo");
    }

    #[test]
    fn parses_a_music_list_item() {
        let item = json!({
            "playlistItemData": { "videoId": "dQw4w9WgXcQ" },
            "flexColumns": [
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Never Gonna Give You Up" }] } } },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Rick Astley • Whenever You Need Somebody • 3:33" }] } } }
            ],
            "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://example.test/small.jpg" },
                { "url": "https://example.test/large.jpg" }
            ] } } }
        });

        let parsed = parse_search_item(&item).expect("item should parse");
        assert_eq!(parsed.video_id, "dQw4w9WgXcQ");
        assert_eq!(parsed.title, "Never Gonna Give You Up");
        assert_eq!(parsed.artist, "Rick Astley");
        assert_eq!(parsed.album, "Whenever You Need Somebody");
        assert_eq!(parsed.duration_ms, 213_000);
        assert_eq!(
            parsed.artwork_url.as_deref(),
            Some("https://example.test/large.jpg")
        );
    }

    #[test]
    fn upgrades_thumbnail_size() {
        assert_eq!(
            upgrade_thumbnail_url("https://lh3.googleusercontent.com/abc=w120-h120-l90-rj"),
            "https://lh3.googleusercontent.com/abc=w544-h544-l90-rj"
        );
        assert_eq!(
            upgrade_thumbnail_url("https://i.ytimg.com/vi/x/hqdefault.jpg"),
            "https://i.ytimg.com/vi/x/hqdefault.jpg"
        );
    }

    #[test]
    fn parses_a_radio_panel_item() {
        let item = json!({
            "videoId": "dQw4w9WgXcQ",
            "title": { "runs": [{ "text": "Instant Crush" }] },
            "longBylineText": { "runs": [{ "text": "Daft Punk • Random Access Memories" }] },
            "lengthText": { "runs": [{ "text": "5:37" }] },
            "thumbnail": { "thumbnails": [
                { "url": "https://lh3.googleusercontent.com/x=w60-h60-l90-rj" },
                { "url": "https://lh3.googleusercontent.com/x=w120-h120-l90-rj" }
            ] }
        });
        let parsed = parse_panel_item(&item).expect("panel item should parse");
        assert_eq!(parsed.video_id, "dQw4w9WgXcQ");
        assert_eq!(parsed.title, "Instant Crush");
        assert_eq!(parsed.artist, "Daft Punk");
        assert_eq!(parsed.album, "Random Access Memories");
        assert_eq!(parsed.duration_ms, 337_000);
        assert_eq!(
            parsed.artwork_url.as_deref(),
            Some("https://lh3.googleusercontent.com/x=w544-h544-l90-rj")
        );
    }

    #[tokio::test]
    #[ignore = "hits the live YouTube Music API"]
    async fn live_radio_smoke() {
        let dir = std::env::temp_dir().join("cavia-ytmusic-radio");
        let service = YtMusicService::new(dir.clone()).expect("service");

        let results = service
            .search("daft punk instant crush")
            .await
            .expect("search");
        let first = results.first().expect("search should return a track");
        let related = service.related(&first.video_id).await.expect("related");
        assert!(!related.is_empty(), "radio returned no tracks");
        for track in related.iter().take(5) {
            println!("radio: {} — {}", track.title, track.artist);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    #[ignore = "hits the live YouTube Music API"]
    async fn live_provider_smoke() {
        let dir = std::env::temp_dir().join("cavia-ytmusic-smoke");
        let service = YtMusicService::new(dir.clone()).expect("service");

        let results = service.search("daft punk").await.expect("search");
        assert!(!results.is_empty(), "search returned no results");
        let first = &results[0];
        println!("first result: {} — {}", first.title, first.artist);

        let audio = service
            .resolve_audio(&first.video_id)
            .await
            .expect("resolve");
        assert!(audio.url.starts_with("http"));
        println!("stream: {} {} bps", audio.mime_type, audio.bitrate);

        let path = service
            .ensure_cached(&first.video_id, &audio.url)
            .await
            .expect("cache");
        let size = std::fs::metadata(&path).expect("metadata").len();
        println!("cached {} bytes at {}", size, path.display());
        assert!(size > 100_000, "cached audio was suspiciously small");

        // The cached stream must decode through the same Rodio/Symphonia path
        // local files use, otherwise playback would fail after the download.
        let file = std::fs::File::open(&path).expect("open cached stream");
        let decoder =
            rodio::Decoder::try_from(file).expect("rodio should decode the cached stream");
        let seconds = decoder
            .total_duration()
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        println!("decoded duration: {seconds}s");
        assert!(seconds > 30, "decoded stream had an implausible duration");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    #[ignore = "plays audio on the default output device"]
    async fn live_playback_smoke() {
        let dir = std::env::temp_dir().join("cavia-ytmusic-play");
        let service = YtMusicService::new(dir.clone()).expect("service");

        let results = service.search("daft punk").await.expect("search");
        let first = &results[0];
        let audio = service
            .resolve_audio(&first.video_id)
            .await
            .expect("resolve");
        let path = service
            .ensure_cached(&first.video_id, &audio.url)
            .await
            .expect("cache");

        let sink = rodio::stream::DeviceSinkBuilder::open_default_sink().expect("audio device");
        let player = rodio::Player::connect_new(&sink.mixer());
        let file = std::fs::File::open(&path).expect("open cached stream");
        let decoder = rodio::Decoder::try_from(file).expect("decode cached stream");
        player.append(decoder);
        player.play();

        std::thread::sleep(std::time::Duration::from_millis(1500));
        let played = player.get_pos();
        player.pause();
        println!("played {:?} of {}", played, first.title);
        assert!(
            played.as_millis() > 500,
            "the audio device produced no playback progress"
        );

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(dir);
    }
}
