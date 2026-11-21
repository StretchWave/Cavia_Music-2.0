use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseUser {
    pub id: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: String,
    pub user: SupabaseUser,
}

#[derive(Clone)]
pub struct SupabaseAuthClient {
    client: Client,
    cached_access_token: Arc<RwLock<Option<String>>>,
    cached_expires_at: Arc<RwLock<Option<i64>>>,
}

impl Default for SupabaseAuthClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SupabaseAuthClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            cached_access_token: Arc::new(RwLock::new(None)),
            cached_expires_at: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_cached_session(&self, access_token: &str, expires_in_secs: i64) {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut token_guard = self.cached_access_token.write().await;
        *token_guard = Some(access_token.to_string());

        let mut exp_guard = self.cached_expires_at.write().await;
        // Expire slightly early (buffer of 60s) to avoid edge-of-expiry races
        *exp_guard = Some(now_secs + (expires_in_secs - 60).max(10));
    }

    pub async fn clear_cached_session(&self) {
        let mut token_guard = self.cached_access_token.write().await;
        *token_guard = None;

        let mut exp_guard = self.cached_expires_at.write().await;
        *exp_guard = None;
    }

    pub async fn get_valid_cached_token(&self) -> Option<String> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let exp_guard = self.cached_expires_at.read().await;
        if let Some(exp) = *exp_guard {
            if now_secs < exp {
                let token_guard = self.cached_access_token.read().await;
                return token_guard.clone();
            }
        }
        None
    }

    pub async fn signup(
        &self,
        supabase_url: &str,
        anon_key: &str,
        email: &str,
        password: &str,
    ) -> Result<AuthResponse, String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!("{clean_url}/auth/v1/signup");

        let body = serde_json::json!({
            "email": email,
            "password": password
        });

        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network error reaching the account service: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let error_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Unknown auth error".to_string());
            return Err(format!("Signup failed ({status}): {error_text}"));
        }

        let auth_resp: AuthResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse auth signup response: {e}"))?;

        if let Some(exp) = auth_resp.expires_in {
            self.set_cached_session(&auth_resp.access_token, exp).await;
        }

        Ok(auth_resp)
    }

    pub async fn login(
        &self,
        supabase_url: &str,
        anon_key: &str,
        email: &str,
        password: &str,
    ) -> Result<AuthResponse, String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!("{clean_url}/auth/v1/token?grant_type=password");

        let body = serde_json::json!({
            "email": email,
            "password": password
        });

        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network error reaching the account service: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let error_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Invalid credentials".to_string());
            return Err(format!("Login failed ({status}): {error_text}"));
        }

        let auth_resp: AuthResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse auth login response: {e}"))?;

        let exp = auth_resp.expires_in.unwrap_or(3600);
        self.set_cached_session(&auth_resp.access_token, exp).await;

        Ok(auth_resp)
    }

    pub async fn refresh(
        &self,
        supabase_url: &str,
        anon_key: &str,
        refresh_token: &str,
    ) -> Result<AuthResponse, String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!("{clean_url}/auth/v1/token?grant_type=refresh_token");

        let body = serde_json::json!({
            "refresh_token": refresh_token
        });

        let resp = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network error reaching the account service: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            return Err("Your session has expired. Please sign in again.".to_string());
        }

        let auth_resp: AuthResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse auth refresh response: {e}"))?;

        let exp = auth_resp.expires_in.unwrap_or(3600);
        self.set_cached_session(&auth_resp.access_token, exp).await;

        Ok(auth_resp)
    }

    pub async fn logout(
        &self,
        supabase_url: &str,
        anon_key: &str,
        access_token: &str,
    ) -> Result<(), String> {
        let clean_url = supabase_url.trim_end_matches('/');
        let endpoint = format!("{clean_url}/auth/v1/logout");

        let _ = self
            .client
            .post(&endpoint)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await;

        self.clear_cached_session().await;
        Ok(())
    }
}
