//! Per-server, per-account system-vault storage for desktop v1 sessions.
//!
//! Both the active-account pointer and the session are stored in the OS
//! credential store. No token or account index is written to a local file.
//! Operations are synchronous and must run off the interface thread.

// keyring otherwise silently selects its non-persistent mock backend on an
// unsupported target, which must never masquerade as a secure vault.
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("Atlas desktop v1 vault requires Windows, macOS, or Linux");

use sha2::{Digest, Sha256};
use url::Url;

use crate::SessionRecord;

const SERVICE: &str = "com.etherealmber.AtlasCommerceDesktop.v1";

/// Vault operations intentionally expose no backend errors, paths or secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
    /// The OS credential store could not complete an operation.
    Unavailable,
    /// A supplied scope or stored value failed validation.
    Invalid,
}

trait Store {
    fn get(&self, key: &str) -> Result<Option<String>, VaultError>;
    fn put(&self, key: &str, value: &str) -> Result<(), VaultError>;
    fn delete(&self, key: &str) -> Result<(), VaultError>;
}

struct SystemStore;

impl Store for SystemStore {
    fn get(&self, key: &str) -> Result<Option<String>, VaultError> {
        match keyring::Entry::new(SERVICE, key)
            .map_err(|_| VaultError::Unavailable)?
            .get_password()
        {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(VaultError::Unavailable),
        }
    }

    fn put(&self, key: &str, value: &str) -> Result<(), VaultError> {
        keyring::Entry::new(SERVICE, key)
            .map_err(|_| VaultError::Unavailable)?
            .set_password(value)
            .map_err(|_| VaultError::Unavailable)
    }

    fn delete(&self, key: &str) -> Result<(), VaultError> {
        match keyring::Entry::new(SERVICE, key)
            .map_err(|_| VaultError::Unavailable)?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(VaultError::Unavailable),
        }
    }
}

fn canonical_origin(raw: &str) -> Result<String, VaultError> {
    let url = Url::parse(raw).map_err(|_| VaultError::Invalid)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(VaultError::Invalid);
    }
    Ok(url.origin().ascii_serialization())
}

fn account(raw: &str) -> Result<String, VaultError> {
    let name = raw.trim().to_lowercase();
    if name.is_empty() || name.len() > 512 || name.chars().any(char::is_control) {
        return Err(VaultError::Invalid);
    }
    Ok(name)
}

