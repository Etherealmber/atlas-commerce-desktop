//! Update discovery and GitHub installation support. CDN manifests are
//! validated independently of GitHub's cache and never imply installation.

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
    /// ETags of the two listing endpoints; a matching one answers `304` and
    /// does not count against the API quota. They are kept apart so
    /// switching channel never reuses the other endpoint's validator.
    pub etag: Option<String>,
    pub etag_prerelease: Option<String>,
    /// A 304 is meaningful only together with the previously parsed manifest.
    pub manifest_stable: Option<UpdateManifest>,
    pub manifest_prerelease: Option<UpdateManifest>,
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
    pub desktop_release: Option<atlas_core::desktop_update::DesktopRelease>,
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
fn remember_headers(
    cache: &mut UpdateCache,
    headers: &reqwest::header::HeaderMap,
    prerelease: bool,
) {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    if let Some(etag) = header("etag") {
        if prerelease {
            cache.etag_prerelease = Some(etag);
        } else {
            cache.etag = Some(etag);
        }
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

const WORKER_URL: &str = "https://api.afmarbre.com/api/app/desktop-update";
const CDN_URL: &str = "https://cdn1.afmarbre.com/app-updates/desktop/latest.json";
const MAX_DESKTOP_MANIFEST: usize = 128 * 1024;

fn desktop_target(os: &str, cpu: &str) -> Option<(&'static str, &'static str)> {
    let arch = match cpu {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        _ => return None,
    };
    match os {
        "windows" => Some(("windows", "universal")),
        "macos" => Some(("macos", "universal")),
        "linux" => Some(("linux", arch)),
        _ => None,
    }
}

#[derive(Debug)]
enum DesktopFetchError {
    Unavailable,
    Invalid(String),
}

async fn desktop_source(
    client: &reqwest::Client,
    url: &str,
    params: Option<&[(&str, &str)]>,
    prerelease: bool,
    os: &str,
    arch: &str,
    _direct: bool,
) -> Result<Option<atlas_core::desktop_update::DesktopRelease>, DesktopFetchError> {
    let mut request = client
        .get(url)
        .header(reqwest::header::CACHE_CONTROL, "no-cache");
    if let Some(params) = params {
        request = request.query(params);
    }
    let mut response = request
        .send()
        .await
        .map_err(|_| DesktopFetchError::Unavailable)?;
    let status = response.status();
    if status.is_server_error() {
        return Err(DesktopFetchError::Unavailable);
    }
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(DesktopFetchError::Invalid(format!(
            "Flux Desktop : HTTP {status}"
        )));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_DESKTOP_MANIFEST as u64)
    {
        return Err(DesktopFetchError::Invalid(
            "Manifeste Desktop trop volumineux".into(),
        ));
    }
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| DesktopFetchError::Unavailable)?
    {
        if chunk.len() > MAX_DESKTOP_MANIFEST.saturating_sub(data.len()) {
            return Err(DesktopFetchError::Invalid(
                "Manifeste Desktop trop volumineux".into(),
            ));
        }
        data.extend_from_slice(&chunk);
    }
    atlas_core::desktop_update::parse_desktop_release(
        &data,
        env!("CARGO_PKG_VERSION"),
        prerelease,
        os,
        arch,
    )
    .map_err(|error| DesktopFetchError::Invalid(format!("Manifeste Desktop invalide : {error}")))
}

/// Discover Desktop metadata without credentials. Network failures can fall
/// back, but cannot be presented as proof that the application is current.
///
/// # Errors
/// Invalid metadata, HTTP errors, and unavailable sources are reported explicitly.
pub async fn fetch_manifest(
    prerelease: bool,
    cache: UpdateCache,
    cache_path: std::path::PathBuf,
) -> Result<UpdateCheck, String> {
    let Some((os, arch)) = desktop_target(std::env::consts::OS, std::env::consts::ARCH) else {
        return fetch_github_manifest(prerelease, cache, cache_path).await;
    };
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "atlas-commerce-desktop/",
            env!("CARGO_PKG_VERSION")
        ))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| "Client de mise à jour indisponible".to_owned())?;
    let channel = if prerelease { "prerelease" } else { "stable" };
    let params = [
        ("platform", os),
        ("arch", arch),
        ("version", env!("CARGO_PKG_VERSION")),
        ("channel", channel),
    ];
    let (found, unreachable) =
        discover_desktop(&client, WORKER_URL, CDN_URL, &params, prerelease, os, arch).await?;
    if let Some(release) = found {
        let mut cache = cache;
        cache.last_checked_unix = now_unix();
        return Ok(UpdateCheck {
            manifest: Some(release.manifest.clone()),
            desktop_release: Some(release),
            cache,
            not_modified: false,
        });
    }
    let github = fetch_github_manifest(prerelease, cache, cache_path).await?;
    if unreachable
        && github.manifest.as_ref().is_none_or(|m| {
            !matches!(
                m.decision_for(env!("CARGO_PKG_VERSION")),
                atlas_core::update_feed::UpdateDecision::Optional { .. }
                    | atlas_core::update_feed::UpdateDecision::Mandatory { .. }
            )
        })
    {
        return Err("Le service Desktop est injoignable ; impossible de confirmer que cette version est à jour.".into());
    }
    Ok(github)
}

