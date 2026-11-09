//! Self-update, fed exclusively by this repository's GitHub Releases.
//!
//! - [`fetch_manifest`] reads the `update.json` release asset and powers the
//!   mandatory-update gate. No CDN is ever contacted.
//! - [`build_updater`] hands the rest (checksum verification, helper install
//!   with rollback, AppImage replacement) to `fastframe-update`.

use atlas_core::update_feed::{MANIFEST_ASSET, UpdateManifest};
use fastframe_update::{MacConfig, ReqwestTransport, UpdateConfig, Updater};
use serde::Deserialize;
use std::io::Read;

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
/// # Errors
/// Network or parse failures, as French messages.
pub async fn fetch_manifest(prerelease: bool) -> Result<Option<UpdateManifest>, String> {
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
        let releases: Vec<ReleaseListing> = client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|error| format!("Requête GitHub impossible : {error}"))?
            .error_for_status()
            .map_err(|error| format!("GitHub a répondu {error}"))?
            .json()
            .await
            .map_err(|error| format!("Liste des versions illisible : {error}"))?;
        releases.into_iter().find(|release| !release.draft)
    } else {
        let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
        match client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
        {
            Ok(response) if response.status().as_u16() == 404 => return Ok(None),
            Ok(response) => Some(
                response
                    .error_for_status()
                    .map_err(|error| format!("GitHub a répondu {error}"))?
                    .json()
                    .await
                    .map_err(|error| format!("Version illisible : {error}"))?,
            ),
            Err(error) => return Err(format!("Requête GitHub impossible : {error}")),
        }
    };

    let Some(release) = listing else {
        return Ok(None);
    };
    let Some(asset) = release
        .assets
        .iter()
        .find(|asset| asset.name == MANIFEST_ASSET)
    else {
        return Ok(None);
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
        return Ok(None);
    }
    let manifest: UpdateManifest = manifest_response
        .error_for_status()
        .map_err(|error| format!("GitHub a répondu {error}"))?
        .json()
        .await
        .map_err(|error| format!("Manifeste illisible : {error}"))?;
    Ok(Some(manifest))
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
    fn config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(CONFIG.slug, SLUG);
    }
}
