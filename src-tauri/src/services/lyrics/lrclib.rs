use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER, USER_AGENT};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::Mutex;

const DEFAULT_BASE_URL: &str = "https://lrclib.net";
const CAVIA_USER_AGENT: &str = "Cavia Music/2.0 (https://github.com/StretchWave/Cavia_Music-2.0)";
const MIN_REQUEST_SPACING: Duration = Duration::from_millis(250);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(8);

/// LRCLIB accepts `duration` only in the inclusive range 1..=3600 seconds.
/// Longer or invalid durations are omitted instead of producing HTTP 400.
const LRCLIB_MIN_DURATION_S: i64 = 1;
const LRCLIB_MAX_DURATION_S: i64 = 3600;

/// Documented duration tolerance for identity validation. The LRCLIB server
/// itself filters by ±2 s when a duration is supplied; we allow one extra
/// second for scanner rounding (truncated milliseconds).
const DURATION_TOLERANCE_S: f64 = 3.0;

/// Maximum time a caller will block waiting for a cooldown to elapse. A longer
/// server-provided cooldown is surfaced as a typed rate-limit error so the UI
/// does not spin indefinitely.
const MAX_COOLDOWN_WAIT: Duration = Duration::from_secs(5);
const DEFAULT_RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(2);
const DEFAULT_UNAVAILABLE_COOLDOWN: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Deserialize)]
pub struct LrclibResponse {
    pub id: Option<i64>,
    #[serde(rename = "trackName")]
    pub track_name: Option<String>,
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    #[serde(rename = "albumName")]
    pub album_name: Option<String>,
    pub duration: Option<f64>,
    pub instrumental: Option<bool>,
    #[serde(rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    pub synced_lyrics: Option<String>,
}

/// Why a 200 response was rejected as belonging to a different recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityMismatch {
    MissingTitle,
    MissingArtist,
    Title,
    Artist,
    Album,
    Duration,
}

impl std::fmt::Display for IdentityMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentityMismatch::MissingTitle => f.write_str("response has no track title"),
            IdentityMismatch::MissingArtist => f.write_str("response has no artist name"),
            IdentityMismatch::Title => f.write_str("track title differs from the request"),
            IdentityMismatch::Artist => f.write_str("artist differs from the request"),
            IdentityMismatch::Album => f.write_str("album contradicts the request"),
            IdentityMismatch::Duration => f.write_str("duration is outside the tolerance"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LrclibResult {
    Found {
        lrclib_id: Option<i64>,
        plain_lyrics: Option<String>,
        synced_lyrics: Option<String>,
        is_instrumental: bool,
    },
    Instrumental {
        lrclib_id: Option<i64>,
    },
    NotFound,
    /// The server returned lyrics, but the response metadata does not describe
    /// the requested recording. Never cached and never shown as this track.
    Mismatch {
        reason: IdentityMismatch,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LrclibError {
    RateLimited { retry_after_s: Option<u64> },
    ServiceUnavailable { retry_after_s: Option<u64> },
    Timeout,
    Network(String),
    InvalidResponse(String),
}

impl std::fmt::Display for LrclibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LrclibError::RateLimited { retry_after_s } => {
                write!(
                    f,
                    "Rate limited by LRCLIB (Retry-After: {:?}s)",
                    retry_after_s
                )
            }
            LrclibError::ServiceUnavailable { retry_after_s } => {
                write!(
                    f,
                    "LRCLIB service temporarily unavailable (Retry-After: {:?}s)",
                    retry_after_s
                )
            }
            LrclibError::Timeout => write!(f, "Lyrics request timed out"),
            LrclibError::Network(msg) => write!(f, "Network error: {}", msg),
            LrclibError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
        }
    }
}

