//! Server choosing and the saved-server list for user-provided Atlas workers.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// What the installer writes to `server.json` beside the executable.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct InstallChoice {
    url: String,
    #[serde(default)]
    label: String,
}

/// One saved server (label + normalized URL).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedServerEntry {
    pub label: String,
    pub url: String,
}

/// Everything persisted about server choice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ServerStore {
    /// The base URL the app talks to, or `None` before first-run setup.
    #[serde(default)]
    pub selected: Option<String>,
    #[serde(default)]
    pub saved: Vec<SavedServerEntry>,
    /// Whether the onboarding welcome was shown once.
    #[serde(default)]
    pub onboarding_done: bool,
    /// Set by the installer or by the first server choice: the address is
    /// then read-only in Settings. Only a reinstall can pick another server.
    #[serde(default)]
    pub locked: bool,
    /// Installer-provisioned origin must pass a health check before use.
    /// Missing on older installations: retain their existing verified choice.
    #[serde(default)]
    pub requires_verification: bool,
}

impl ServerStore {
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).ok();
        text.and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Validates, normalizes and selects a server, adding it to the saved
    /// list (deduplicated by normalized URL). Switching server is the
    /// caller's cue to sign the current session out.
    pub fn select(&mut self, raw_url: &str, label: &str) -> Result<String, ServerError> {
        let normalized = ServerUrl::normalize(raw_url).ok_or(ServerError::InvalidUrl)?;
        if !self.saved.iter().any(|entry| entry.url == normalized) {
            self.saved.push(SavedServerEntry {
                label: if label.trim().is_empty() {
                    ServerUrl::host(&normalized)
                } else {
                    label.trim().to_owned()
                },
                url: normalized.clone(),
            });
        }
        self.selected = Some(normalized.clone());
        self.onboarding_done = true;
        Ok(normalized)
    }

    /// Applies the address the installer wrote next to the executable: it
    /// becomes the only saved server and the whole installation is locked to
    /// it. Returns the normalized URL, or `None` when the file is unreadable
    /// or unsafe (non-HTTPS, credentials, path…).
    pub fn apply_install_choice(&mut self, url: &str, label: &str) -> Option<String> {
        let normalized = ServerUrl::normalize(url)?;
        if ServerUrl::is_cleartext(&normalized) && !ServerUrl::is_loopback(&normalized) {
            return None;
        }
        let unchanged_verified =
            self.selected.as_deref() == Some(normalized.as_str()) && !self.requires_verification;
        self.requires_verification = !unchanged_verified;
        self.saved = vec![SavedServerEntry {
            label: if label.trim().is_empty() {
                ServerUrl::host(&normalized)
            } else {
                label.trim().to_owned()
            },
            url: normalized.clone(),
        }];
        self.selected = Some(normalized.clone());
        self.onboarding_done = true;
        self.locked = true;
        Some(normalized)
    }

    /// Locks the current choice. Called once the user picked a server on a
    /// build that has no installer prompt (Linux/macOS archives).
    pub fn lock_current(&mut self) -> bool {
        if self.selected.is_some() {
            self.locked = true;
        }
        self.locked
    }

    /// Whether the address may still be changed from the interface.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Imports the `server.json` the Windows installer writes beside the
    /// executable, then removes the hand-off file. The file is data, not
    /// code: only an HTTPS (or loopback HTTP) origin without credentials,
    /// path, query or fragment is accepted.
    ///
    /// The installer writes the *previous* value during an upgrade (it
    /// pre-fills its page from the registry), so an upgrade keeps the
    /// address while a deliberate reinstall can provision a new one. Seeing
    /// this file means setup ran, which is exactly the "reinstall to change
    /// the server" path the interface documents.
    pub fn import_install_file(&mut self, path: &Path) -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        let choice: Option<InstallChoice> = serde_json::from_str(&text).ok();
        let applied = choice
            .as_ref()
            .and_then(|choice| self.apply_install_choice(&choice.url, &choice.label));
        // Never leave a stale hand-off: it is read once.
        let _ = std::fs::remove_file(path);
        applied
    }

    pub fn forget(&mut self, url: &str) {
        self.saved.retain(|entry| entry.url != url);
        if self.selected.as_deref() == Some(url) {
            self.selected = None;
        }
    }

    #[must_use]
    pub fn selected_url(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Atomic whole-file replacement, off the UI thread in practice.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        fastframe_file::write(path, json)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServerError {
    #[error("Adresse de serveur invalide.")]
    InvalidUrl,
    #[error("HTTP n'est pas autorisé pour cette adresse. Utilisez HTTPS.")]
    CleartextRefused,
}

/// URL helpers shared by the setup screen, the probe and deep-link parsing.
pub struct ServerUrl;

impl ServerUrl {
    /// Normalizes a user-typed address into an `https://host` base without a
    /// trailing path, or `None` when it is not an HTTP(S) URL.
    #[must_use]
    pub fn normalize(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        // Distinguish an explicit URI scheme from a bare `host:port`. The
        // grammar is RFC 3986's scheme rule; an all-digit part after the colon
        // identifies a port (localhost:8080), while mailto:, ftp: and custom
        // app schemes are rejected instead of becoming fake https hosts.
        let first_slash = trimmed.find('/').unwrap_or(trimmed.len());
        let explicit_scheme = trimmed[..first_slash]
            .split_once(':')
            .filter(|(scheme, rest)| {
                let valid_scheme = !scheme.is_empty()
                    && scheme
                        .chars()
                        .next()
                        .is_some_and(|first| first.is_ascii_alphabetic())
                    && scheme
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
                // Closures, not fn pointers: is_ascii_digit takes &u8, while
                // Iterator::all passes u8 by value.
                let looks_like_port =
                    !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit());
                valid_scheme && !looks_like_port
            })
            .map(|(scheme, _)| scheme);
        let with_scheme = match explicit_scheme {
            Some(scheme) if matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") => {
                trimmed.to_owned()
            }
            Some(_) => return None,
            None => format!("https://{trimmed}"),
        };
        let parsed = url::Url::parse(&with_scheme).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }
        parsed.host_str()?;
        // A desktop authorization is bound to exactly the server origin.
        // Never silently reinterpret a path, query, fragment or credentials.
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return None;
        }
        Some(parsed.origin().ascii_serialization())
    }

    #[must_use]
    pub fn is_cleartext(base: &str) -> bool {
        base.starts_with("http://")
    }

    /// True for a loopback literal, the only host allowed over plain HTTP so
    /// a shop can try a local worker without faking a TLS certificate.
    #[must_use]
    pub fn is_loopback(base: &str) -> bool {
        url::Url::parse(base)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .is_some_and(|host| {
                let host = host.trim_matches(['[', ']']);
                host.eq_ignore_ascii_case("localhost")
                    || host == "::1"
                    || host
                        .parse::<std::net::Ipv4Addr>()
                        .is_ok_and(|ip| ip.is_loopback())
            })
    }

    #[must_use]
    pub fn host(base: &str) -> String {
        url::Url::parse(base)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_else(|| base.to_owned())
    }

    /// Joins a path such as `api/health` onto the base, exactly one slash.
    #[must_use]
    pub fn join(base: &str, path: &str) -> String {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_choice_locks_the_only_server() {
        let mut store = ServerStore::default();
        // Plain HTTP is refused except on loopback.
        assert_eq!(
            store.apply_install_choice("http://shop.example.com", ""),
            None
        );
        assert_eq!(
            store.apply_install_choice("http://localhost:8787", " Atelier "),
            Some("http://localhost:8787".to_owned())
        );
        assert!(store.is_locked());
        assert_eq!(store.saved.len(), 1);
        assert_eq!(store.saved[0].label, "Atelier");
        assert_eq!(store.selected_url(), Some("http://localhost:8787"));
        // Credentials or a path never enter a server address.
        assert_eq!(
            store.apply_install_choice("https://u:p@example.com", ""),
            None
        );
        assert_eq!(
            store.apply_install_choice("https://example.com/api", ""),
            None
        );
    }

    #[test]
    fn install_file_is_data_only_and_applies_each_reinstall() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("atlas-server-{unique}.json"));
        std::fs::write(
            &path,
            r#"{"url":"https://worker.example.dev/","label":"Boutique"}"#,
        )
        .unwrap();
        let mut store = ServerStore::default();
        assert_eq!(
            store.import_install_file(&path).as_deref(),
            Some("https://worker.example.dev")
        );
        assert!(store.is_locked());
        // The hand-off file is consumed.
        assert!(!path.exists());
        // A later setup run is the documented way to reprovision the server;
        // the installer pre-fills an upgrade with the previous address, so
        // this only changes on a deliberate reinstall.
        std::fs::write(&path, r#"{"url":"https://other.example.dev"}"#).unwrap();
        assert_eq!(
            store.import_install_file(&path).as_deref(),
            Some("https://other.example.dev")
        );
        assert_eq!(store.selected_url(), Some("https://other.example.dev"));
        // Invalid data is discarded and leaves the current choice intact.
        std::fs::write(&path, r#"{"url":"http://evil.example.com"}"#).unwrap();
        assert_eq!(store.import_install_file(&path), None);
        assert_eq!(store.selected_url(), Some("https://other.example.dev"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn normalizes_typed_addresses() {
        assert_eq!(
            ServerUrl::normalize("stock.afmarbre.com").as_deref(),
            Some("https://stock.afmarbre.com")
        );
        assert_eq!(
            ServerUrl::normalize("https://worker.example.dev/").as_deref(),
            Some("https://worker.example.dev")
        );
        assert_eq!(ServerUrl::normalize("https://atlas.local:8787/api"), None);
        assert_eq!(
            ServerUrl::normalize("https://alice:secret@atlas.local"),
            None
        );
        assert_eq!(ServerUrl::normalize("https://atlas.local/?key=value"), None);
        assert_eq!(ServerUrl::normalize("https://atlas.local/#fragment"), None);
        assert_eq!(
            ServerUrl::normalize("https://[::1]:8443").as_deref(),
            Some("https://[::1]:8443")
        );
        assert_eq!(ServerUrl::normalize("not a url"), None);
        assert_eq!(ServerUrl::normalize(""), None);
        assert_eq!(ServerUrl::normalize("ftp://example.com"), None);
        // Schemes without "://" must not be turned into https hosts.
        assert_eq!(ServerUrl::normalize("mailto:user@example.com"), None);
        assert_eq!(ServerUrl::normalize("atlascommerce://verify?x=1"), None);
        // A bare host:port is still a host, not a scheme.
        assert_eq!(
            ServerUrl::normalize("localhost:8080").as_deref(),
            Some("https://localhost:8080")
        );
    }

    #[test]
    fn joins_paths_with_one_slash() {
        assert_eq!(
            ServerUrl::join("https://h.test/", "/api/health"),
            "https://h.test/api/health"
        );
    }

    #[test]
    fn store_select_dedupes_and_persists() {
        let dir = std::env::temp_dir().join(format!("atlas-servers-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("servers.json");
        let mut store = ServerStore::default();
        let url = store
            .select("boutique.afmarbre.com", "Ma boutique")
            .unwrap();
        assert_eq!(url, "https://boutique.afmarbre.com");
        assert_eq!(store.saved.len(), 1);
        // Selecting the same URL again must not duplicate it.
        store.select("https://boutique.afmarbre.com", "").unwrap();
        assert_eq!(store.saved.len(), 1);
        store.save(&path).unwrap();
        let loaded = ServerStore::load(&path);
        assert_eq!(loaded, store);
        store.forget(&url);
        assert!(store.selected_url().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_server_store_remains_usable() {
        let legacy = r#"{"selected":"https://shop.example.test","saved":[],"onboarding_done":true,"locked":true}"#;
        let store: ServerStore = serde_json::from_str(legacy).unwrap();
        assert!(!store.requires_verification);
        assert_eq!(store.selected_url(), Some("https://shop.example.test"));
    }

    #[test]
    fn fresh_store_has_no_default_server_and_provisioning_requires_probe() {
        let mut store = ServerStore::load(
            &std::env::temp_dir().join(format!("atlas-missing-{}", uuid::Uuid::new_v4())),
        );
        assert!(store.selected_url().is_none());
        assert!(store.saved.is_empty());
        store
            .apply_install_choice("https://shop.example.test", "")
            .unwrap();
        assert!(store.requires_verification);
        store.requires_verification = false;
        store
            .apply_install_choice("https://shop.example.test/", "")
            .unwrap();
        assert!(
            !store.requires_verification,
            "upgrade retains verified server"
        );
        store
            .apply_install_choice("https://other.example.test", "")
            .unwrap();
        assert!(
            store.requires_verification,
            "a different server must be checked"
        );
    }
}
