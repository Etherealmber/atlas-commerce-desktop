//! Isolated per-user, per-server sessions.
//!
//! Only `username + server + session token` is ever persisted — never a
//! password. Session and account files are written privately (mode `0600`
//! on Unix) through [`fastframe_file`]. The isolation key follows the
//! Android `SessionManager`: the lowercased server origin, username and role
//! together, so two accounts on two servers never share a cache or token.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The authenticated session, restorable on the next launch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub server_url: String,
    pub username: String,
    pub role: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub avatar_url: String,
    pub token: String,
}

impl SessionRecord {
    /// Cross-server, cross-account isolation key (same shape as Android).
    #[must_use]
    pub fn scope_identity(&self) -> String {
        format!(
            "{}|{}|{}",
            self.server_url.trim().to_lowercase(),
            self.username.trim().to_lowercase(),
            self.role.trim().to_lowercase()
        )
    }

    #[must_use]
    pub fn is_owner(&self) -> bool {
        matches!(self.role.as_str(), "owner" | "admin")
    }
}

/// A saved account in the account switcher. It carries the same token as the
/// active session for that scope; there is no separate credential.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedAccount {
    pub server_url: String,
    pub username: String,
    pub role: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub avatar_url: String,
    pub token: String,
}

impl From<&SessionRecord> for SavedAccount {
    fn from(session: &SessionRecord) -> Self {
        Self {
            server_url: session.server_url.clone(),
            username: session.username.clone(),
            role: session.role.clone(),
            display_name: session.display_name.clone(),
            email: session.email.clone(),
            avatar_url: session.avatar_url.clone(),
            token: session.token.clone(),
        }
    }
}

/// Persistent account vault, keyed by scope identity.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    #[serde(default)]
    accounts: BTreeMap<String, SavedAccount>,
}

impl AccountStore {
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn upsert(&mut self, session: &SessionRecord) {
        self.accounts
            .insert(session.scope_identity(), SavedAccount::from(session));
    }

    pub fn remove(&mut self, session: &SessionRecord) {
        self.accounts.remove(&session.scope_identity());
    }

    pub fn forget(&mut self, server_url: &str, username: &str) {
        let server = server_url.trim().to_lowercase();
        let user = username.trim().to_lowercase();
        self.accounts
            .retain(|_, account| !(account.server_url.eq_ignore_ascii_case(&server)
                && account.username.eq_ignore_ascii_case(&user)));
    }

    pub fn clear_server(&mut self, server_url: &str) {
        self.accounts
            .retain(|_, account| account.server_url != server_url);
    }

    #[must_use]
    pub fn for_server(&self, server_url: &str) -> Vec<&SavedAccount> {
        self.accounts
            .values()
            .filter(|account| account.server_url == server_url)
            .collect()
    }

    #[must_use]
    pub fn all(&self) -> Vec<&SavedAccount> {
        self.accounts.values().collect()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        // Tokens live here: owner-only read.
        fastframe_file::write_private(path, json)
    }
}

/// Loads the active session when it belongs to the currently selected server.
#[must_use]
pub fn load_session(path: &Path, current_server: &str) -> Option<SessionRecord> {
    let record: SessionRecord =
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    if record
        .server_url
        .eq_ignore_ascii_case(current_server.trim_end_matches('/'))
    {
        Some(record)
    } else {
        None
    }
}

pub fn save_session(path: &Path, session: &SessionRecord) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(session)?;
    fastframe_file::write_private(path, json)
}

pub fn clear_session(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(server: &str, user: &str, role: &str) -> SessionRecord {
        SessionRecord {
            server_url: server.to_owned(),
            username: user.to_owned(),
            role: role.to_owned(),
            display_name: String::new(),
            email: String::new(),
            avatar_url: String::new(),
            token: format!("token-{server}-{user}"),
        }
    }

    #[test]
    fn scopes_isolate_server_and_account() {
        let a = session("https://a.test", "alice", "owner");
        let b = session("https://b.test", "alice", "owner");
        assert_ne!(a.scope_identity(), b.scope_identity());
    }

    #[test]
    fn vault_upsert_forget_and_filter() {
        let mut vault = AccountStore::default();
        let one = session("https://a.test", "alice", "owner");
        let two = session("https://a.test", "bob", "stock");
        vault.upsert(&one);
        vault.upsert(&two);
        assert_eq!(vault.all().len(), 2);
        assert_eq!(vault.for_server("https://a.test").len(), 2);
        vault.forget("https://a.test", "ALICE");
        assert_eq!(vault.all().len(), 1);
        vault.clear_server("https://a.test");
        assert!(vault.all().is_empty());
    }

    #[test]
    fn session_only_restores_for_its_server() {
        let dir = std::env::temp_dir().join(format!("atlas-session-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.json");
        let record = session("https://a.test", "alice", "owner");
        save_session(&path, &record).unwrap();
        assert!(load_session(&path, "https://a.test/").is_some());
        assert!(load_session(&path, "https://other.test").is_none());
        clear_session(&path);
        assert!(load_session(&path, "https://a.test").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