/// Shared request gate. Requests are fully serialized: the mutex guard is held
/// from before the request is sent until its response has been consumed. This
/// both spaces request starts and prevents overlapping outbound calls.
/// A server-provided cooldown (Retry-After on 429/503) is stored here so later
/// callers cannot ignore it. Dropping a future releases the tokio mutex, so
/// cancellation cannot leave the gate locked.
#[derive(Debug, Default)]
struct RequestGate {
    last_request: Option<Instant>,
    cooldown_until: Option<Instant>,
    cooldown_error: Option<LrclibError>,
}

pub struct LrclibClient {
    client: reqwest::Client,
    base_url: String,
    gate: Arc<Mutex<RequestGate>>,
}

impl LrclibClient {
    pub fn new() -> Result<Self, String> {
        Self::with_base_url(DEFAULT_BASE_URL.to_string())
    }

    pub fn with_base_url(base_url: String) -> Result<Self, String> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(USER_AGENT, HeaderValue::from_static(CAVIA_USER_AGENT));

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TOTAL_TIMEOUT)
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

        Ok(Self {
            client,
            base_url,
            gate: Arc::new(Mutex::new(RequestGate::default())),
        })
    }

    /// Fetches lyrics using the exact lookup endpoint GET /api/get.
    ///
    /// Identity policy (documented tolerances):
    /// - Title: normalized equality (case, whitespace, punctuation and common
    ///   typographic quote/dash variants are ignored). Version qualifiers such
    ///   as "(Live)" or "(Remastered)" are significant, so a live recording is
    ///   never silently accepted for a studio track.
    /// - Artist: normalized equality, or equal primary artist before a
    ///   "feat"/"ft"/"featuring"/"with"/"vs" segment.
    /// - Album: skipped when either side is absent. When both are present they
    ///   must match exactly after normalization, or one must be the other plus
    ///   an edition qualifier (deliberately not a general substring match).
    /// - Duration: skipped when either side is absent. When both are present a
    ///   difference greater than 3 seconds is rejected.
    /// - A response without a title or artist is rejected: identity cannot be
    ///   established from the remaining fields.
    pub async fn get_lyrics(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_s: Option<i64>,
    ) -> Result<LrclibResult, LrclibError> {
        let trimmed_title = title.trim();
        let trimmed_artist = artist.trim();

        if trimmed_title.is_empty() || trimmed_artist.is_empty() {
            return Ok(LrclibResult::NotFound);
        }

        let mut gate = self.gate.lock().await;

        let now = Instant::now();
        let mut wait = Duration::ZERO;
        if let Some(prev) = gate.last_request {
            wait = wait.max(MIN_REQUEST_SPACING.saturating_sub(now.duration_since(prev)));
        }
        if let Some(until) = gate.cooldown_until {
            if until > now {
                wait = wait.max(until.duration_since(now));
            }
        }
        if wait > MAX_COOLDOWN_WAIT {
            let seconds = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
            let error = match &gate.cooldown_error {
                Some(LrclibError::ServiceUnavailable { .. }) => LrclibError::ServiceUnavailable {
                    retry_after_s: Some(seconds),
                },
                _ => LrclibError::RateLimited {
                    retry_after_s: Some(seconds),
                },
            };
            return Err(error);
        }
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        gate.last_request = Some(Instant::now());

        let url = format!("{}/api/get", self.base_url.trim_end_matches('/'));

        let mut query_params: Vec<(&str, String)> = Vec::new();
        query_params.push(("track_name", trimmed_title.to_string()));
        query_params.push(("artist_name", trimmed_artist.to_string()));

        if let Some(al) = album {
            let trimmed_album = al.trim();
            if !trimmed_album.is_empty() {
                query_params.push(("album_name", trimmed_album.to_string()));
            }
        }

        // Omit durations outside LRCLIB's accepted range instead of provoking
        // an HTTP 400 for long tracks. Title and artist remain sufficient.
        if let Some(dur) = duration_s {
            if (LRCLIB_MIN_DURATION_S..=LRCLIB_MAX_DURATION_S).contains(&dur) {
                query_params.push(("duration", dur.to_string()));
            }
        }

        let resp = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .map_err(|err| {
                if err.is_timeout() {
                    LrclibError::Timeout
                } else {
                    LrclibError::Network(err.to_string())
                }
            })?;

        let status = resp.status();
        let headers = resp.headers().clone();

        if status.is_success() {
            let json_body: LrclibResponse = resp.json().await.map_err(|err| {
                LrclibError::InvalidResponse(format!("Failed to parse LRCLIB response: {err}"))
            })?;

            if let Some(reason) =
                validate_identity(trimmed_title, trimmed_artist, album, duration_s, &json_body)
            {
                return Ok(LrclibResult::Mismatch { reason });
            }

            if json_body.instrumental == Some(true) {
                return Ok(LrclibResult::Instrumental {
                    lrclib_id: json_body.id,
                });
            }

            let has_synced = json_body
                .synced_lyrics
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            let has_plain = json_body
                .plain_lyrics
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);

            if has_synced || has_plain {
                return Ok(LrclibResult::Found {
                    lrclib_id: json_body.id,
                    plain_lyrics: json_body.plain_lyrics.filter(|s| !s.trim().is_empty()),
                    synced_lyrics: json_body.synced_lyrics.filter(|s| !s.trim().is_empty()),
                    is_instrumental: false,
                });
            }

            return Ok(LrclibResult::NotFound);
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(LrclibResult::NotFound);
        }

        let retry_after_s = parse_retry_after(&headers);

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let error = LrclibError::RateLimited { retry_after_s };
            apply_cooldown(&mut gate, &error, DEFAULT_RATE_LIMIT_COOLDOWN);
            return Err(error);
        }

        if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            let error = LrclibError::ServiceUnavailable { retry_after_s };
            apply_cooldown(&mut gate, &error, DEFAULT_UNAVAILABLE_COOLDOWN);
            return Err(error);
        }

        if status.is_server_error() {
            return Err(LrclibError::Network(format!(
                "LRCLIB service returned HTTP {}",
                status.as_u16()
            )));
        }

        Err(LrclibError::InvalidResponse(format!(
            "Unexpected HTTP status {}",
            status.as_u16()
        )))
    }
}

