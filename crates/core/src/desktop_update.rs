//! Validation of the Windows Desktop feed's metadata. This module does not
//! download or execute an installer; consumers must authenticate and verify it.
use serde::Deserialize;
use url::Url;

use crate::update_feed::{UpdateDecision, UpdateManifest};

/// An explicitly bounded size avoids treating unbounded or Android artifacts as
/// Windows installers. The Worker uses the same 250 MiB maximum.
pub const MAX_INSTALLER_SIZE: u64 = 250 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDesktopRelease {
    has_release: Option<bool>,
    platform: Option<String>,
    arch: Option<String>,
    channel: Option<String>,
    version: Option<String>,
    #[serde(default)]
    mandatory: bool,
    #[serde(rename = "minimum_version")]
    minimum_version: Option<String>,
    #[serde(default)]
    minimum_version_required: bool,
    sha256: Option<String>,
    size_bytes: Option<u64>,
    download_url: Option<String>,
    notes: Option<String>,
    published_at: Option<String>,
}

/// A structurally validated Windows release; not proof of publisher identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopRelease {
    pub manifest: UpdateManifest,
    pub channel: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub download_url: String,
}

/// Parse the public Worker response or CDN latest.json. `None` is an explicit
/// no-release answer, not a network error. Network transport must separately
/// bound the body and must not follow redirects to unapproved origins.
///
/// # Errors
/// Returns an error on malformed, unsafe, or non-Windows metadata.
pub fn parse_desktop_release(
    body: &[u8],
    current: &str,
    prerelease: bool,
) -> Result<Option<DesktopRelease>, &'static str> {
    if body.len() > 128 * 1024 {
        return Err("Manifeste trop volumineux");
    }
    let raw: RawDesktopRelease = serde_json::from_slice(body).map_err(|_| "Manifeste invalide")?;
    if raw.has_release == Some(false) {
        return Ok(None);
    }
    if raw.platform.as_deref() != Some("windows")
        || !matches!(raw.arch.as_deref(), Some("universal" | "universal-x86_64"))
    {
        return Err("Plateforme ou architecture incompatible");
    }
    let channel = raw.channel.ok_or("Canal absent")?;
    if channel != "stable" && channel != "prerelease" {
        return Err("Canal invalide");
    }
    if !prerelease && channel == "prerelease" {
        return Ok(None);
    }
    let sha256 = raw.sha256.ok_or("SHA-256 absent")?;
    if sha256.len() != 64 || !sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("SHA-256 invalide");
    }
    let size_bytes = raw
        .size_bytes
        .filter(|size| *size > 0 && *size <= MAX_INSTALLER_SIZE)
        .ok_or("Taille invalide")?;
    let download_url = raw.download_url.ok_or("URL absente")?;
    let url = Url::parse(&download_url).map_err(|_| "URL invalide")?;
    if url.scheme() != "https"
        || url.host_str() != Some("cdn1.afmarbre.com")
        || url.port().is_some_and(|port| port != 443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().starts_with("/app-updates/desktop/")
        || !url.path().ends_with(".exe")
        || url.path().contains('%')
    {
        return Err("URL d'installateur non autorisée");
    }
    let manifest = UpdateManifest {
        version: raw.version.ok_or("Version absente")?,
        mandatory: raw.mandatory || raw.minimum_version_required,
        minimum_version: raw.minimum_version,
        notes: raw.notes.unwrap_or_default(),
        published_at: raw.published_at.unwrap_or_default(),
    };
    if matches!(
        manifest.decision_for(current),
        UpdateDecision::InvalidVersion
    ) {
        return Err("Version invalide");
    }
    Ok(Some(DesktopRelease {
        manifest,
        channel,
        sha256: sha256.to_ascii_lowercase(),
        size_bytes,
        download_url,
    }))
}

/// Human-readable binary size using French unit names.
#[must_use]
pub fn format_installer_size(bytes: u64) -> String {
    format!("{:.1} Mo", bytes as f64 / (1024.0 * 1024.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> serde_json::Value {
        serde_json::json!({"hasRelease":true,"platform":"windows","arch":"universal-x86_64",
          "channel":"stable","version":"0.4.0","mandatory":false,"minimum_version":"0.3.0",
          "minimumVersionRequired":true,"sha256":"a".repeat(64),"sizeBytes":44879053,
          "downloadUrl":"https://cdn1.afmarbre.com/app-updates/desktop/app-setup.exe",
          "notes":"Notes","publishedAt":"2026-01-01"})
    }

    #[test]
    fn worker_response_and_required_minimum() {
        let release =
            parse_desktop_release(&serde_json::to_vec(&fixture()).unwrap(), "0.2.0", false)
                .unwrap()
                .unwrap();
        assert!(matches!(
            release.manifest.decision_for("0.2.0"),
            UpdateDecision::Mandatory { .. }
        ));
        assert_eq!(format_installer_size(release.size_bytes), "42.8 Mo");
        assert_eq!(release.sha256.len(), 64);
    }

    #[test]
    fn empty_release_falls_back() {
        assert_eq!(
            parse_desktop_release(br#"{"hasRelease":false,"version":null}"#, "0.2.0", false),
            Ok(None)
        );
    }

    #[test]
    fn rejects_unsafe_and_android_downloads() {
        let mut value = fixture();
        for url in [
            "http://cdn1.afmarbre.com/app-updates/desktop/a.exe",
            "https://evil.example/app-updates/desktop/a.exe",
            "https://cdn1.afmarbre.com/app-updates/android/a.apk",
            "https://cdn1.afmarbre.com/app-updates/desktop/a.exe?x=1",
        ] {
            value["downloadUrl"] = url.into();
            assert!(
                parse_desktop_release(&serde_json::to_vec(&value).unwrap(), "0.2.0", false)
                    .is_err()
            );
        }
        value = fixture();
        value["sha256"] = "bad".into();
        assert!(
            parse_desktop_release(&serde_json::to_vec(&value).unwrap(), "0.2.0", false).is_err()
        );
    }
}
