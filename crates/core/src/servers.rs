//! Server choosing: the production Cloudflare entry, custom self-hosted
//! workers, and the saved-server list. Mirrors the Android app's
//! `ServerConfig` / `SavedServerEntry` architecture.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The Atlas Cloud production worker.
pub const PRODUCTION_URL: &str = "https://stock.afmarbre.com";
/// Label of the prefilled production entry.
pub const PRODUCTION_LABEL: &str = "Atlas Cloud";

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
}

impl ServerStore {
    /// The list every fresh install starts from: Atlas Cloud first.
    #[must_use]
    pub fn with_production() -> Self {
        Self {
            selected: None,
            saved: vec![SavedServerEntry {
                label: PRODUCTION_LABEL.to_owned(),
                url: PRODUCTION_URL.to_owned(),
            }],
            onboarding_done: false,
        }
    }

    #[must_use]
    pub fn load(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).ok();
        text.and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(Self::with_production)
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
        let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            trimmed.to_owned()
        } else {
            format!("https://{trimmed}")
        };
        let parsed = url::Url::parse(&with_scheme).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }
        let host = parsed.host_str()?;
        if host.is_empty() {
            return None;
        }
        let mut base = format!("{}://{host}", parsed.scheme());
        if let Some(port) = parsed.port() {
            base.push_str(&format!(":{port}"));
        }
        Some(base)
    }

    #[must_use]
    pub fn is_cleartext(base: &str) -> bool {
        base.starts_with("http://")
    }

    #[must_use]
    pub fn host(base: &str) -> String {
        url::Url::parse(base)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_else(|| base.to_owned())
    }

    #[must_use]
    pub fn is_production(base: &str) -> bool {
        base.trim_end_matches('/') == PRODUCTION_URL
    }

    /// Joins a path such as `api/health` onto the base, exactly one slash.
    #[must_use]
    pub fn join(base: &str, path: &str) -> String {
        format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            ServerUrl::normalize(" https://atlas.local:8787/api ").as_deref(),
            Some("https://atlas.local:8787")
        );
        assert_eq!(ServerUrl::normalize("not a url"), None);
        assert_eq!(ServerUrl::normalize(""), None);
        assert_eq!(ServerUrl::normalize("ftp://example.com"), None);
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
        let mut store = ServerStore::with_production();
        let url = store.select("boutique.afmarbre.com", "Ma boutique").unwrap();
        assert_eq!(url, "https://boutique.afmarbre.com");
        assert_eq!(store.saved.len(), 2);
        // Selecting the same URL again must not duplicate it.
        store.select("https://boutique.afmarbre.com", "").unwrap();
        assert_eq!(store.saved.len(), 2);
        store.save(&path).unwrap();
        let loaded = ServerStore::load(&path);
        assert_eq!(loaded, store);
        store.forget(&url);
        assert!(store.selected_url().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn production_is_recognized() {
        assert!(ServerUrl::is_production("https://stock.afmarbre.com/"));
        assert!(!ServerUrl::is_production("https://stock.afmarbre.com.evil.test"));
    }
}
