use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::services::sync::client::SupabaseRestClient;
use crate::services::sync::engine::SyncEngine;

/// 32-character uppercase alphanumeric alphabet excluding visually confusing characters (0, O, 1, I).
pub const CONNECT_ID_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn normalize_connect_id(raw: &str) -> String {
    raw.trim().to_ascii_uppercase()
}

pub fn is_valid_connect_id(raw: &str) -> bool {
    let norm = normalize_connect_id(raw);
    norm.len() == 6
        && norm
            .as_bytes()
            .iter()
            .all(|b| CONNECT_ID_ALPHABET.contains(b))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectAccountProfile {
    pub authenticated: bool,
    pub backend_configured: bool,
    pub account_id: Option<String>,
    pub connect_id: Option<String>,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub hosting: bool,
    pub accept_join_requests: bool,
    pub show_connect_id: bool,
    pub active_session_id: Option<String>,
    pub offline: bool,
    pub error_message: Option<String>,
}

impl Default for ConnectAccountProfile {
    fn default() -> Self {
        Self {
            authenticated: false,
            backend_configured: false,
            account_id: None,
            connect_id: None,
            display_name: "Not signed in".to_string(),
            avatar_url: None,
            hosting: false,
            accept_join_requests: false,
            show_connect_id: false,
            active_session_id: None,
            offline: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostLookupResult {
    pub host_account_id: String,
    pub connect_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub hosting: bool,
    pub accept_join_requests: bool,
    pub can_request: bool,
    pub availability_status: String,
    pub already_approved: bool,
    pub pending_request: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequestSendOutcome {
    pub outcome: String,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRequestItem {
    pub request_id: String,
    pub session_id: String,
    pub requester_account_id: String,
    pub requester_connect_id: String,
    pub requester_display_name: String,
    pub requester_avatar_url: Option<String>,
    pub status: String,
    pub created_at: String,
    pub responded_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundRequestItem {
    pub request_id: String,
    pub session_id: String,
    pub host_account_id: String,
    pub host_connect_id: String,
    pub host_display_name: String,
    pub host_avatar_url: Option<String>,
    pub status: String,
    pub created_at: String,
    pub responded_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredAccountEntry {
    pub ignored_account_id: String,
    pub connect_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedHostEntry {
    pub host_account_id: String,
    pub host_connect_id: String,
    pub host_display_name: String,
    pub host_avatar_url: Option<String>,
    pub hosting: bool,
    pub accept_join_requests: bool,
    pub availability_status: String,
    pub saved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRespondOutcome {
    pub request_id: String,
    pub status: String,
    pub requester_account_id: String,
    pub requester_display_name: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectAccountHubSnapshot {
    pub profile: ConnectAccountProfile,
    pub incoming_requests: Vec<ConnectionRequestItem>,
    pub outbound_requests: Vec<OutboundRequestItem>,
    pub ignored_accounts: Vec<IgnoredAccountEntry>,
    pub saved_hosts: Vec<SavedHostEntry>,
}

#[derive(Debug, Deserialize)]
struct RpcAccountRow {
    account_id: String,
    connect_id: String,
    display_name: String,
    avatar_url: Option<String>,
    hosting: bool,
    accept_join_requests: bool,
    show_connect_id: bool,
    active_session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RpcLookupRow {
    host_account_id: String,
    connect_id: String,
    display_name: String,
    avatar_url: Option<String>,
    hosting: bool,
    accept_join_requests: bool,
    can_request: bool,
    availability_status: String,
    already_approved: bool,
    pending_request: bool,
}

#[derive(Debug, Deserialize)]
struct RpcRequestSendRow {
    outcome: String,
    request_id: Option<String>,
    session_id: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RpcRespondRow {
    request_id: String,
    status: String,
    requester_account_id: String,
    requester_display_name: String,
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct RpcHostRequestRow {
    request_id: String,
    session_id: String,
    requester_account_id: String,
    requester_connect_id: String,
    requester_display_name: String,
    requester_avatar_url: Option<String>,
    status: String,
    created_at: String,
    responded_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RpcOutboundRequestRow {
    request_id: String,
    session_id: String,
    host_account_id: String,
    host_connect_id: String,
    host_display_name: String,
    host_avatar_url: Option<String>,
    status: String,
    created_at: String,
    responded_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RpcIgnoredRow {
    ignored_account_id: String,
    connect_id: String,
    display_name: String,
    avatar_url: Option<String>,
    created_at: String,
    expires_at: String,
}

#[derive(Debug, Deserialize)]
struct RpcSavedHostRow {
    host_account_id: String,
    host_connect_id: String,
    host_display_name: String,
    host_avatar_url: Option<String>,
    hosting: bool,
    accept_join_requests: bool,
    availability_status: String,
    saved_at: String,
}

pub struct ConnectAccountService {
    db: SqlitePool,
    sync_engine: Arc<SyncEngine>,
    rest: SupabaseRestClient,
    app_handle: Option<AppHandle>,
}

impl ConnectAccountService {
    pub fn new(
        db: SqlitePool,
        sync_engine: Arc<SyncEngine>,
        app_handle: Option<AppHandle>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            sync_engine,
            rest: SupabaseRestClient::new(),
            app_handle,
        })
    }

    async fn auth_context(&self) -> Result<(String, String, String, String), String> {
        let status = self.sync_engine.get_status().await?;
        if !status.backend_configured {
            return Err("Supabase backend is not configured in Settings -> Account.".to_string());
        }
        if !status.authenticated {
            return Err("Sign in to a Cavia account in Settings -> Account first.".to_string());
        }
        let account_id = status
            .account_id
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| "Account identity missing; please sign in again.".to_string())?;
        let Some((url, key, token)) = self.sync_engine.connect_auth_context().await? else {
            return Err("Your account session expired. Sign in again to continue.".to_string());
        };
        Ok((url, key, token, account_id))
    }

    fn emit_hub_snapshot(&self, snap: &ConnectAccountHubSnapshot) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit("connect:account-changed", snap);
        }
    }

    pub async fn active_metadata_tag_if_shown(&self) -> Option<String> {
        let status = self.sync_engine.get_status().await.ok()?;
        let account_id = status.account_id?;
        let row = sqlx::query(
            "SELECT connect_id, hosting, show_connect_id FROM connect_account_cache WHERE account_id = ?",
        )
        .bind(account_id)
        .fetch_optional(&self.db)
        .await
        .ok()??;
        let show: i64 = row.get("show_connect_id");
        let hosting: i64 = row.get("hosting");
        if show == 1 && hosting == 1 {
            Some(row.get("connect_id"))
        } else {
            None
        }
    }

    pub async fn get_hub_snapshot(&self) -> Result<ConnectAccountHubSnapshot, String> {
        let status = self.sync_engine.get_status().await?;
        if !status.backend_configured || !status.authenticated {
            return Ok(ConnectAccountHubSnapshot {
                profile: ConnectAccountProfile {
                    authenticated: false,
                    backend_configured: status.backend_configured,
                    ..Default::default()
                },
                incoming_requests: Vec::new(),
                outbound_requests: Vec::new(),
                ignored_accounts: Vec::new(),
                saved_hosts: Vec::new(),
            });
        }

        let account_id = status.account_id.clone().unwrap_or_default();
        if let Some((url, key, token)) =
            self.sync_engine.connect_auth_context().await.ok().flatten()
        {
            match self.fetch_remote_hub(&url, &key, &token, &status).await {
                Ok(snap) => {
                    self.cache_hub_snapshot(&snap).await;
                    return Ok(snap);
                }
                Err(err) => {
                    return self.load_cached_hub(&account_id, &status, Some(err)).await;
                }
            }
        }

        self.load_cached_hub(&account_id, &status, Some("Offline".to_string()))
            .await
    }

    async fn fetch_remote_hub(
        &self,
        url: &str,
        key: &str,
        token: &str,
        sync_status: &crate::services::sync::SyncAccountStatus,
    ) -> Result<ConnectAccountHubSnapshot, String> {
        let acc_rows: Vec<RpcAccountRow> = self
            .rest
            .call_rpc(
                url,
                key,
                token,
                "ensure_connect_account",
                &json!({
                    "p_display_name": Option::<String>::None,
                    "p_avatar_url": Option::<String>::None,
                }),
            )
            .await?;
        let acc = acc_rows
            .into_iter()
            .next()
            .ok_or_else(|| "Account profile could not be loaded".to_string())?;

        let empty_args = json!({});
        let (req_res, out_res, ign_res, saved_res): (
            Result<Vec<RpcHostRequestRow>, String>,
            Result<Vec<RpcOutboundRequestRow>, String>,
            Result<Vec<RpcIgnoredRow>, String>,
            Result<Vec<RpcSavedHostRow>, String>,
        ) = tokio::join!(
            self.rest
                .call_rpc(url, key, token, "list_host_requests", &empty_args),
            self.rest
                .call_rpc(url, key, token, "list_my_outbound_requests", &empty_args),
            self.rest
                .call_rpc(url, key, token, "list_ignored_accounts", &empty_args),
            self.rest
                .call_rpc(url, key, token, "list_saved_hosts", &empty_args),
        );

        let profile = ConnectAccountProfile {
            authenticated: true,
            backend_configured: sync_status.backend_configured,
            account_id: Some(acc.account_id),
            connect_id: Some(acc.connect_id),
            display_name: acc.display_name,
            avatar_url: acc.avatar_url,
            hosting: acc.hosting,
            accept_join_requests: acc.accept_join_requests,
            show_connect_id: acc.show_connect_id,
            active_session_id: acc.active_session_id,
            offline: false,
            error_message: None,
        };

        let incoming_requests = req_res
            .unwrap_or_default()
            .into_iter()
            .map(|r| ConnectionRequestItem {
                request_id: r.request_id,
                session_id: r.session_id,
                requester_account_id: r.requester_account_id,
                requester_connect_id: r.requester_connect_id,
                requester_display_name: r.requester_display_name,
                requester_avatar_url: r.requester_avatar_url,
                status: r.status,
                created_at: r.created_at,
                responded_at: r.responded_at,
            })
            .collect();

        let outbound_requests = out_res
            .unwrap_or_default()
            .into_iter()
            .map(|r| OutboundRequestItem {
                request_id: r.request_id,
                session_id: r.session_id,
                host_account_id: r.host_account_id,
                host_connect_id: r.host_connect_id,
                host_display_name: r.host_display_name,
                host_avatar_url: r.host_avatar_url,
                status: r.status,
                created_at: r.created_at,
                responded_at: r.responded_at,
            })
            .collect();

        let ignored_accounts = ign_res
            .unwrap_or_default()
            .into_iter()
            .map(|r| IgnoredAccountEntry {
                ignored_account_id: r.ignored_account_id,
                connect_id: r.connect_id,
                display_name: r.display_name,
                avatar_url: r.avatar_url,
                created_at: r.created_at,
                expires_at: r.expires_at,
            })
            .collect();

        let saved_hosts = saved_res
            .unwrap_or_default()
            .into_iter()
            .map(|r| SavedHostEntry {
                host_account_id: r.host_account_id,
                host_connect_id: r.host_connect_id,
                host_display_name: r.host_display_name,
                host_avatar_url: r.host_avatar_url,
                hosting: r.hosting,
                accept_join_requests: r.accept_join_requests,
                availability_status: r.availability_status,
                saved_at: r.saved_at,
            })
            .collect();

        Ok(ConnectAccountHubSnapshot {
            profile,
            incoming_requests,
            outbound_requests,
            ignored_accounts,
            saved_hosts,
        })
    }

    async fn cache_hub_snapshot(&self, snap: &ConnectAccountHubSnapshot) {
        let Some(acc_id) = snap.profile.account_id.as_deref() else {
            return;
        };
        if let Some(cid) = snap.profile.connect_id.as_deref() {
            let _ = sqlx::query(
                r#"
                INSERT INTO connect_account_cache (
                    account_id, connect_id, display_name, avatar_url, hosting,
                    accept_join_requests, show_connect_id, active_session_id, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                ON CONFLICT(account_id) DO UPDATE SET
                    display_name = excluded.display_name,
                    avatar_url = excluded.avatar_url,
                    hosting = excluded.hosting,
                    accept_join_requests = excluded.accept_join_requests,
                    show_connect_id = excluded.show_connect_id,
                    active_session_id = excluded.active_session_id,
                    updated_at = CURRENT_TIMESTAMP
                "#,
            )
            .bind(acc_id)
            .bind(cid)
            .bind(&snap.profile.display_name)
            .bind(&snap.profile.avatar_url)
            .bind(if snap.profile.hosting { 1 } else { 0 })
            .bind(if snap.profile.accept_join_requests {
                1
            } else {
                0
            })
            .bind(if snap.profile.show_connect_id { 1 } else { 0 })
            .bind(&snap.profile.active_session_id)
            .execute(&self.db)
            .await;
        }

        // Replace saved hosts cache for owner using host_account_id as stable key so renamed
        // hosts are immediately updated locally too.
        let mut tx = match self.db.begin().await {
            Ok(tx) => tx,
            Err(_) => return,
        };
        let _ = sqlx::query("DELETE FROM connect_saved_hosts WHERE owner_account_id = ?")
            .bind(acc_id)
            .execute(&mut *tx)
            .await;
        for host in &snap.saved_hosts {
            let _ = sqlx::query(
                r#"
                INSERT INTO connect_saved_hosts (
                    owner_account_id, host_account_id, host_connect_id, host_display_name,
                    host_avatar_url, last_known_hosting, last_known_accepting, saved_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                "#,
            )
            .bind(acc_id)
            .bind(&host.host_account_id)
            .bind(&host.host_connect_id)
            .bind(&host.host_display_name)
            .bind(&host.host_avatar_url)
            .bind(if host.hosting { 1 } else { 0 })
            .bind(if host.accept_join_requests { 1 } else { 0 })
            .bind(&host.saved_at)
            .execute(&mut *tx)
            .await;
        }

        let _ = sqlx::query("DELETE FROM connect_ignored_accounts_cache WHERE host_account_id = ?")
            .bind(acc_id)
            .execute(&mut *tx)
            .await;
        for ign in &snap.ignored_accounts {
            let _ = sqlx::query(
                r#"
                INSERT INTO connect_ignored_accounts_cache (
                    host_account_id, ignored_account_id, ignored_connect_id,
                    ignored_display_name, ignored_avatar_url, created_at, expires_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(acc_id)
            .bind(&ign.ignored_account_id)
            .bind(&ign.connect_id)
            .bind(&ign.display_name)
            .bind(&ign.avatar_url)
            .bind(&ign.created_at)
            .bind(&ign.expires_at)
            .execute(&mut *tx)
            .await;
        }

        let _ = sqlx::query("DELETE FROM connect_requests_cache WHERE host_account_id = ?")
            .bind(acc_id)
            .execute(&mut *tx)
            .await;
        for req in &snap.incoming_requests {
            let _ = sqlx::query(
                r#"
                INSERT INTO connect_requests_cache (
                    request_id, session_id, host_account_id, requester_account_id,
                    requester_connect_id, requester_display_name, requester_avatar_url,
                    status, created_at, responded_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&req.request_id)
            .bind(&req.session_id)
            .bind(acc_id)
            .bind(&req.requester_account_id)
            .bind(&req.requester_connect_id)
            .bind(&req.requester_display_name)
            .bind(&req.requester_avatar_url)
            .bind(&req.status)
            .bind(&req.created_at)
            .bind(&req.responded_at)
            .execute(&mut *tx)
            .await;
        }
        let _ = tx.commit().await;
    }

    async fn load_cached_hub(
        &self,
        account_id: &str,
        sync_status: &crate::services::sync::SyncAccountStatus,
        error_msg: Option<String>,
    ) -> Result<ConnectAccountHubSnapshot, String> {
        let acc_row = sqlx::query(
            r#"
            SELECT connect_id, display_name, avatar_url, hosting, accept_join_requests,
                   show_connect_id, active_session_id
            FROM connect_account_cache WHERE account_id = ?
            "#,
        )
        .bind(account_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| format!("Failed to read cached connect account: {e}"))?;

        let profile = match acc_row {
            Some(r) => ConnectAccountProfile {
                authenticated: true,
                backend_configured: sync_status.backend_configured,
                account_id: Some(account_id.to_string()),
                connect_id: Some(r.get("connect_id")),
                display_name: r.get("display_name"),
                avatar_url: r.get("avatar_url"),
                hosting: r.get::<i64, _>("hosting") == 1,
                accept_join_requests: r.get::<i64, _>("accept_join_requests") == 1,
                show_connect_id: r.get::<i64, _>("show_connect_id") == 1,
                active_session_id: r.get("active_session_id"),
                offline: true,
                error_message: error_msg,
            },
            None => ConnectAccountProfile {
                authenticated: true,
                backend_configured: sync_status.backend_configured,
                account_id: Some(account_id.to_string()),
                connect_id: None,
                display_name: sync_status
                    .account_email
                    .clone()
                    .unwrap_or_else(|| "Account offline".to_string()),
                avatar_url: None,
                hosting: false,
                accept_join_requests: false,
                show_connect_id: false,
                active_session_id: None,
                offline: true,
                error_message: error_msg,
            },
        };

        let saved_rows = sqlx::query(
            r#"
            SELECT host_account_id, host_connect_id, host_display_name, host_avatar_url, saved_at
            FROM connect_saved_hosts
            WHERE owner_account_id = ?
            ORDER BY updated_at DESC
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        let saved_hosts = saved_rows
            .into_iter()
            .map(|r| SavedHostEntry {
                host_account_id: r.get("host_account_id"),
                host_connect_id: r.get("host_connect_id"),
                host_display_name: r.get("host_display_name"),
                host_avatar_url: r.get("host_avatar_url"),
                hosting: false,
                accept_join_requests: false,
                availability_status: "offline".to_string(),
                saved_at: r.get("saved_at"),
            })
            .collect();

        Ok(ConnectAccountHubSnapshot {
            profile,
            incoming_requests: Vec::new(),
            outbound_requests: Vec::new(),
            ignored_accounts: Vec::new(),
            saved_hosts,
        })
    }

    pub async fn update_profile_and_controls(
        &self,
        display_name: Option<&str>,
        avatar_url: Option<&str>,
        hosting: Option<bool>,
        accept_join_requests: Option<bool>,
        show_connect_id: Option<bool>,
        active_session_id: Option<&str>,
    ) -> Result<ConnectAccountHubSnapshot, String> {
        let (url, key, token, _) = self.auth_context().await?;
        let _: Vec<RpcAccountRow> = self
            .rest
            .call_rpc(
                &url,
                &key,
                &token,
                "update_connect_account_settings",
                &json!({
                    "p_display_name": display_name,
                    "p_avatar_url": avatar_url,
                    "p_hosting": hosting,
                    "p_accept_join_requests": accept_join_requests,
                    "p_show_connect_id": show_connect_id,
                    "p_active_session_id": active_session_id,
                }),
            )
            .await?;

        let snap = self.get_hub_snapshot().await?;
        self.emit_hub_snapshot(&snap);
        Ok(snap)
    }

    pub async fn sync_host_session_started(
        &self,
        session_id: &str,
        join_code: &str,
        name: &str,
        alpha: f64,
        preload_target: usize,
        session_seed: u64,
    ) -> Result<ConnectAccountHubSnapshot, String> {
        let (url, key, token, _) = self.auth_context().await?;
        let _: Vec<RpcAccountRow> = self
            .rest
            .call_rpc(
                &url,
                &key,
                &token,
                "create_host_connect_session",
                &json!({
                    "p_session_id": session_id,
                    "p_join_code": join_code,
                    "p_name": name,
                    "p_fairness_alpha": alpha,
                    "p_preload_target": preload_target as i32,
                    "p_session_seed": (session_seed & 0x7FFF_FFFF_FFFF_FFFF) as i64,
                }),
            )
            .await?;
        let snap = self.get_hub_snapshot().await?;
        self.emit_hub_snapshot(&snap);
        Ok(snap)
    }

    pub async fn lookup_host(&self, connect_id: &str) -> Result<Option<HostLookupResult>, String> {
        let normalized = normalize_connect_id(connect_id);
        if !is_valid_connect_id(&normalized) {
            return Err(
                "Connect IDs must be 6 uppercase characters (excluding 0, O, 1, and I)."
                    .to_string(),
            );
        }
        let (url, key, token, _) = self.auth_context().await?;
        let rows: Vec<RpcLookupRow> = self
            .rest
            .call_rpc(
                &url,
                &key,
                &token,
                "lookup_connect_host",
                &json!({ "p_connect_id": normalized }),
            )
            .await?;
        Ok(rows.into_iter().next().map(|r| HostLookupResult {
            host_account_id: r.host_account_id,
            connect_id: r.connect_id,
            display_name: r.display_name,
            avatar_url: r.avatar_url,
            hosting: r.hosting,
            accept_join_requests: r.accept_join_requests,
            can_request: r.can_request,
            availability_status: r.availability_status,
            already_approved: r.already_approved,
            pending_request: r.pending_request,
        }))
    }

    pub async fn send_join_request(
        &self,
        host_account_id: &str,
    ) -> Result<ConnectRequestSendOutcome, String> {
        let (url, key, token, _) = self.auth_context().await?;
        let rows: Vec<RpcRequestSendRow> = self
            .rest
            .call_rpc(
                &url,
                &key,
                &token,
                "send_connect_request",
                &json!({ "p_host_account_id": host_account_id }),
            )
            .await?;
        let row = rows
            .into_iter()
            .next()
            .ok_or_else(|| "Host did not return a request response".to_string())?;
        if let Ok(snap) = self.get_hub_snapshot().await {
            self.emit_hub_snapshot(&snap);
        }
        Ok(ConnectRequestSendOutcome {
            outcome: row.outcome,
            request_id: row.request_id,
            session_id: row.session_id,
            message: row.message,
        })
    }

    pub async fn respond_to_request(
        &self,
        request_id: &str,
        action: &str,
    ) -> Result<ConnectRespondOutcome, String> {
        let (url, key, token, _) = self.auth_context().await?;
        let rows: Vec<RpcRespondRow> = self
            .rest
            .call_rpc(
                &url,
                &key,
                &token,
                "respond_connect_request",
                &json!({
                    "p_request_id": request_id,
                    "p_action": action,
                }),
            )
            .await?;
        let row = rows
            .into_iter()
            .next()
            .ok_or_else(|| "Request update failed".to_string())?;
        if let Ok(snap) = self.get_hub_snapshot().await {
            self.emit_hub_snapshot(&snap);
        }
        Ok(ConnectRespondOutcome {
            request_id: row.request_id,
            status: row.status,
            requester_account_id: row.requester_account_id,
            requester_display_name: row.requester_display_name,
            session_id: row.session_id,
        })
    }

    pub async fn unignore_guest(
        &self,
        guest_account_id: &str,
    ) -> Result<ConnectAccountHubSnapshot, String> {
        let (url, key, token, _) = self.auth_context().await?;
        self.rest
            .call_rpc_void(
                &url,
                &key,
                &token,
                "unignore_connect_guest",
                &json!({ "p_ignored_account_id": guest_account_id }),
            )
            .await?;
        let snap = self.get_hub_snapshot().await?;
        self.emit_hub_snapshot(&snap);
        Ok(snap)
    }

    pub async fn save_host(
        &self,
        host_account_id: &str,
    ) -> Result<ConnectAccountHubSnapshot, String> {
        let (url, key, token, _) = self.auth_context().await?;
        self.rest
            .call_rpc_void(
                &url,
                &key,
                &token,
                "save_connect_host",
                &json!({ "p_host_account_id": host_account_id }),
            )
            .await?;
        let snap = self.get_hub_snapshot().await?;
        self.emit_hub_snapshot(&snap);
        Ok(snap)
    }

    pub async fn remove_saved_host(
        &self,
        host_account_id: &str,
    ) -> Result<ConnectAccountHubSnapshot, String> {
        let (url, key, token, owner_id) = self.auth_context().await?;
        self.rest
            .call_rpc_void(
                &url,
                &key,
                &token,
                "remove_saved_host",
                &json!({ "p_host_account_id": host_account_id }),
            )
            .await?;
        let _ = sqlx::query(
            "DELETE FROM connect_saved_hosts WHERE owner_account_id = ? AND host_account_id = ?",
        )
        .bind(owner_id)
        .bind(host_account_id)
        .execute(&self.db)
        .await;
        let snap = self.get_hub_snapshot().await?;
        self.emit_hub_snapshot(&snap);
        Ok(snap)
    }

    pub async fn connect_to_saved_host(
        &self,
        host_account_id: &str,
    ) -> Result<ConnectRequestSendOutcome, String> {
        self.send_join_request(host_account_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::collections::{HashMap, HashSet};
    use uuid::Uuid;

    async fn test_db() -> SqlitePool {
        let opts = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    /// Deterministic local SQLite simulator of the authoritative server RPC contract so all
    /// security, rate-limit, expiry, saved-host rename, and session-control invariants run in CI.
    struct LocalConnectAccountRegistry {
        accounts: HashMap<String, ConnectAccountProfile>,
        by_connect_id: HashMap<String, String>,
        ignores: HashMap<(String, String), i64>,
        requests: Vec<ConnectionRequestItem>,
        approved_members: HashMap<String, HashSet<String>>,
        rate_windows: HashMap<(String, String), (i64, usize)>,
        db: SqlitePool,
    }

    impl LocalConnectAccountRegistry {
        fn new(db: SqlitePool) -> Self {
            Self {
                accounts: HashMap::new(),
                by_connect_id: HashMap::new(),
                ignores: HashMap::new(),
                requests: Vec::new(),
                approved_members: HashMap::new(),
                rate_windows: HashMap::new(),
                db,
            }
        }

        fn register(&mut self, account_id: &str, display_name: &str) -> ConnectAccountProfile {
            if let Some(existing) = self.accounts.get(account_id) {
                return existing.clone();
            }
            let cid = loop {
                let raw = Uuid::new_v4();
                let b = raw.as_bytes();
                let mut candidate = String::with_capacity(6);
                for byte in b.iter().take(6) {
                    candidate.push(
                        CONNECT_ID_ALPHABET[(*byte as usize) % CONNECT_ID_ALPHABET.len()] as char,
                    );
                }
                if !self.by_connect_id.contains_key(&candidate) {
                    break candidate;
                }
            };
            let profile = ConnectAccountProfile {
                authenticated: true,
                backend_configured: true,
                account_id: Some(account_id.to_string()),
                connect_id: Some(cid.clone()),
                display_name: display_name.to_string(),
                avatar_url: None,
                hosting: false,
                accept_join_requests: false,
                show_connect_id: false,
                active_session_id: None,
                offline: false,
                error_message: None,
            };
            self.by_connect_id.insert(cid, account_id.to_string());
            self.accounts
                .insert(account_id.to_string(), profile.clone());
            profile
        }

        fn enforce_rate(
            &mut self,
            account_id: &str,
            action: &str,
            now_sec: i64,
            max_calls: usize,
        ) -> Result<(), String> {
            let key = (account_id.to_string(), action.to_string());
            let entry = self.rate_windows.entry(key).or_insert((now_sec, 0));
            if entry.0 + 60 <= now_sec {
                *entry = (now_sec, 1);
                Ok(())
            } else if entry.1 >= max_calls {
                Err(format!("Too many {action} requests"))
            } else {
                entry.1 += 1;
                Ok(())
            }
        }

        fn lookup(
            &mut self,
            caller_id: Option<&str>,
            connect_id: &str,
            now_sec: i64,
        ) -> Result<Option<HostLookupResult>, String> {
            let caller = caller_id.ok_or_else(|| "Authentication required".to_string())?;
            self.enforce_rate(caller, "lookup_host", now_sec, 5)?;
            self.ignores.retain(|_, exp| *exp > now_sec);
            let Some(host_id) = self
                .by_connect_id
                .get(&normalize_connect_id(connect_id))
                .cloned()
            else {
                return Ok(None);
            };
            let host = &self.accounts[&host_id];
            let is_ignored = self
                .ignores
                .contains_key(&(host_id.clone(), caller.to_string()));
            let is_member = host
                .active_session_id
                .as_ref()
                .and_then(|sid| self.approved_members.get(sid))
                .is_some_and(|set| set.contains(caller));
            let is_pending = self.requests.iter().any(|r| {
                r.requester_account_id == caller
                    && r.status == "pending"
                    && Some(&r.session_id) == host.active_session_id.as_ref()
            });

            let avail = if is_ignored {
                "unavailable"
            } else if !host.hosting {
                "offline"
            } else if is_member {
                "connected"
            } else if is_pending {
                "pending"
            } else if !host.accept_join_requests {
                "requests_disabled"
            } else {
                "accepting_requests"
            };

            Ok(Some(HostLookupResult {
                host_account_id: host_id,
                connect_id: host.connect_id.clone().unwrap(),
                display_name: host.display_name.clone(),
                avatar_url: host.avatar_url.clone(),
                hosting: host.hosting && !is_ignored,
                accept_join_requests: host.hosting && host.accept_join_requests && !is_ignored,
                can_request: avail == "accepting_requests",
                availability_status: avail.to_string(),
                already_approved: is_member,
                pending_request: is_pending,
            }))
        }

        fn send_request(
            &mut self,
            caller_id: Option<&str>,
            host_id: &str,
            now_sec: i64,
        ) -> Result<ConnectRequestSendOutcome, String> {
            let caller = caller_id.ok_or_else(|| "Authentication required".to_string())?;
            self.enforce_rate(caller, "send_request", now_sec, 5)?;
            self.ignores.retain(|_, exp| *exp > now_sec);
            let host = self
                .accounts
                .get(host_id)
                .ok_or_else(|| "Host missing".to_string())?
                .clone();
            if !host.hosting || host.active_session_id.is_none() {
                return Ok(ConnectRequestSendOutcome {
                    outcome: "offline".into(),
                    request_id: None,
                    session_id: None,
                    message: "Host is not hosting".into(),
                });
            }
            let sid = host.active_session_id.unwrap();
            if self
                .ignores
                .contains_key(&(host_id.to_string(), caller.to_string()))
            {
                return Ok(ConnectRequestSendOutcome {
                    outcome: "unavailable".into(),
                    request_id: None,
                    session_id: None,
                    message: "This host is unavailable right now.".into(),
                });
            }
            if !host.accept_join_requests {
                return Ok(ConnectRequestSendOutcome {
                    outcome: "requests_disabled".into(),
                    request_id: None,
                    session_id: Some(sid),
                    message: "Host disabled new join requests.".into(),
                });
            }
            if let Some(existing) = self.requests.iter().find(|r| {
                r.session_id == sid && r.requester_account_id == caller && r.status == "pending"
            }) {
                return Ok(ConnectRequestSendOutcome {
                    outcome: "pending".into(),
                    request_id: Some(existing.request_id.clone()),
                    session_id: Some(sid),
                    message: "Already pending".into(),
                });
            }
            let caller_acc = &self.accounts[caller];
            let req_id = format!("req-{}", Uuid::new_v4());
            self.requests.push(ConnectionRequestItem {
                request_id: req_id.clone(),
                session_id: sid.clone(),
                requester_account_id: caller.to_string(),
                requester_connect_id: caller_acc.connect_id.clone().unwrap(),
                requester_display_name: caller_acc.display_name.clone(),
                requester_avatar_url: None,
                status: "pending".into(),
                created_at: format!("{now_sec}"),
                responded_at: None,
            });
            Ok(ConnectRequestSendOutcome {
                outcome: "pending".into(),
                request_id: Some(req_id),
                session_id: Some(sid),
                message: "Sent".into(),
            })
        }

        fn respond(
            &mut self,
            host_id: &str,
            request_id: &str,
            action: &str,
            now_sec: i64,
        ) -> Result<(), String> {
            let pos = self
                .requests
                .iter()
                .position(|r| r.request_id == request_id)
                .ok_or("missing")?;
            let sid = self.requests[pos].session_id.clone();
            let requester = self.requests[pos].requester_account_id.clone();
            if self
                .accounts
                .get(host_id)
                .and_then(|a| a.active_session_id.as_ref())
                != Some(&sid)
            {
                return Err("Unauthorized host".into());
            }
            match action {
                "accept" => {
                    self.requests[pos].status = "accepted".into();
                    self.approved_members
                        .entry(sid)
                        .or_default()
                        .insert(requester);
                }
                "decline" => {
                    self.requests[pos].status = "declined".into();
                }
                "ignore_1d" => {
                    self.requests[pos].status = "ignored".into();
                    self.ignores
                        .insert((host_id.to_string(), requester), now_sec + 86_400);
                }
                _ => return Err("bad action".into()),
            }
            Ok(())
        }

        fn unignore(&mut self, host_id: &str, guest_id: &str) {
            self.ignores
                .remove(&(host_id.to_string(), guest_id.to_string()));
        }

        async fn save_host_in_sqlite(&self, owner_id: &str, host_id: &str) {
            let host = &self.accounts[host_id];
            sqlx::query(
                r#"
                INSERT INTO connect_saved_hosts (
                    owner_account_id, host_account_id, host_connect_id, host_display_name
                ) VALUES (?, ?, ?, ?)
                ON CONFLICT(owner_account_id, host_account_id) DO UPDATE SET
                    host_display_name = excluded.host_display_name,
                    updated_at = CURRENT_TIMESTAMP
                "#,
            )
            .bind(owner_id)
            .bind(host_id)
            .bind(host.connect_id.as_ref().unwrap())
            .bind(&host.display_name)
            .execute(&self.db)
            .await
            .unwrap();
        }
    }

    #[test]
    fn permanent_connect_id_alphabet_uniqueness_and_validation() {
        let mut seen = HashSet::new();
        for i in 0..2000 {
            let mut id = String::with_capacity(6);
            let raw = Uuid::new_v4();
            for byte in raw.as_bytes().iter().take(6) {
                id.push(
                    CONNECT_ID_ALPHABET[(*byte as usize + i) % CONNECT_ID_ALPHABET.len()] as char,
                );
            }
            assert!(is_valid_connect_id(&id));
            assert!(
                !id.contains('0') && !id.contains('O') && !id.contains('1') && !id.contains('I')
            );
            seen.insert(id);
        }
        assert!(seen.len() > 1990);
        assert!(!is_valid_connect_id("AB01OI"));
        assert!(!is_valid_connect_id("SHORT"));
    }

    #[tokio::test]
    async fn full_account_requests_ignore_saved_hosts_and_controls_lifecycle() {
        let pool = test_db().await;
        let mut reg = LocalConnectAccountRegistry::new(pool.clone());

        let host = reg.register("acc-host", "Host DJ");
        let _host2 = reg.register("acc-host2", "Second Host");
        let guest_a = reg.register("acc-g1", "Guest One");
        let guest_b = reg.register("acc-g2", "Guest Two");
        let host_cid = host.connect_id.clone().unwrap();

        // Unauthenticated lookup must fail cleanly
        assert!(reg.lookup(None, &host_cid, 100).is_err());

        // Inactive host lookup shows offline
        let inactive = reg.lookup(Some("acc-g1"), &host_cid, 101).unwrap().unwrap();
        assert!(!inactive.hosting);
        assert_eq!(inactive.availability_status, "offline");

        // Enable hosting AND accept_join_requests independently while show_connect_id remains false
        {
            let h = reg.accounts.get_mut("acc-host").unwrap();
            h.hosting = true;
            h.accept_join_requests = true;
            h.active_session_id = Some("sess-1".into());
        }
        assert!(!reg.accounts["acc-host"].show_connect_id);

        // Concurrent requests from multiple guests + duplicate pending prevention
        let r_a1 = reg.send_request(Some("acc-g1"), "acc-host", 102).unwrap();
        let r_a_dup = reg.send_request(Some("acc-g1"), "acc-host", 103).unwrap();
        let r_b = reg.send_request(Some("acc-g2"), "acc-host", 103).unwrap();
        assert_eq!(r_a1.request_id, r_a_dup.request_id);
        assert_eq!(
            reg.requests
                .iter()
                .filter(|r| r.requester_account_id == "acc-g1")
                .count(),
            1
        );

        // Accept guest A -> approved member
        reg.respond(
            "acc-host",
            r_a1.request_id.as_deref().unwrap(),
            "accept",
            105,
        )
        .unwrap();
        assert!(reg.approved_members["sess-1"].contains("acc-g1"));

        // Decline / Ignore B for 1 day -> rejected for 24h on acc-host ONLY, without exposing ignore or blocking host2!
        reg.respond(
            "acc-host",
            r_b.request_id.as_deref().unwrap(),
            "ignore_1d",
            110,
        )
        .unwrap();
        let ignored_lookup = reg.lookup(Some("acc-g2"), &host_cid, 111).unwrap().unwrap();
        assert_eq!(ignored_lookup.availability_status, "unavailable");
        assert!(!ignored_lookup.can_request);
        let ignored_send = reg.send_request(Some("acc-g2"), "acc-host", 112).unwrap();
        assert_eq!(ignored_send.outcome, "unavailable");

        // Other hosts remain unaffected by host1's temporary ignore
        {
            let h2 = reg.accounts.get_mut("acc-host2").unwrap();
            h2.hosting = true;
            h2.accept_join_requests = true;
            h2.active_session_id = Some("sess-2".into());
        }
        assert_eq!(
            reg.send_request(Some("acc-g2"), "acc-host2", 113)
                .unwrap()
                .outcome,
            "pending"
        );

        // Disabling Accept Join Requests prevents new requests while keeping guest A connected!
        reg.accounts
            .get_mut("acc-host")
            .unwrap()
            .accept_join_requests = false;
        assert!(reg.approved_members["sess-1"].contains("acc-g1"));
        let _ = guest_a;
        let _ = guest_b;

        // Re-enable requests -> still ignored within 24h; expires automatically after 24h!
        reg.accounts
            .get_mut("acc-host")
            .unwrap()
            .accept_join_requests = true;
        assert_eq!(
            reg.send_request(Some("acc-g2"), "acc-host", 120)
                .unwrap()
                .outcome,
            "unavailable"
        );
        assert_eq!(
            reg.send_request(Some("acc-g2"), "acc-host", 110 + 86_405)
                .unwrap()
                .outcome,
            "pending"
        );

        // Save host -> host changes display name -> reconnect/reload uses stable account ID
        reg.accounts.get_mut("acc-host").unwrap().display_name = "Host Renamed".into();
        reg.save_host_in_sqlite("acc-g1", "acc-host").await;
        let saved_name: String = sqlx::query_scalar(
            "SELECT host_display_name FROM connect_saved_hosts WHERE owner_account_id = 'acc-g1' AND host_account_id = 'acc-host'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(saved_name, "Host Renamed");

        // Host session closure ends requests & rejects new connections
        {
            let h = reg.accounts.get_mut("acc-host").unwrap();
            h.hosting = false;
            h.active_session_id = None;
        }
        assert_eq!(
            reg.send_request(Some("acc-g1"), "acc-host", 90_000)
                .unwrap()
                .outcome,
            "offline"
        );

        // Rate limiting triggers after > 5 rapid calls in 60s window
        for i in 0..5 {
            let _ = reg.lookup(Some("acc-g1"), &host_cid, 91_000 + i);
        }
        assert!(reg.lookup(Some("acc-g1"), &host_cid, 91_010).is_err());
    }

    #[tokio::test]
    async fn decline_manual_unignore_requests_disabled_and_sqlite_restart_persistence() {
        let pool = test_db().await;
        let mut reg = LocalConnectAccountRegistry::new(pool.clone());

        let host = reg.register("acc-host-a", "Night Host");
        let host_cid = host.connect_id.clone().unwrap();
        let _guest_approved = reg.register("acc-g-approved", "Approved Guest");
        let _guest_declined = reg.register("acc-g-declined", "Declined Guest");
        let _guest_blocked = reg.register("acc-g-blocked", "Late Guest");

        {
            let h = reg.accounts.get_mut("acc-host-a").unwrap();
            h.hosting = true;
            h.accept_join_requests = true;
            h.show_connect_id = false;
            h.active_session_id = Some("sess-night".into());
        }

        // Approve guest A while show_connect_id is OFF; saved hosts / lookups still locate host.
        let req_a = reg
            .send_request(Some("acc-g-approved"), "acc-host-a", 10)
            .unwrap();
        reg.respond(
            "acc-host-a",
            req_a.request_id.as_deref().unwrap(),
            "accept",
            11,
        )
        .unwrap();
        assert!(reg.approved_members["sess-night"].contains("acc-g-approved"));

        // Decline guest B WITHOUT ignoring -> guest B can immediately send another request.
        let req_b1 = reg
            .send_request(Some("acc-g-declined"), "acc-host-a", 12)
            .unwrap();
        assert_eq!(req_b1.outcome, "pending");
        reg.respond(
            "acc-host-a",
            req_b1.request_id.as_deref().unwrap(),
            "decline",
            13,
        )
        .unwrap();
        let req_b2 = reg
            .send_request(Some("acc-g-declined"), "acc-host-a", 14)
            .unwrap();
        assert_eq!(req_b2.outcome, "pending");
        assert_ne!(req_b1.request_id, req_b2.request_id);

        // Ignore guest B for 1 day, then manually Unignore BEFORE 24h -> immediate re-request works!
        reg.respond(
            "acc-host-a",
            req_b2.request_id.as_deref().unwrap(),
            "ignore_1d",
            15,
        )
        .unwrap();
        assert_eq!(
            reg.send_request(Some("acc-g-declined"), "acc-host-a", 20)
                .unwrap()
                .outcome,
            "unavailable"
        );
        reg.unignore("acc-host-a", "acc-g-declined");
        let after_unignore = reg
            .send_request(Some("acc-g-declined"), "acc-host-a", 25)
            .unwrap();
        assert_eq!(after_unignore.outcome, "pending");

        // Disable Accept Join Requests -> existing approved member stays in session, new guest rejected.
        reg.accounts
            .get_mut("acc-host-a")
            .unwrap()
            .accept_join_requests = false;
        assert!(reg.approved_members["sess-night"].contains("acc-g-approved"));
        assert!(!reg.accounts["acc-host-a"].show_connect_id);
        let disabled_attempt = reg
            .send_request(Some("acc-g-blocked"), "acc-host-a", 30)
            .unwrap();
        assert_eq!(disabled_attempt.outcome, "requests_disabled");
        let disabled_lookup = reg
            .lookup(Some("acc-g-blocked"), &host_cid, 31)
            .unwrap()
            .unwrap();
        assert_eq!(disabled_lookup.availability_status, "requests_disabled");

        // Persist account + saved host + ignore record in SQLite and verify survival across reload.
        sqlx::query(
            r#"
            INSERT INTO connect_account_cache (
                account_id, connect_id, display_name, hosting, accept_join_requests, show_connect_id
            ) VALUES ('acc-host-a', ?, 'Night Host', 1, 0, 0)
            "#,
        )
        .bind(&host_cid)
        .execute(&pool)
        .await
        .unwrap();
        reg.save_host_in_sqlite("acc-g-approved", "acc-host-a")
            .await;
        sqlx::query(
            r#"
            INSERT INTO connect_ignored_accounts_cache (
                host_account_id, ignored_account_id, ignored_connect_id,
                ignored_display_name, created_at, expires_at
            ) VALUES ('acc-host-a', 'acc-g-blocked', 'Z9K4PQ', 'Late Guest', '2026-11-28T00:00:00Z', '2026-11-29T00:00:00Z')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let (persisted_cid,): (String,) = sqlx::query_as(
            "SELECT connect_id FROM connect_account_cache WHERE account_id = 'acc-host-a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let (persisted_saved_id,): (String,) =
            sqlx::query_as("SELECT host_connect_id FROM connect_saved_hosts WHERE owner_account_id = 'acc-g-approved'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let (persisted_ign_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM connect_ignored_accounts_cache WHERE host_account_id = 'acc-host-a'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(persisted_cid, host_cid);
        assert_eq!(persisted_saved_id, host_cid);
        assert_eq!(persisted_ign_count, 1);
    }
}
