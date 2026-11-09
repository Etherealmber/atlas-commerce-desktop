//! Crash-safe local authority for browser-session vault restoration.
//!
//! This file contains only a hashed server origin and a state, never a token.
//! The credential store remains the only place holding a browser session.
//! A blocked authority is written before a save or sign-out; therefore an
//! interrupted or delayed vault operation cannot by itself restore a session.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    server: String,
    state: State,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum State {
    Blocked,
    Active,
    Legacy,
}

/// How startup may restore a session for the selected server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreMode {
    /// No browser-v1 authority exists for this server; retain legacy behavior.
    Legacy,
    /// Look only in the OS vault, not the legacy session file.
    Vault,
    /// Do not restore either source.
    Blocked,
}

fn server_hash(server: &str) -> Option<String> {
    let url = Url::parse(server).ok()?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(format!(
        "{:x}",
        Sha256::digest(url.origin().ascii_serialization().as_bytes())
    ))
}

/// Determines the startup source. Invalid, unreadable or corrupt marker data
/// fails closed; absence preserves existing legacy-session behavior.
#[must_use]
pub fn restore_mode(path: &Path, server: &str) -> RestoreMode {
    let Some(hash) = server_hash(server) else {
        return RestoreMode::Legacy;
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) if bytes.len() <= 1024 => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return RestoreMode::Legacy,
        _ => return RestoreMode::Blocked,
    };
    let Ok(record) = serde_json::from_slice::<Record>(&bytes) else {
        return RestoreMode::Blocked;
    };
    if record.version != 1
        || record.server.len() != 64
        || !record.server.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return RestoreMode::Blocked;
    }
    if record.server != hash {
        return RestoreMode::Legacy;
    }
    match record.state {
        State::Active => RestoreMode::Vault,
        State::Blocked => RestoreMode::Blocked,
        State::Legacy => RestoreMode::Legacy,
    }
}

fn write(path: &Path, server: &str, state: State) -> std::io::Result<()> {
    let server =
        server_hash(server).ok_or_else(|| std::io::Error::other("invalid vault server"))?;
    let record = Record {
        version: 1,
        server,
        state,
    };
    let bytes = serde_json::to_vec(&record)?;
    fastframe_file::write_private(path, bytes)
}

/// Coordinates short local authority writes against late background saves.
/// Never hold this lock while accessing the OS credential store.
#[derive(Clone)]
pub struct Authority {
    path: PathBuf,
    generation: Arc<Mutex<u64>>,
}

impl Authority {
    /// Creates an authority bound to a non-secret local marker file.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            generation: Arc::new(Mutex::new(0)),
        }
    }

    /// Blocks restoration and returns the generation a save must later prove.
    ///
    /// # Errors
    /// The local authority cannot be written or the server is invalid.
    pub fn block(&self, server: &str) -> std::io::Result<u64> {
        let mut generation = self
            .generation
            .lock()
            .map_err(|_| std::io::Error::other("authority unavailable"))?;
        *generation = generation.wrapping_add(1);
        write(&self.path, server, State::Blocked)?;
        Ok(*generation)
    }

    /// Enables legacy restore for a successful legacy sign-in, invalidating
    /// outstanding vault saves from this process.
    ///
    /// # Errors
    /// The local authority cannot be written or the server is invalid.
    pub fn legacy(&self, server: &str) -> std::io::Result<()> {
        let mut generation = self
            .generation
            .lock()
            .map_err(|_| std::io::Error::other("authority unavailable"))?;
        *generation = generation.wrapping_add(1);
        write(&self.path, server, State::Legacy)
    }

    /// Activates a vault save only if no later block or legacy sign-in won.
    /// Returns false for stale work; it must not publish a success notice.
    ///
    /// # Errors
    /// The local authority cannot be written or the server is invalid.
    pub fn activate(&self, server: &str, expected: u64) -> std::io::Result<bool> {
        let generation = self
            .generation
            .lock()
            .map_err(|_| std::io::Error::other("authority unavailable"))?;
        if *generation != expected {
            return Ok(false);
        }
        write(&self.path, server, State::Active)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_logout_cannot_be_undone_by_a_late_save() {
        let dir =
            std::env::temp_dir().join(format!("atlas-vault-authority-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("authority.json");
        let authority = Authority::new(path.clone());
        let server = "https://a.example:8443";
        assert_eq!(restore_mode(&path, server), RestoreMode::Legacy);
        let first = authority.block(server).unwrap();
        assert_eq!(restore_mode(&path, server), RestoreMode::Blocked);
        authority.block(server).unwrap();
        assert!(!authority.activate(server, first).unwrap());
        assert_eq!(restore_mode(&path, server), RestoreMode::Blocked);
        let latest = authority.block(server).unwrap();
        assert!(authority.activate(server, latest).unwrap());
        assert_eq!(restore_mode(&path, server), RestoreMode::Vault);
        assert_eq!(
            restore_mode(&path, "https://a.example"),
            RestoreMode::Legacy
        );
        authority.legacy(server).unwrap();
        assert_eq!(restore_mode(&path, server), RestoreMode::Legacy);
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(restore_mode(&path, server), RestoreMode::Blocked);
        let _ = std::fs::remove_dir_all(dir);
    }
}
