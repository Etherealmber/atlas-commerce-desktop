//! Self-update, fed exclusively by this repository's GitHub Releases.
//!
//! - [`fetch_manifest`] reads the `update.json` release asset and powers the
//!   mandatory-update gate. No CDN is ever contacted.
//! - [`build_updater`] hands the rest (checksum verification, helper install
//!   with rollback, AppImage replacement) to `fastframe-update`.

use atlas_core::update_feed::{MANIFEST_ASSET, UpdateManifest};
use fastframe_update::{MacConfig, ReqwestTransport, UpdateConfig, Updater};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

const REPOSITORY: &str = "Etherealmber/atlas-commerce-desktop";
const SLUG: &str = "atlas-commerce-desktop";
const APP_NAME: &str = "Atlas Commerce Desktop";

/// The updater configuration. The Ed25519 publisher key is compiled in only
/// when the release workflow supplies `ATLAS_UPDATE_PUBLIC_KEY`; without it
/// the signed-checksum step is skipped and GitHub-served sha256 checksums
/// still guard every download.
pub const CONFIG: UpdateConfig = UpdateConfig {
    macos: MacConfig {
        bundle_ids: &["com.etherealmber.AtlasCommerceDesktop"],
        executable_names: &[SLUG],
        legacy_bundle_names: &[],
    },
    publisher_key: option_env!("ATLAS_UPDATE_PUBLIC_KEY"),
    // The release workflow lists the AppImage in checksums.txt, so an
    // AppImage install may replace itself on the next release.
    appimage: true,
    ..UpdateConfig::new(REPOSITORY, APP_NAME, SLUG, env!("CARGO_PKG_VERSION"))
};

/// What survives between launches so automatic checks stay inside GitHub's
/// unauthenticated quota (60 requests per hour per public address).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateCache {
    /// ETag of the last release listing; a matching one answers `304` and
    /// does not count against the API quota.
    pub etag: Option<String>,
    /// Unix timestamp of the last completed check, successful or not.
    pub last_checked_unix: u64,
    /// While set, GitHub answered that the quota was exhausted; no request
    /// is sent before it.
    pub rate_limited_until_unix: u64,
    /// Newest version seen, for an informative status line.
    pub last_version: Option<String>,
}

impl UpdateCache {
    /// Reads the file; a corrupt or missing cache simply starts empty.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        let json = serde_json::to_vec_pretty(self).unwrap_or_default();
        if let Err(error) = fastframe_file::write(path, json) {
            // A cache that cannot be written only costs one extra request;
            // never block an update or the interface for it.
            log::warn!("update cache write failed: {error}");
        }
    }

    /// Seconds still to wait when GitHub asked us to back off.
    #[must_use]
    pub fn blocked_for(&self, now: u64) -> Option<u64> {
        (self.rate_limited_until_unix > now).then(|| self.rate_limited_until_unix - now)
    }
}

/// Answer of one check, with the cache to persist.
pub struct UpdateCheck {
    pub manifest: Option<UpdateManifest>,
    pub cache: UpdateCache,
    /// GitHub answered `304 Not Modified`: nothing changed since last time.
    pub not_modified: bool,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Reads the rate-limit headers and the ETag of a GitHub response.
fn remember_headers(cache: &mut UpdateCache, headers: &reqwest::header::HeaderMap) {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    if let Some(etag) = header("etag") {
        cache.etag = Some(etag);
    }
    let remaining = header("x-ratelimit-remaining").and_then(|value| value.parse::<u64>().ok());
    if remaining == Some(0)
        && let Some(reset) = header("x-ratelimit-reset").and_then(|value| value.parse().ok())
    {
        cache.rate_limited_until_unix = reset;
    }
}

#[derive(Deserialize)]
struct ReleaseListing {
    #[serde(default)]
    draft: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Fetches the newest release's `update.json`, if a published release exists.
///
/// Sends `If-None-Match` when a previous ETag is known, records GitHub's
/// rate-limit window and never contacts the API while that window is open.
///
/// # Errors
/// Network or parse failures, as French messages.
pub async fn fetch_manifest(
    prerelease: bool,
    mut cache: UpdateCache,
) -> Result<UpdateCheck, String> {
    let now = now_unix();
    if let Some(remaining) = cache.blocked_for(now) {
        return Err(format!(
            "GitHub limite les vérifications pour encore {} min. Réessayez ensuite.",
            remaining.div_ceil(60)
        ));
    }
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "atlas-commerce-desktop/",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| format!("Client HTTP impossible : {error}"))?;