fn apply_cooldown(gate: &mut RequestGate, error: &LrclibError, fallback: Duration) {
    let wait = match error {
        LrclibError::RateLimited { retry_after_s }
        | LrclibError::ServiceUnavailable { retry_after_s } => {
            retry_after_s.map(Duration::from_secs).unwrap_or(fallback)
        }
        _ => fallback,
    };
    gate.cooldown_until = Some(Instant::now() + wait);
    gate.cooldown_error = Some(error.clone());
}

/// Parses both valid Retry-After representations: delay-seconds and HTTP-date.
fn parse_retry_after(headers: &HeaderMap) -> Option<u64> {
    let raw = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = raw.parse::<u64>() {
        return Some(seconds);
    }
    if let Ok(when) = httpdate::parse_http_date(raw) {
        return Some(match when.duration_since(SystemTime::now()) {
            Ok(delta) => delta.as_secs().max(1),
            Err(_) => 1,
        });
    }
    None
}

/// Normalizes text for identity comparison: lowercases, maps typographic
/// quotes/dashes, treats every non-alphanumeric character as a word separator,
/// and collapses whitespace. Accents remain significant (É != E).
fn normalize_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;
    for ch in input.chars() {
        let mapped = match ch {
            '\u{2018}' | '\u{2019}' | '\u{02bc}' | '`' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            other => other,
        };
        for lower in mapped.to_lowercase() {
            if lower.is_alphanumeric() {
                if pending_space && !out.is_empty() {
                    out.push(' ');
                }
                pending_space = false;
                out.push(lower);
            } else {
                pending_space = true;
            }
        }
    }
    out
}

fn primary_artist(normalized: &str) -> &str {
    for separator in [" feat ", " ft ", " featuring ", " with ", " vs "] {
        if let Some(index) = normalized.find(separator) {
            return &normalized[..index];
        }
    }
    normalized
}