fn digest(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn active_key(server: &str) -> String {
    format!("active-{}", digest(&[server]))
}

fn session_key(server: &str, username: &str) -> String {
    format!("session-{}", digest(&[server, username]))
}

fn store_session(store: &impl Store, session: &SessionRecord) -> Result<(), VaultError> {
    let server = canonical_origin(&session.server_url)?;
    let username = account(&session.username)?;
    if session.token.is_empty() || session.role.trim().is_empty() {
        return Err(VaultError::Invalid);
    }
    let serialized = serde_json::to_string(session).map_err(|_| VaultError::Invalid)?;
    // Write the secret first. An interrupted write cannot point to a missing
    // new session; the account pointer itself never leaves the OS vault.
    store.put(&session_key(&server, &username), &serialized)?;
    store.put(&active_key(&server), &username)
}

fn restore_session(store: &impl Store, origin: &str) -> Result<Option<SessionRecord>, VaultError> {
    let server = canonical_origin(origin)?;
    let Some(username) = store.get(&active_key(&server))? else {
        return Ok(None);
    };
    let username = account(&username)?;
    let Some(json) = store.get(&session_key(&server, &username))? else {
        return Ok(None);
    };
    let session: SessionRecord = serde_json::from_str(&json).map_err(|_| VaultError::Invalid)?;
    if canonical_origin(&session.server_url)? != server
        || account(&session.username)? != username
        || session.token.is_empty()
        || session.role.trim().is_empty()
    {
        return Err(VaultError::Invalid);
    }
    Ok(Some(session))
}

fn remove_session(store: &impl Store, session: &SessionRecord) -> Result<(), VaultError> {
    let server = canonical_origin(&session.server_url)?;
    let username = account(&session.username)?;
    let active = active_key(&server);
    if store.get(&active)?.as_deref() == Some(username.as_str()) {
        // Remove the pointer first: failure deleting the secret cannot cause
        // a signed-out session to be restored on the next launch.
        store.delete(&active)?;
    }
    store.delete(&session_key(&server, &username))
}

/// Saves the v1 session in the system credential store. Never falls back to a file.
///
/// # Errors
/// The scope is invalid or the OS vault is unavailable.
pub fn save(session: &SessionRecord) -> Result<(), VaultError> {
    store_session(&SystemStore, session)
}

/// Restores the active v1 account for exactly this HTTPS server.
///
/// # Errors
/// The scope or stored value is invalid, or the OS vault is unavailable.
pub fn load(server: &str) -> Result<Option<SessionRecord>, VaultError> {
    restore_session(&SystemStore, server)
}

/// Removes this account and its active pointer, without touching other accounts.
///
/// # Errors
/// The scope is invalid or the OS vault is unavailable.
pub fn remove(session: &SessionRecord) -> Result<(), VaultError> {
    remove_session(&SystemStore, session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap};

    #[derive(Default)]
    struct MemoryStore(RefCell<HashMap<String, String>>);
    impl Store for MemoryStore {
        fn get(&self, key: &str) -> Result<Option<String>, VaultError> {
            Ok(self.0.borrow().get(key).cloned())
        }
        fn put(&self, key: &str, value: &str) -> Result<(), VaultError> {
            self.0.borrow_mut().insert(key.into(), value.into());
            Ok(())
        }
        fn delete(&self, key: &str) -> Result<(), VaultError> {
            self.0.borrow_mut().remove(key);
            Ok(())
        }
    }

    fn session(server: &str, user: &str) -> SessionRecord {
        SessionRecord {
            server_url: server.into(),
            username: user.into(),
            role: "owner".into(),
            display_name: String::new(),
            email: String::new(),
            avatar_url: String::new(),
            token: "fictional-token".into(),
        }
    }

    #[test]
    fn servers_accounts_and_ports_are_isolated() {
        let store = MemoryStore::default();
        let first = session("https://a.example", "Alice");
        let other = session("https://a.example:8443", "Alice");
        let bob = session("https://a.example", "Bob");
        store_session(&store, &first).unwrap();
        store_session(&store, &other).unwrap();
        store_session(&store, &bob).unwrap();
        assert_eq!(
            restore_session(&store, "https://a.example/").unwrap(),
            Some(bob.clone())
        );
        assert_eq!(
            restore_session(&store, "https://a.example:8443").unwrap(),
            Some(other)
        );
        remove_session(&store, &first).unwrap();
        assert_eq!(
            restore_session(&store, "https://a.example").unwrap(),
            Some(bob.clone())
        );
        remove_session(&store, &bob).unwrap();
        assert!(
            restore_session(&store, "https://a.example")
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .0
                .borrow()
                .keys()
                .all(|key| !key.contains("alice") && !key.contains("example"))
        );
    }

    #[test]
    fn rejects_bad_origins_and_tampered_scope() {
        let store = MemoryStore::default();
        let good = session("https://a.example", "alice");
        store_session(&store, &good).unwrap();
        for bad in [
            "http://a.example",
            "https://a.example/path",
            "https://a.example/?q=x",
            "https://user@a.example",
        ] {
            assert_eq!(
                restore_session(&store, bad).err(),
                Some(VaultError::Invalid)
            );
        }
        let other = session("https://elsewhere.example", "alice");
        store
            .put(
                &session_key("https://a.example", "alice"),
                &serde_json::to_string(&other).unwrap(),
            )
            .unwrap();
        assert_eq!(
            restore_session(&store, "https://a.example").err(),
            Some(VaultError::Invalid)
        );
    }
}