    let listing = if prerelease {
        let url = format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=5");
        let mut request = client
            .get(url)
            .header("Accept", "application/vnd.github+json");
        if let Some(etag) = cache.etag.clone() {
            request = request.header("If-None-Match", etag);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("Requête GitHub impossible : {error}"))?;
        cache.last_checked_unix = now;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(UpdateCheck {
                manifest: None,
                cache,
                not_modified: true,
            });
        }
        remember_headers(&mut cache, response.headers());
        if !response.status().is_success() {
            return Err(format!("GitHub a répondu {}", response.status()));
        }
        let releases: Vec<ReleaseListing> = response
            .json()
            .await
            .map_err(|error| format!("Liste des versions illisible : {error}"))?;
        releases.into_iter().find(|release| !release.draft)
    } else {
        let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
        let mut request = client
            .get(url)
            .header("Accept", "application/vnd.github+json");
        if let Some(etag) = cache.etag.clone() {
            request = request.header("If-None-Match", etag);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => return Err(format!("Requête GitHub impossible : {error}")),
        };
        cache.last_checked_unix = now;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(UpdateCheck {
                manifest: None,
                cache,
                not_modified: true,
            });
        }
        if response.status().as_u16() == 404 {
            return Ok(UpdateCheck {
                manifest: None,
                cache,
                not_modified: false,
            });
        }
        remember_headers(&mut cache, response.headers());
        if !response.status().is_success() {
            return Err(format!("GitHub a répondu {}", response.status()));
        }
        Some(
            response
                .json()
                .await
                .map_err(|error| format!("Version illisible : {error}"))?,
        )
    };

    let Some(release) = listing else {
        return Ok(UpdateCheck {
            manifest: None,
            cache,
            not_modified: false,
        });
    };
    let Some(asset) = release
        .assets
        .iter()
        .find(|asset| asset.name == MANIFEST_ASSET)
    else {
        return Ok(UpdateCheck {
            manifest: None,
            cache,
            not_modified: false,
        });
    };
    let manifest_response = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|error| format!("Téléchargement du manifeste impossible : {error}"))?;
    // A freshly created release is visible through the API before every asset
    // has propagated (the workflow creates the release, then uploads files).
    // Treat that window as "no update yet", not as an error the user sees.
    if manifest_response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateCheck {
            manifest: None,
            cache,
            not_modified: false,
        });
    }
    let manifest: UpdateManifest = manifest_response
        .error_for_status()
        .map_err(|error| format!("GitHub a répondu {error}"))?
        .json()
        .await
        .map_err(|error| format!("Manifeste illisible : {error}"))?;
    cache.last_checked_unix = now_unix();
    cache.last_version = Some(manifest.version.clone());
    Ok(UpdateCheck {
        manifest: Some(manifest),
        cache,
        not_modified: false,
    })
}

/// Whether a fastframe error describes a release whose assets are not
/// downloadable yet. fastframe reports a missing/duplicated asset as
/// "The release has no unique {name} download"; a not-yet-propagated file
/// comes back as an HTTP 404 from the release asset host. Both happen in the
/// publish window between release creation and the asset upload step.
#[must_use]
pub fn is_release_not_ready(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("404") || lower.contains("no unique")
}

/// Hide MSI assets only from the updater's release metadata when the
/// running installation has an Inno uninstaller. fastframe otherwise prefers
/// MSI even for Inno installs, creating a second Windows registration.
/// Asset downloads and checksum/signature validation are unchanged.
struct InstallerAwareTransport {
    inner: ReqwestTransport,
    inno: bool,
}