fn title_matches(requested: &str, returned: &str) -> bool {
    normalize_text(requested) == normalize_text(returned)
}

fn artist_matches(requested: &str, returned: &str) -> bool {
    let requested_norm = normalize_text(requested);
    let returned_norm = normalize_text(returned);
    if requested_norm.is_empty() || returned_norm.is_empty() {
        return false;
    }
    if requested_norm == returned_norm {
        return true;
    }
    let requested_primary = primary_artist(&requested_norm);
    !requested_primary.is_empty() && requested_primary == primary_artist(&returned_norm)
}

/// Words that introduce a legitimate edition/repackaging suffix. Only these
/// may extend an album title: "Obsidian" vs "Obsidian (Deluxe Edition)" is the
/// same release family, while "Obsidianic", "The Obsidian Sessions" or
/// "Obsidian, Vol. 2" are different albums and are rejected.
const EDITION_QUALIFIERS: &[&str] = &[
    "deluxe",
    "remaster",
    "remastered",
    "expanded",
    "anniversary",
    "special",
    "bonus",
    "edition",
    "version",
    "explicit",
    "collector",
    "reissue",
];

fn is_edition_of(base: &str, candidate: &str) -> bool {
    let Some(rest) = candidate.strip_prefix(base) else {
        return false;
    };
    let rest = rest.trim_start();
    !rest.is_empty()
        && rest.len() < candidate.len()
        && EDITION_QUALIFIERS.iter().any(|word| rest.starts_with(word))
}

/// Album policy: skipped when either side is absent. When both are present they
/// must match exactly after normalization, or one must be the other plus an
/// edition qualifier. Deliberately not a substring match, so a near-miss album
/// title cannot smuggle in a different recording.
fn album_matches(requested: Option<&str>, returned: Option<&str>) -> bool {
    let (Some(requested), Some(returned)) = (requested, returned) else {
        return true;
    };
    let requested_norm = normalize_text(requested);
    let returned_norm = normalize_text(returned);
    if requested_norm.is_empty() || returned_norm.is_empty() {
        return true;
    }
    requested_norm == returned_norm
        || is_edition_of(&requested_norm, &returned_norm)
        || is_edition_of(&returned_norm, &requested_norm)
}

fn duration_matches(requested_s: Option<i64>, returned_s: Option<f64>) -> bool {
    match (requested_s, returned_s) {
        (Some(requested), Some(returned)) if requested > 0 && returned > 0.0 => {
            (requested as f64 - returned).abs() <= DURATION_TOLERANCE_S
        }
        _ => true,
    }
}

