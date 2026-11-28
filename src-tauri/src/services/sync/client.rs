use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncEventRecord {
    pub server_seq: Option<i64>,
    pub operation_id: String,
    pub user_id: Option<String>,
    pub device_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub operation_type: String,
    pub payload: serde_json::Value,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudPlaylistRecord {
    pub id: String,
    pub user_id: Option<String>,
    pub name: String,
    pub description: String,
    pub artwork_uri: Option<String>,
    pub is_deleted: bool,
    pub client_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudPlaylistTrackRecord {
    pub id: String,
    pub playlist_id: String,
    pub user_id: Option<String>,
    pub cloud_track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub content_hash: Option<String>,
    pub position: i64,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudLikedTrackRecord {
    pub cloud_track_id: String,
    pub user_id: Option<String>,
    pub content_hash: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub is_liked: bool,
}

#[derive(Clone)]
pub struct SupabaseRestClient {
    client: Client,
}

impl Default for SupabaseRestClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SupabaseRestClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client }
    }

    pub async fn register_device(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        user_id: &str,
        device_id: &str,
        device_name: &str,
        platform: &str,
    ) -> Result<(), String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!("{clean_url}/rest/v1/cloud_devices");

        let body = serde_json::json!({
            "user_id": user_id,
            "device_id": device_id,
            "device_name": device_name,
            "platform": platform
        });

        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .header("Prefer", "resolution=merge-duplicates")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to register device: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Your session has expired. Please sign in again.".to_string());
        }
        if !status.is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Device registration failed: {err}"));
        }

        Ok(())
    }

    pub async fn push_sync_event(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        user_id: &str,
        device_id: &str,
        operation_id: &str,
        entity_type: &str,
        entity_id: &str,
        operation_type: &str,
        payload: &serde_json::Value,
    ) -> Result<(), String> {
        let clean_url = supabase_url.trim_end_matches('/');
        // PostgREST needs the unique key named explicitly: without on_conflict a
        // retried push returns 409 and the outbox entry would never clear.
        let endpoint =
            format!("{clean_url}/rest/v1/cloud_sync_events?on_conflict=user_id,operation_id");

        let body = serde_json::json!({
            "operation_id": operation_id,
            "user_id": user_id,
            "device_id": device_id,
            "entity_type": entity_type,
            "entity_id": entity_id,
            "operation_type": operation_type,
            "payload": payload
        });

        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .header("Prefer", "resolution=ignore-duplicates")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network error pushing sync event: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Your session has expired. Please sign in again.".to_string());
        }
        if !status.is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Failed to push sync event ({status}): {err}"));
        }

        // Also update the projected table for direct queryability
        match (entity_type, operation_type) {
            ("playlist", "create" | "update") => {
                let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let desc = payload
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let updated_at = payload.get("clientUpdatedAt").and_then(|v| v.as_str());

                let pl_endpoint = format!("{clean_url}/rest/v1/cloud_playlists");
                let mut pl_body = serde_json::json!({
                    "id": entity_id,
                    "user_id": user_id,
                    "name": name,
                    "description": desc,
                    "is_deleted": false
                });
                if let Some(updated_at) = updated_at {
                    pl_body["client_updated_at"] = serde_json::json!(updated_at);
                }

                let proj_resp = self
                    .client
                    .post(&pl_endpoint)
                    .header("apikey", anon_key)
                    .header("Authorization", format!("Bearer {access_token}"))
                    .header("Content-Type", "application/json")
                    .header("Prefer", "resolution=merge-duplicates")
                    .json(&pl_body)
                    .send()
                    .await
                    .map_err(|e| format!("Network error projecting playlist upsert: {e}"))?;
                check_projection_response(proj_resp, "cloud_playlists upsert").await?;
            }
            ("playlist", "delete") => {
                let pl_endpoint = format!("{clean_url}/rest/v1/cloud_playlists");
                let pl_body = serde_json::json!({
                    "is_deleted": true
                });

                let proj_resp = self
                    .client
                    .patch(&pl_endpoint)
                    .query(&[("id", format!("eq.{entity_id}"))])
                    .header("apikey", anon_key)
                    .header("Authorization", format!("Bearer {access_token}"))
                    .header("Content-Type", "application/json")
                    .json(&pl_body)
                    .send()
                    .await
                    .map_err(|e| format!("Network error projecting playlist delete: {e}"))?;
                check_projection_response(proj_resp, "cloud_playlists delete").await?;
            }
            ("playlist_track", "add") => {
                let pl_id = payload
                    .get("playlistId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let cloud_track_id = payload
                    .get("cloudTrackId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let artist = payload.get("artist").and_then(|v| v.as_str()).unwrap_or("");
                let album = payload.get("album").and_then(|v| v.as_str()).unwrap_or("");
                let duration_ms = payload
                    .get("durationMs")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let content_hash = payload.get("contentHash").and_then(|v| v.as_str());
                let pos = payload
                    .get("position")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                let pt_endpoint = format!("{clean_url}/rest/v1/cloud_playlist_tracks");
                let pt_body = serde_json::json!({
                    "id": entity_id,
                    "user_id": user_id,
                    "playlist_id": pl_id,
                    "cloud_track_id": cloud_track_id,
                    "title": title,
                    "artist": artist,
                    "album": album,
                    "duration_ms": duration_ms,
                    "content_hash": content_hash,
                    "position": pos,
                    "is_deleted": false
                });

                let proj_resp = self
                    .client
                    .post(&pt_endpoint)
                    .header("apikey", anon_key)
                    .header("Authorization", format!("Bearer {access_token}"))
                    .header("Content-Type", "application/json")
                    .header("Prefer", "resolution=merge-duplicates")
                    .json(&pt_body)
                    .send()
                    .await
                    .map_err(|e| format!("Network error projecting playlist track add: {e}"))?;
                check_projection_response(proj_resp, "cloud_playlist_tracks add").await?;
            }
            ("playlist_track", "remove") => {
                let pt_endpoint = format!("{clean_url}/rest/v1/cloud_playlist_tracks");
                let pt_body = serde_json::json!({
                    "is_deleted": true
                });

                let proj_resp = self
                    .client
                    .patch(&pt_endpoint)
                    .query(&[("id", format!("eq.{entity_id}"))])
                    .header("apikey", anon_key)
                    .header("Authorization", format!("Bearer {access_token}"))
                    .header("Content-Type", "application/json")
                    .json(&pt_body)
                    .send()
                    .await
                    .map_err(|e| format!("Network error projecting playlist track remove: {e}"))?;
                check_projection_response(proj_resp, "cloud_playlist_tracks remove").await?;
            }
            ("liked_track", "like" | "unlike") => {
                let is_liked = operation_type == "like";
                let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let artist = payload.get("artist").and_then(|v| v.as_str()).unwrap_or("");
                let album = payload.get("album").and_then(|v| v.as_str()).unwrap_or("");
                let duration_ms = payload
                    .get("durationMs")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let content_hash = payload.get("contentHash").and_then(|v| v.as_str());

                let lt_endpoint = format!("{clean_url}/rest/v1/cloud_liked_tracks");
                let lt_body = serde_json::json!({
                    "user_id": user_id,
                    "cloud_track_id": entity_id,
                    "title": title,
                    "artist": artist,
                    "album": album,
                    "duration_ms": duration_ms,
                    "content_hash": content_hash,
                    "is_liked": is_liked
                });

                let proj_resp = self
                    .client
                    .post(&lt_endpoint)
                    .header("apikey", anon_key)
                    .header("Authorization", format!("Bearer {access_token}"))
                    .header("Content-Type", "application/json")
                    .header("Prefer", "resolution=merge-duplicates")
                    .json(&lt_body)
                    .send()
                    .await
                    .map_err(|e| format!("Network error projecting liked track status: {e}"))?;
                check_projection_response(proj_resp, "cloud_liked_tracks upsert").await?;
            }
            _ => {}
        }

        Ok(())
    }

    pub async fn pull_events_since(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        since_seq: i64,
        limit: usize,
    ) -> Result<Vec<CloudSyncEventRecord>, String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!(
            "{clean_url}/rest/v1/cloud_sync_events?server_seq=gt.{since_seq}&order=server_seq.asc&limit={limit}"
        );

        let resp = self
            .client
            .get(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await
            .map_err(|e| format!("Failed to pull sync events: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Your session has expired. Please sign in again.".to_string());
        }
        if !status.is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Pull events query failed: {err}"));
        }

        let events: Vec<CloudSyncEventRecord> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse sync events response: {e}"))?;

        Ok(events)
    }

    pub async fn delete_all_cloud_data(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
    ) -> Result<(), String> {
        let clean_url = supabase_url.trim_end_matches('/');

        for (table, filter_col) in [
            ("cloud_playlists", "id"),
            ("cloud_playlist_tracks", "id"),
            ("cloud_liked_tracks", "cloud_track_id"),
            ("cloud_sync_events", "operation_id"),
            ("cloud_devices", "device_id"),
        ] {
            let resp = self
                .client
                .delete(format!(
                    "{clean_url}/rest/v1/{table}?{filter_col}=not.is.null"
                ))
                .header("apikey", anon_key)
                .header("Authorization", format!("Bearer {access_token}"))
                .send()
                .await
                .map_err(|e| format!("Network error deleting {table}: {e}"))?;

            check_projection_response(resp, &format!("delete {table}")).await?;
        }

        Ok(())
    }

    pub async fn upsert_table_rows(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        table: &str,
        rows: &serde_json::Value,
        on_conflict: Option<&str>,
    ) -> Result<(), String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = match on_conflict {
            Some(cols) if !cols.trim().is_empty() => {
                format!("{clean_url}/rest/v1/{table}?on_conflict={cols}")
            }
            _ => format!("{clean_url}/rest/v1/{table}"),
        };

        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Prefer", "resolution=merge-duplicates,return=minimal")
            .json(rows)
            .send()
            .await
            .map_err(|e| format!("Network error pushing {table}: {e}"))?;

        check_projection_response(resp, &format!("upsert {table}")).await
    }

    pub async fn insert_table_rows_ignore_duplicates(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        table: &str,
        rows: &serde_json::Value,
        on_conflict: Option<&str>,
    ) -> Result<(), String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = match on_conflict {
            Some(cols) if !cols.trim().is_empty() => {
                format!("{clean_url}/rest/v1/{table}?on_conflict={cols}")
            }
            _ => format!("{clean_url}/rest/v1/{table}"),
        };

        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Prefer", "resolution=ignore-duplicates,return=minimal")
            .json(rows)
            .send()
            .await
            .map_err(|e| format!("Network error pushing {table}: {e}"))?;

        check_projection_response(resp, &format!("insert {table}")).await
    }

    pub async fn get_table_rows(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        table: &str,
        query: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = if query.trim().is_empty() {
            format!("{clean_url}/rest/v1/{table}")
        } else {
            format!("{clean_url}/rest/v1/{table}?{query}")
        };

        let resp = self
            .client
            .get(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await
            .map_err(|e| format!("Network error querying {table}: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Your session has expired. Please sign in again.".to_string());
        }
        if !status.is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Cloud query {table} failed ({status}): {err}"));
        }

        resp.json::<Vec<serde_json::Value>>()
            .await
            .map_err(|e| format!("Invalid JSON querying {table}: {e}"))
    }

    pub async fn patch_table_rows(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        table: &str,
        filter_query: &str,
        patch: &serde_json::Value,
    ) -> Result<(), String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!("{clean_url}/rest/v1/{table}?{filter_query}");
        let resp = self
            .client
            .patch(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Prefer", "return=minimal")
            .json(patch)
            .send()
            .await
            .map_err(|e| format!("Network error updating {table}: {e}"))?;

        check_projection_response(resp, &format!("patch {table}")).await
    }

    pub async fn call_rpc<T: serde::de::DeserializeOwned>(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        function_name: &str,
        params: &serde_json::Value,
    ) -> Result<T, String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!("{clean_url}/rest/v1/rpc/{function_name}");
        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .json(params)
            .send()
            .await
            .map_err(|e| format!("Network error reaching Connect service: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Your account session has expired. Please sign in again.".to_string());
        }
        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&err_text)
                && let Some(msg) = val.get("message").and_then(serde_json::Value::as_str)
            {
                return Err(msg.to_string());
            }
            return Err(format!("Connect request failed ({status}): {err_text}"));
        }

        resp.json::<T>()
            .await
            .map_err(|e| format!("Invalid response from Connect service: {e}"))
    }

    pub async fn call_rpc_void(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
        function_name: &str,
        params: &serde_json::Value,
    ) -> Result<(), String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!("{clean_url}/rest/v1/rpc/{function_name}");
        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .json(params)
            .send()
            .await
            .map_err(|e| format!("Network error reaching Connect service: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Your account session has expired. Please sign in again.".to_string());
        }
        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&err_text)
                && let Some(msg) = val.get("message").and_then(serde_json::Value::as_str)
            {
                return Err(msg.to_string());
            }
            return Err(format!("Connect request failed ({status}): {err_text}"));
        }
        Ok(())
    }
}

async fn check_projection_response(resp: reqwest::Response, op: &str) -> Result<(), String> {
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Your session has expired. Please sign in again.".to_string());
    }
    if !status.is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Cloud {op} failed ({status}): {err}"));
    }
    Ok(())
}
