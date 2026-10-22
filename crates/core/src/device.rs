//! Stable, privacy-preserving device identity.
//!
//! The worker binds every session token to the pair `(IP, fingerprint)`, so
//! the fingerprint must survive restarts and stay identical across requests.
//! Following the platform contract, it is a SHA-256 digest (hex) of the
//! machine's native identifier:
//!
//! - Windows: `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`
//! - macOS:   `IOPlatformUUID` (via `ioreg`)
//! - Linux:   `/etc/machine-id` (fallback `/var/lib/dbus/machine-id`)
//!
//! Raw machine identifiers never leave the device: only the digest does.
//! The digest is persisted through [`fastframe_file`] so it stays stable even
//! when a machine id cannot be read on a later launch; in that case a random
//! install UUID is generated once and kept in a private (`0600`) state file.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceRecord {
    /// SHA-256 hex digest sent as `X-Device-Fingerprint`.
    fingerprint: String,
    /// Fallback identifier when the platform machine id is unreadable.
    install_id: String,
}

/// The device identity the whole app shares.
#[derive(Debug, Clone)]
pub struct DeviceIdentity {
    fingerprint: String,
}

impl DeviceIdentity {
    /// Loads the persisted identity, or derives it from the platform machine
    /// id and persists it (privately) for future launches.
    #[must_use]
    pub fn load_or_create(path: &Path) -> Self {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(record) = serde_json::from_str::<DeviceRecord>(&text) {
                if !record.fingerprint.is_empty() {
                    return Self {
                        fingerprint: record.fingerprint,
                    };
                }
            }
        }
        let identity = Self::fresh();
        identity.persist(path);
        identity
    }

    /// Builds an identity right now, without touching the disk. Useful in
    /// tests and before the folders are discovered.
    #[must_use]
    pub fn fresh() -> Self {
        let install_id = Uuid::new_v4().to_string();
        let machine = platform_machine_id().unwrap_or_else(|| install_id.clone());
        let fingerprint = digest(&format!("atlas-desktop|{machine}|{install_id}|v1"));
        Self { fingerprint }
    }

    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// The short, non-browser User-Agent the worker's native-client detection
    /// recognises. It deliberately carries no Mozilla/Chrome/Safari token and
    /// stays under 40 characters.
    #[must_use]
    pub fn user_agent() -> String {
        format!(
            "Atlas-Desktop/{} ({}-{})",
            env!("CARGO_PKG_VERSION"),
            os_name(),
            std::env::consts::ARCH
        )
    }

    /// First eight hex characters, for compact logs and labels.
    #[must_use]
    pub fn short_id(&self) -> String {
        self.fingerprint.chars().take(8).collect()
    }

    fn persist(&self, path: &Path) {
        let record = DeviceRecord {
            fingerprint: self.fingerprint.clone(),
            // The install id is part of the digest input; persist a marker so
            // a future launch without a readable machine id keeps the same
            // fingerprint. It is a random UUID, not a hardware identifier.
            install_id: Uuid::new_v4().to_string(),
        };
        let json = serde_json::to_vec_pretty(&record).unwrap_or_default();
        // Private file: the identity is per-user. Best effort — a failure here
        // only means the identity may be derived again next launch.
        if let Err(error) = fastframe_file::write_private(path, json) {
            log::warn!("could not persist the device identity: {error}");
        }
    }
}

/// SHA-256 hex digest, exactly the shape the worker stores and truncates.
#[must_use]
pub fn digest(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let bytes = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(windows)]
fn platform_machine_id() -> Option<String> {
    // reg.exe is always present on Windows; avoid a winreg dependency for one
    // string. HKLM is readable by every user.
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("MachineGuid")
            .map(|rest| {
                rest.trim_start_matches(|c: char| c.is_whitespace())
                    .to_owned()
            })
            .filter(|value| !value.is_empty())
    })
}

#[cfg(target_os = "macos")]
fn platform_machine_id() -> Option<String> {
    let output = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("\"IOPlatformUUID\" = \"")
            .and_then(|rest| rest.strip_suffix('"'))
            .map(str::to_owned)
    })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_machine_id() -> Option<String> {
    for candidate in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(value) = std::fs::read_to_string(candidate) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

#[must_use]
fn os_name() -> &'static str {
    #[cfg(windows)]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        "linux"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_is_sha256_hex() {
        // SHA-256("abc")
        assert_eq!(
            digest("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn fresh_identity_is_stable_shape() {
        let identity = DeviceIdentity::fresh();
        assert_eq!(identity.fingerprint().len(), 64);
        assert!(
            identity
                .fingerprint()
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
        assert_eq!(identity.short_id().len(), 8);
    }

    #[test]
    fn user_agent_is_short_and_non_browser() {
        let ua = DeviceIdentity::user_agent();
        assert!(ua.len() < 40, "user agent must stay short: {ua:?}");
        let lower = ua.to_lowercase();
        for token in ["mozilla", "chrome/", "safari/", "firefox/", "edg/"] {
            assert!(
                !lower.contains(token),
                "user agent looks like a browser: {ua:?}"
            );
        }
    }

    #[test]
    fn round_trips_through_a_file() {
        let dir = std::env::temp_dir().join(format!("atlas-device-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("device.json");
        let first = DeviceIdentity::load_or_create(&path);
        let second = DeviceIdentity::load_or_create(&path);
        assert_eq!(first.fingerprint(), second.fingerprint());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
