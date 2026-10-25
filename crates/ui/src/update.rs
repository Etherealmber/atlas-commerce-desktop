//! Self-update, fed exclusively by this repository's GitHub Releases.
//!
//! - [`fetch_manifest`] reads the `update.json` release asset and powers the
//!   mandatory-update gate. No CDN is ever contacted.
//! - [`build_updater`] hands the rest (checksum verification, helper install
//!   with rollback, AppImage replacement) to `fastframe-update`.

use atlas_core::update_feed::{MANIFEST_ASSET, UpdateManifest};
use fastframe_update::{MacConfig, ReqwestTransport, UpdateConfig, Updater};
use serde::Deserialize;

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
/// "The release has no unique <name> download"; a not-yet-propagated file
/// comes back as an HTTP 404 from the release asset host. Both happen in the
/// publish window between release creation and the asset upload step.
#[must_use]
pub fn is_release_not_ready(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("404") || lower.contains("no unique")
}

/// Builds the blocking fastframe updater on the system-proxy-aware client.
///
/// # Errors
/// When the transport cannot be built.
pub fn build_updater() -> anyhow::Result<Updater> {
    let builder = reqwest::blocking::Client::builder();
    Ok(Updater::new(CONFIG, ReqwestTransport::new(builder)?))
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
    fn config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(CONFIG.slug, SLUG);
    }
}
