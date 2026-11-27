use std::collections::HashMap;
use std::sync::RwLock;
#[cfg(target_os = "android")]
use tauri::{Wry, plugin::PluginHandle};

pub const CAVIA_CREDENTIAL_SERVICE: &str = "com.cavia.music";
pub const SUPABASE_REFRESH_TOKEN_ACCOUNT: &str = "supabase_refresh_token";
pub const SPOTIFY_REFRESH_TOKEN_ACCOUNT: &str = "spotify_refresh_token";

/// Pluggable credential store trait allowing test isolation and native OS storage.
pub trait CredentialStore: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String>;
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, service: &str, account: &str) -> Result<(), String>;
}

/// Production credential store using OS-level secure storage (Windows Credential Manager,
/// macOS Keychain, etc.) via the `keyring` crate.
pub struct KeyringStore {
    #[cfg(target_os = "android")]
    plugin: PluginHandle<Wry>,
}

impl KeyringStore {
    #[cfg(not(target_os = "android"))]
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(target_os = "android")]
    pub fn new(plugin: PluginHandle<Wry>) -> Self {
        Self { plugin }
    }
}

#[cfg(not(target_os = "android"))]
impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for KeyringStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            struct Args<'a> {
                service: &'a str,
                account: &'a str,
            }
            #[derive(serde::Deserialize)]
            struct Response {
                found: bool,
                secret: Option<String>,
            }
            let response: Response = self
                .plugin
                .run_mobile_plugin("credentialGet", Args { service, account })
                .map_err(|_| "Android secure credential read failed".to_string())?;
            return Ok(if response.found {
                response.secret
            } else {
                None
            });
        }

        #[cfg(not(target_os = "android"))]
        {
            let entry = keyring::Entry::new(service, account)
                .map_err(|e| format!("Failed to initialize keyring entry for '{account}': {e}"))?;

            match entry.get_password() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(format!(
                    "Failed to retrieve credential for '{account}': {e}"
                )),
            }
        }
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            struct Args<'a> {
                service: &'a str,
                account: &'a str,
                secret: &'a str,
            }
            let _: serde_json::Value = self
                .plugin
                .run_mobile_plugin(
                    "credentialSet",
                    Args {
                        service,
                        account,
                        secret,
                    },
                )
                .map_err(|_| "Android secure credential write failed".to_string())?;
            return Ok(());
        }

        #[cfg(not(target_os = "android"))]
        {
            let entry = keyring::Entry::new(service, account)
                .map_err(|e| format!("Failed to initialize keyring entry for '{account}': {e}"))?;

            entry
                .set_password(secret)
                .map_err(|e| format!("Failed to store credential for '{account}': {e}"))
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            #[derive(serde::Serialize)]
            struct Args<'a> {
                service: &'a str,
                account: &'a str,
            }
            let _: serde_json::Value = self
                .plugin
                .run_mobile_plugin("credentialDelete", Args { service, account })
                .map_err(|_| "Android secure credential deletion failed".to_string())?;
            return Ok(());
        }

        #[cfg(not(target_os = "android"))]
        {
            let entry = keyring::Entry::new(service, account)
                .map_err(|e| format!("Failed to initialize keyring entry for '{account}': {e}"))?;

            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(format!("Failed to delete credential for '{account}': {e}")),
            }
        }
    }
}

/// In-memory credential store for hermetic unit and integration testing without
/// touching or polluting the developer's real host credential manager.
pub struct InMemoryCredentialStore {
    store: RwLock<HashMap<(String, String), String>>,
}

impl InMemoryCredentialStore {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        let map = self
            .store
            .read()
            .map_err(|_| "Poisoned credential store lock".to_string())?;
        Ok(map
            .get(&(service.to_string(), account.to_string()))
            .cloned())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), String> {
        let mut map = self
            .store
            .write()
            .map_err(|_| "Poisoned credential store lock".to_string())?;
        map.insert(
            (service.to_string(), account.to_string()),
            secret.to_string(),
        );
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), String> {
        let mut map = self
            .store
            .write()
            .map_err(|_| "Poisoned credential store lock".to_string())?;
        map.remove(&(service.to_string(), account.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_credential_store_lifecycle() {
        let store = InMemoryCredentialStore::new();

        // 1. Initially empty
        assert_eq!(store.get("test_service", "user1").unwrap(), None);

        // 2. Set secret
        store
            .set("test_service", "user1", "secret_pass_123")
            .unwrap();
        assert_eq!(
            store.get("test_service", "user1").unwrap(),
            Some("secret_pass_123".to_string())
        );

        // 3. Independent accounts
        assert_eq!(store.get("test_service", "user2").unwrap(), None);
        assert_eq!(store.get("other_service", "user1").unwrap(), None);

        // 4. Update secret
        store
            .set("test_service", "user1", "new_secret_456")
            .unwrap();
        assert_eq!(
            store.get("test_service", "user1").unwrap(),
            Some("new_secret_456".to_string())
        );

        // 5. Delete secret
        store.delete("test_service", "user1").unwrap();
        assert_eq!(store.get("test_service", "user1").unwrap(), None);

        // 6. Delete non-existent is idempotent Ok(())
        assert!(store.delete("test_service", "user1").is_ok());
    }
}