async fn discover_desktop(
    client: &reqwest::Client,
    worker_url: &str,
    cdn_url: &str,
    params: &[(&str, &str)],
    prerelease: bool,
    os: &str,
    arch: &str,
) -> Result<(Option<atlas_core::desktop_update::DesktopRelease>, bool), String> {
    match desktop_source(
        client,
        worker_url,
        Some(params),
        prerelease,
        os,
        arch,
        false,
    )
    .await
    {
        Ok(Some(release)) => Ok((Some(release), false)),
        Err(DesktopFetchError::Invalid(error)) => Err(error),
        worker_result => {
            let worker_unreachable = matches!(worker_result, Err(DesktopFetchError::Unavailable));
            match desktop_source(client, cdn_url, None, prerelease, os, arch, true).await {
                Ok(result) => Ok((result, false)),
                Err(DesktopFetchError::Invalid(error)) => Err(error),
                Err(DesktopFetchError::Unavailable) => Ok((None, worker_unreachable)),
            }
        }
    }
}

/// Fetches the newest release's `update.json`, if a published release exists.
///
/// Sends `If-None-Match` when a previous ETag is known, records GitHub's
/// rate-limit window and never contacts the API while that window is open.
///
/// # Errors
/// Network or parse failures, as French messages.
async fn fetch_github_manifest(
    prerelease: bool,
    mut cache: UpdateCache,
    cache_path: std::path::PathBuf,
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
        if let Some(etag) = cache
            .etag_prerelease
            .clone()
            .filter(|_| cache.manifest_prerelease.is_some())
        {
            request = request.header("If-None-Match", etag);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("Requête GitHub impossible : {error}"))?;
        cache.last_checked_unix = now;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(UpdateCheck {
                desktop_release: None,
                manifest: cache.manifest_prerelease.clone(),
                cache,
                not_modified: true,
            });
        }
        let old_etag = cache.etag_prerelease.clone();
        remember_headers(&mut cache, response.headers(), true);
        let new_etag = cache.etag_prerelease.take();
        cache.etag_prerelease = old_etag;
        cache.save(&cache_path);
        if !response.status().is_success() {
            return Err(format!("GitHub a répondu {}", response.status()));
        }
        let releases: Vec<ReleaseListing> = response
            .json()
            .await
            .map_err(|error| format!("Liste des versions illisible : {error}"))?;
        let release = releases.into_iter().find(|release| !release.draft);
        cache.etag_prerelease = new_etag;
        release
    } else {
        let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
        let mut request = client
            .get(url)
            .header("Accept", "application/vnd.github+json");
        if let Some(etag) = cache
            .etag
            .clone()
            .filter(|_| cache.manifest_stable.is_some())
        {
            request = request.header("If-None-Match", etag);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => return Err(format!("Requête GitHub impossible : {error}")),
        };
        cache.last_checked_unix = now;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(UpdateCheck {
                desktop_release: None,
                manifest: cache.manifest_stable.clone(),
                cache,
                not_modified: true,
            });
        }
        if response.status().as_u16() == 404 {
            return Ok(UpdateCheck {
                desktop_release: None,
                manifest: None,
                cache,
                not_modified: false,
            });
        }
        let old_etag = cache.etag.clone();
        remember_headers(&mut cache, response.headers(), false);
        let new_etag = cache.etag.take();
        cache.etag = old_etag;
        cache.save(&cache_path);
        if !response.status().is_success() {
            return Err(format!("GitHub a répondu {}", response.status()));
        }
        let release = response
            .json()
            .await
            .map_err(|error| format!("Version illisible : {error}"))?;
        cache.etag = new_etag;
        Some(release)
    };

    let Some(release) = listing else {
        if prerelease {
            cache.etag_prerelease = None;
        } else {
            cache.etag = None;
        }
        return Ok(UpdateCheck {
            desktop_release: None,
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
        if prerelease {
            cache.etag_prerelease = None;
        } else {
            cache.etag = None;
        }
        return Ok(UpdateCheck {
            desktop_release: None,
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
        if prerelease {
            cache.etag_prerelease = None;
        } else {
            cache.etag = None;
        }
        return Ok(UpdateCheck {
            desktop_release: None,
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
    if prerelease {
        cache.manifest_prerelease = Some(manifest.clone());
    } else {
        cache.manifest_stable = Some(manifest.clone());
    }
    Ok(UpdateCheck {
        desktop_release: None,
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
    let builder = reqwest_updater::blocking::Client::builder();
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
    fn target_maps_cpu_and_rejects_unsupported() {
        assert_eq!(desktop_target("linux", "aarch64"), Some(("linux", "arm64")));
        assert_eq!(
            desktop_target("macos", "aarch64"),
            Some(("macos", "universal"))
        );
        assert_eq!(
            desktop_target("windows", "x86_64"),
            Some(("windows", "universal"))
        );
        assert_eq!(desktop_target("linux", "riscv64"), None);
    }

    fn mock_http(status: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let body = body.to_owned();
        let job = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut bytes = [0; 4096];
            let len = socket.read(&mut bytes).unwrap();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&bytes[..len]).into_owned()
        });
        (format!("http://{address}/manifest"), job)
    }

    #[tokio::test]
    async fn desktop_requests_handle_empty_failure_and_invalid_without_etags() {
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let (url, job) = mock_http("200 OK", r#"{"hasRelease":false}"#);
        assert!(
            desktop_source(
                &client,
                &url,
                Some(&[("arch", "arm64")]),
                false,
                "linux",
                "arm64",
                false
            )
            .await
            .unwrap()
            .is_none()
        );
        let request = job.join().unwrap().to_lowercase();
        assert!(request.contains("arch=arm64"));
        assert!(!request.contains("if-none-match"));
        assert!(!request.contains("authorization:"));
        let (url, job) = mock_http("503 Service Unavailable", "");
        assert!(matches!(
            desktop_source(&client, &url, None, false, "linux", "arm64", false).await,
            Err(DesktopFetchError::Unavailable)
        ));
        job.join().unwrap();
        let (url, job) = mock_http("200 OK", "not json");
        assert!(matches!(
            desktop_source(&client, &url, None, false, "linux", "arm64", true).await,
            Err(DesktopFetchError::Invalid(_))
        ));
        job.join().unwrap();
    }

    #[tokio::test]
    async fn worker_falls_back_to_cdn_without_forwarding_headers() {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        for status in ["204 No Content", "404 Not Found", "503 Service Unavailable"] {
            let (worker, first) = mock_http(status, "");
            let (cdn, second) = mock_http("200 OK", r#"{"hasRelease":false}"#);
            let (release, unavailable) = discover_desktop(
                &client,
                &worker,
                &cdn,
                &[("arch", "arm64")],
                false,
                "linux",
                "arm64",
            )
            .await
            .unwrap();
            assert!(release.is_none());
            assert!(!unavailable);
            first.join().unwrap();
            let request = second.join().unwrap().to_lowercase();
            assert!(!request.contains("if-none-match"));
            assert!(!request.contains("authorization:"));
        }
        let (worker, job) = mock_http("200 OK", "not json");
        assert!(
            discover_desktop(
                &client,
                &worker,
                "http://127.0.0.1:1",
                &[],
                false,
                "linux",
                "arm64"
            )
            .await
            .is_err()
        );
        job.join().unwrap();
    }

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
        remember_headers(&mut cache, &headers, false);
        assert_eq!(cache.etag.as_deref(), Some("\"release-42\""));
        // Still blocked one hour before the reset, free afterwards.
        assert_eq!(cache.blocked_for(1_999_996_400), Some(3_600));
        assert_eq!(cache.blocked_for(2_000_000_000), None);
        // An ordinary answer without headers never invents a quota window.
        let mut plain = UpdateCache::default();
        remember_headers(&mut plain, &reqwest::header::HeaderMap::new(), false);
        assert_eq!(plain.etag, None);
        assert_eq!(plain.etag_prerelease, None);
        assert_eq!(plain.rate_limited_until_unix, 0);
    }

    #[test]
    fn config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(CONFIG.slug, SLUG);
    }
}