fn without_msi_metadata(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut release: serde_json::Value = serde_json::from_slice(bytes)?;
    let assets = release
        .get_mut("assets")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| anyhow::anyhow!("Métadonnées de mise à jour invalides"))?;
    assets.retain(|asset| {
        !asset
            .get("name")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| name.ends_with(".msi"))
    });
    Ok(serde_json::to_vec(&release)?)
}

impl fastframe_update::Transport for InstallerAwareTransport {
    fn get(
        &self,
        request: &fastframe_update::Request<'_>,
    ) -> anyhow::Result<fastframe_update::Response> {
        let response = fastframe_update::Transport::get(&self.inner, request)?;
        let metadata_prefix = format!("https://api.github.com/repos/{REPOSITORY}/releases/tags/");
        if self.inno && response.status == 200 && request.url.starts_with(&metadata_prefix) {
            const MAX_METADATA: u64 = 4 * 1024 * 1024;
            let mut body = Vec::new();
            response
                .body
                .take(MAX_METADATA + 1)
                .read_to_end(&mut body)?;
            anyhow::ensure!(
                body.len() as u64 <= MAX_METADATA,
                "Métadonnées de mise à jour trop volumineuses"
            );
            return Ok(fastframe_update::Response::ok(std::io::Cursor::new(
                without_msi_metadata(&body)?,
            )));
        }
        Ok(response)
    }
}

fn is_inno_install(executable: &std::path::Path) -> bool {
    executable
        .parent()
        .is_some_and(|dir| dir.join("unins000.exe").is_file())
}

/// Builds the blocking fastframe updater on the system-proxy-aware client.
///
/// # Errors
/// When the transport cannot be built.
pub fn build_updater() -> anyhow::Result<Updater> {
    let builder = reqwest::blocking::Client::builder();
    let inno = cfg!(target_os = "windows")
        && std::env::current_exe()
            .ok()
            .is_some_and(|path| is_inno_install(&path));
    Ok(Updater::new(
        CONFIG,
        InstallerAwareTransport {
            inner: ReqwestTransport::new(builder)?,
            inno,
        },
    ))
}

/// Blocking check for a newer release, run inside `spawn_blocking`.
///
/// # Errors
/// Propagated updater errors.
pub fn blocking_check() -> anyhow::Result<Option<fastframe_update::Release>> {
    build_updater()?.check()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inno_updates_keep_setup_and_checksums_not_msi() {
        let raw = br#"{"assets":[{"name":"atlas.msi"},{"name":"atlas-setup.exe"},{"name":"checksums.txt"}]}"#;
        let filtered: serde_json::Value =
            serde_json::from_slice(&without_msi_metadata(raw).unwrap()).unwrap();
        let names: Vec<_> = filtered["assets"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|asset| asset["name"].as_str())
            .collect();
        assert_eq!(names, ["atlas-setup.exe", "checksums.txt"]);
        assert!(without_msi_metadata(b"{}").is_err());
    }

    #[test]
    fn etag_and_rate_limit_are_remembered() {
        let mut cache = UpdateCache::default();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("etag", "\"release-42\"".parse().unwrap());
        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        headers.insert("x-ratelimit-reset", "2000000000".parse().unwrap());
        remember_headers(&mut cache, &headers);
        assert_eq!(cache.etag.as_deref(), Some("\"release-42\""));
        // Still blocked one hour before the reset, free afterwards.
        assert_eq!(cache.blocked_for(1_999_996_400), Some(3_600));
        assert_eq!(cache.blocked_for(2_000_000_000), None);
        // An ordinary answer without headers never invents a quota window.
        let mut plain = UpdateCache::default();
        remember_headers(&mut plain, &reqwest::header::HeaderMap::new());
        assert_eq!(plain.etag, None);
        assert_eq!(plain.rate_limited_until_unix, 0);
    }

    #[test]
    fn config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(CONFIG.slug, SLUG);
    }
}