fn validate_identity(
    requested_title: &str,
    requested_artist: &str,
    requested_album: Option<&str>,
    requested_duration_s: Option<i64>,
    response: &LrclibResponse,
) -> Option<IdentityMismatch> {
    let Some(returned_title) = response.track_name.as_deref() else {
        return Some(IdentityMismatch::MissingTitle);
    };
    if !title_matches(requested_title, returned_title) {
        return Some(IdentityMismatch::Title);
    }

    let Some(returned_artist) = response.artist_name.as_deref() else {
        return Some(IdentityMismatch::MissingArtist);
    };
    if !artist_matches(requested_artist, returned_artist) {
        return Some(IdentityMismatch::Artist);
    }

    if !album_matches(requested_album, response.album_name.as_deref()) {
        return Some(IdentityMismatch::Album);
    }

    if !duration_matches(requested_duration_s, response.duration) {
        return Some(IdentityMismatch::Duration);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    struct MockServer {
        base_url: String,
        requests: Arc<Mutex<Vec<(Instant, String)>>>,
    }

    impl MockServer {
        async fn request_count(&self) -> usize {
            self.requests.lock().await.len()
        }

        async fn requests(&self) -> Vec<(Instant, String)> {
            self.requests.lock().await.clone()
        }
    }

    /// Concurrent mock HTTP server. Each connection is handled on its own task,
    /// and every request (arrival time + raw request head) is recorded.
    async fn spawn_mock_server<F>(handler: F) -> MockServer
    where
        F: Fn(usize, String) -> (u16, Vec<(&'static str, &'static str)>, String)
            + Send
            + Sync
            + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let requests_for_task = requests.clone();
        let handler = Arc::new(handler);
        let counter = Arc::new(AtomicUsize::new(0));

        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let requests = requests_for_task.clone();
                let handler = handler.clone();
                let index = counter.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    loop {
                        match socket.read(&mut chunk).await {
                            Ok(0) => break,
                            Ok(n) => {
                                buf.extend_from_slice(&chunk[..n]);
                                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                            Err(_) => return,
                        }
                    }
                    let req_str = String::from_utf8_lossy(&buf).to_string();
                    requests
                        .lock()
                        .await
                        .push((Instant::now(), req_str.clone()));
                    let (status, headers, body) = handler(index, req_str);
                    let status_line = match status {
                        200 => "HTTP/1.1 200 OK",
                        400 => "HTTP/1.1 400 Bad Request",
                        404 => "HTTP/1.1 404 Not Found",
                        429 => "HTTP/1.1 429 Too Many Requests",
                        500 => "HTTP/1.1 500 Internal Server Error",
                        503 => "HTTP/1.1 503 Service Unavailable",
                        _ => "HTTP/1.1 400 Bad Request",
                    };
                    let mut response = format!(
                        "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n",
                        body.len()
                    );
                    for (header, value) in headers {
                        response.push_str(&format!("{header}: {value}\r\n"));
                    }
                    response.push_str("\r\n");
                    response.push_str(&body);
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        MockServer { base_url, requests }
    }

    fn json_response(
        track_name: &str,
        artist_name: &str,
        album_name: Option<&str>,
        duration: Option<f64>,
    ) -> String {
        let album = album_name
            .map(|a| format!("\"{a}\""))
            .unwrap_or_else(|| "null".to_string());
        let duration = duration
            .map(|d| d.to_string())
            .unwrap_or_else(|| "null".to_string());
        format!(
            r#"{{"id":1,"trackName":"{track_name}","artistName":"{artist_name}","albumName":{album},"duration":{duration},"instrumental":false,"plainLyrics":"Line one","syncedLyrics":"[00:01.00]Line one"}}"#
        )
    }

    async fn lookup(
        handler: impl Fn(usize, String) -> (u16, Vec<(&'static str, &'static str)>, String)
        + Send
        + Sync
        + 'static,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration: Option<i64>,
    ) -> (Result<LrclibResult, LrclibError>, MockServer) {
        let server = spawn_mock_server(handler).await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();
        let result = client.get_lyrics(title, artist, album, duration).await;
        (result, server)
    }

    #[tokio::test]
    async fn test_exact_match_accepted() {
        let (result, server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Cavia Artist", Some("Obsidian"), Some(180.0)),
                )
            },
            "Midnight",
            "Cavia Artist",
            Some("Obsidian"),
            Some(180),
        )
        .await;

        assert!(matches!(result, Ok(LrclibResult::Found { .. })));
        let requests = server.requests().await;
        assert_eq!(requests.len(), 1);
        let request = &requests[0].1;
        assert!(
            request
                .to_lowercase()
                .contains("user-agent: cavia music/2.0")
        );
        assert!(request.contains("track_name=Midnight"));
        assert!(request.contains("artist_name=Cavia+Artist"));
        assert!(request.contains("album_name=Obsidian"));
        assert!(request.contains("duration=180"));
    }

    #[tokio::test]
    async fn test_case_and_punctuation_variations_accepted() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response(
                        "  midnight   echoes! ",
                        "CAVIA-ARTIST",
                        Some("Obsidian (Deluxe Edition)"),
                        Some(180.0),
                    ),
                )
            },
            "Midnight Echoes",
            "Cavia Artist",
            Some("Obsidian"),
            Some(180),
        )
        .await;

        assert!(matches!(result, Ok(LrclibResult::Found { .. })));
    }

    #[tokio::test]
    async fn test_different_artist_rejected() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Some Other Band", None, Some(180.0)),
                )
            },
            "Midnight",
            "Cavia Artist",
            None,
            Some(180),
        )
        .await;

        assert_eq!(
            result,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::Artist
            })
        );
    }

    #[tokio::test]
    async fn test_same_title_different_artist_rejected() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Shared Title", "Tribute Band", None, None),
                )
            },
            "Shared Title",
            "Original Artist",
            None,
            None,
        )
        .await;

        assert_eq!(
            result,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::Artist
            })
        );
    }

    #[tokio::test]
    async fn test_live_version_not_accepted_for_studio_track() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight (Live)", "Cavia Artist", None, Some(180.0)),
                )
            },
            "Midnight",
            "Cavia Artist",
            None,
            Some(180),
        )
        .await;

        assert_eq!(
            result,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::Title
            })
        );
    }

    #[tokio::test]
    async fn test_cover_by_other_artist_rejected() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Cover Ensemble", None, None),
                )
            },
            "Midnight",
            "Cavia Artist",
            None,
            None,
        )
        .await;

        assert_eq!(
            result,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::Artist
            })
        );
    }

    #[tokio::test]
    async fn test_album_contradiction_rejected() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Cavia Artist", Some("Different Album"), None),
                )
            },
            "Midnight",
            "Cavia Artist",
            Some("Obsidian"),
            None,
        )
        .await;

        assert_eq!(
            result,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::Album
            })
        );
    }

    #[tokio::test]
    async fn test_album_near_miss_rejected() {
        // "Obsidianic" contains "Obsidian" but is a different album. A plain
        // substring test would silently accept it.
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Cavia Artist", Some("Obsidianic"), None),
                )
            },
            "Midnight",
            "Cavia Artist",
            Some("Obsidian"),
            None,
        )
        .await;

        assert_eq!(
            result,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::Album
            })
        );

        // A separate volume is also a different release, not an edition.
        let (volume, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Cavia Artist", Some("Obsidian, Vol. 2"), None),
                )
            },
            "Midnight",
            "Cavia Artist",
            Some("Obsidian"),
            None,
        )
        .await;

        assert_eq!(
            volume,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::Album
            })
        );
    }

    #[tokio::test]
    async fn test_duration_mismatch_rejected() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Cavia Artist", None, Some(240.0)),
                )
            },
            "Midnight",
            "Cavia Artist",
            None,
            Some(180),
        )
        .await;

        assert_eq!(
            result,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::Duration
            })
        );
    }

    #[tokio::test]
    async fn test_duration_within_tolerance_accepted() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Cavia Artist", None, Some(182.5)),
                )
            },
            "Midnight",
            "Cavia Artist",
            None,
            Some(180),
        )
        .await;

        assert!(matches!(result, Ok(LrclibResult::Found { .. })));
    }

    #[tokio::test]
    async fn test_missing_optional_album_and_duration_accepted() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Midnight", "Cavia Artist", None, None),
                )
            },
            "Midnight",
            "Cavia Artist",
            Some("Obsidian"),
            Some(180),
        )
        .await;

        assert!(matches!(result, Ok(LrclibResult::Found { .. })));
    }

    #[tokio::test]
    async fn test_missing_response_artist_rejected() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    r#"{"id":1,"trackName":"Midnight","plainLyrics":"Line one"}"#.to_string(),
                )
            },
            "Midnight",
            "Cavia Artist",
            None,
            None,
        )
        .await;

        assert_eq!(
            result,
            Ok(LrclibResult::Mismatch {
                reason: IdentityMismatch::MissingArtist
            })
        );
    }

    #[tokio::test]
    async fn test_unicode_metadata_matches_case_insensitively() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("ÉLÉGIE POUR UN RÊVE", "Café Ópera", None, None),
                )
            },
            "Élégie pour un rêve",
            "Café Ópera",
            None,
            None,
        )
        .await;

        assert!(matches!(result, Ok(LrclibResult::Found { .. })));
    }

    #[tokio::test]
    async fn test_duration_parameter_range() {
        async fn duration_query(duration: Option<i64>) -> Option<String> {
            let (_, server) = lookup(
                |_, _| {
                    (
                        200,
                        vec![("Content-Type", "application/json")],
                        json_response("Midnight", "Cavia Artist", None, None),
                    )
                },
                "Midnight",
                "Cavia Artist",
                None,
                duration,
            )
            .await;
            let requests = server.requests().await;
            requests[0]
                .1
                .split_whitespace()
                .find(|part| part.starts_with("/api/get"))
                .map(|path| path.to_string())
        }

        let normal = duration_query(Some(180)).await.unwrap();
        assert!(normal.contains("duration=180"));

        let exact_max = duration_query(Some(3600)).await.unwrap();
        assert!(exact_max.contains("duration=3600"));

        let above_max = duration_query(Some(3601)).await.unwrap();
        assert!(!above_max.contains("duration="));

        let zero = duration_query(Some(0)).await.unwrap();
        assert!(!zero.contains("duration="));

        let missing = duration_query(None).await.unwrap();
        assert!(!missing.contains("duration="));
    }

    #[tokio::test]
    async fn test_lrclib_404_not_found() {
        let (result, _server) = lookup(
            |_, _| (404, vec![], "{}".to_string()),
            "Obscure Track",
            "Unknown",
            None,
            None,
        )
        .await;
        assert_eq!(result, Ok(LrclibResult::NotFound));
    }

    #[tokio::test]
    async fn test_lrclib_invalid_json_handling() {
        let (result, _server) = lookup(
            |_, _| {
                (
                    200,
                    vec![("Content-Type", "text/html")],
                    "<html>Error page</html>".to_string(),
                )
            },
            "Track",
            "Artist",
            None,
            None,
        )
        .await;
        assert!(matches!(result, Err(LrclibError::InvalidResponse(_))));
    }

    #[tokio::test]
    async fn test_lrclib_429_retry_after_seconds() {
        let (result, _server) = lookup(
            |_, _| (429, vec![("Retry-After", "12")], "Rate limited".to_string()),
            "Track",
            "Artist",
            None,
            None,
        )
        .await;
        assert_eq!(
            result,
            Err(LrclibError::RateLimited {
                retry_after_s: Some(12)
            })
        );
    }

    #[tokio::test]
    async fn test_lrclib_503_retry_after_seconds() {
        let (result, _server) = lookup(
            |_, _| (503, vec![("Retry-After", "30")], "Unavailable".to_string()),
            "Track",
            "Artist",
            None,
            None,
        )
        .await;
        assert_eq!(
            result,
            Err(LrclibError::ServiceUnavailable {
                retry_after_s: Some(30)
            })
        );
    }

    #[tokio::test]
    async fn test_retry_after_http_date_parsed() {
        let when = SystemTime::now() + Duration::from_secs(5);
        let date: &'static str = Box::leak(httpdate::fmt_http_date(when).into_boxed_str());
        let (result, _server) = lookup(
            move |_, _| (429, vec![("Retry-After", date)], "Rate limited".to_string()),
            "Track",
            "Artist",
            None,
            None,
        )
        .await;

        match result {
            Err(LrclibError::RateLimited {
                retry_after_s: Some(seconds),
            }) => assert!((3..=5).contains(&seconds), "got {seconds}"),
            other => panic!("expected HTTP-date rate limit, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_cooldown_blocks_subsequent_request() {
        let server = spawn_mock_server(|index, _| {
            if index == 0 {
                (429, vec![("Retry-After", "1")], "Rate limited".to_string())
            } else {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Track", "Artist", None, None),
                )
            }
        })
        .await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();

        let first = client.get_lyrics("Track", "Artist", None, None).await;
        assert!(matches!(first, Err(LrclibError::RateLimited { .. })));

        let started = Instant::now();
        let second = client.get_lyrics("Track", "Artist", None, None).await;
        let elapsed = started.elapsed();

        assert!(matches!(second, Ok(LrclibResult::Found { .. })));
        assert!(
            elapsed >= Duration::from_millis(900),
            "second request ignored the cooldown: {elapsed:?}"
        );
        assert_eq!(server.request_count().await, 2);
    }

    #[tokio::test]
    async fn test_long_cooldown_fails_fast() {
        let server = spawn_mock_server(|_, _| {
            (
                429,
                vec![("Retry-After", "120")],
                "Rate limited".to_string(),
            )
        })
        .await;
        let client = LrclibClient::with_base_url(server.base_url.clone()).unwrap();

        let first = client.get_lyrics("Track", "Artist", None, None).await;
        assert!(matches!(first, Err(LrclibError::RateLimited { .. })));

        let started = Instant::now();
        let second = client.get_lyrics("Track", "Artist", None, None).await;
        let elapsed = started.elapsed();

        match second {
            Err(LrclibError::RateLimited {
                retry_after_s: Some(seconds),
            }) => assert!(seconds >= 100, "unexpected remaining cooldown: {seconds}"),
            other => panic!("expected fail-fast rate limit, got {other:?}"),
        }
        assert!(
            elapsed < Duration::from_secs(1),
            "long cooldown should fail fast, waited {elapsed:?}"
        );
        assert_eq!(server.request_count().await, 1);
    }

    #[tokio::test]
    async fn test_requests_are_serialized() {
        let server = spawn_mock_server(|_, _| {
            std::thread::sleep(Duration::from_millis(120));
            (
                200,
                vec![("Content-Type", "application/json")],
                json_response("Track", "Artist", None, None),
            )
        })
        .await;
        let client = Arc::new(LrclibClient::with_base_url(server.base_url.clone()).unwrap());

        let first = {
            let client = client.clone();
            tokio::spawn(async move { client.get_lyrics("Track", "Artist", None, None).await })
        };
        let second = {
            let client = client.clone();
            tokio::spawn(async move { client.get_lyrics("Track", "Artist", None, None).await })
        };

        assert!(matches!(
            first.await.unwrap(),
            Ok(LrclibResult::Found { .. })
        ));
        assert!(matches!(
            second.await.unwrap(),
            Ok(LrclibResult::Found { .. })
        ));

        let requests = server.requests().await;
        assert_eq!(requests.len(), 2);
        let gap = requests[1].0.duration_since(requests[0].0);
        assert!(
            gap >= Duration::from_millis(110),
            "requests overlapped instead of serializing: {gap:?}"
        );
    }

    #[tokio::test]
    async fn test_cancelled_waiter_does_not_lock_gate() {
        let server = spawn_mock_server(|index, _| {
            if index == 0 {
                (429, vec![("Retry-After", "1")], "Rate limited".to_string())
            } else {
                (
                    200,
                    vec![("Content-Type", "application/json")],
                    json_response("Track", "Artist", None, None),
                )
            }
        })
        .await;
        let client = Arc::new(LrclibClient::with_base_url(server.base_url.clone()).unwrap());

        let first = client.get_lyrics("Track", "Artist", None, None).await;
        assert!(matches!(first, Err(LrclibError::RateLimited { .. })));

        let cancelled = {
            let client = client.clone();
            tokio::spawn(async move { client.get_lyrics("Track", "Artist", None, None).await })
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancelled.abort();
        let _ = cancelled.await;

        let result = tokio::time::timeout(
            Duration::from_secs(6),
            client.get_lyrics("Track", "Artist", None, None),
        )
        .await
        .expect("gate remained locked after cancellation");

        assert!(matches!(result, Ok(LrclibResult::Found { .. })));
    }
}
