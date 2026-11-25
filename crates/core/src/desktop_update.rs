//! Strict parsing of Desktop update metadata. Parsing alone does not verify a
//! downloaded artifact or authenticate the publisher.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use url::Url;

use crate::update_feed::{UpdateDecision, UpdateManifest};

/// The Worker accepts artifacts up to 350 MiB.
pub const MAX_INSTALLER_SIZE: u64 = 350 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDesktopRelease {
    has_release: Option<bool>,
    platform_available: Option<bool>,
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
    platforms: Option<HashMap<String, HashMap<String, Asset>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Asset {
    platform: String,
    arch: String,
    kind: String,
    sha256: Option<String>,
    size_bytes: Option<u64>,
    download_url: Option<String>,
}

/// Structurally validated release, not proof of publisher identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopRelease {
    pub manifest: UpdateManifest,
    pub channel: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub download_url: String,
}

/// Parse a bounded API response or latest.json for a particular OS and CPU.
/// `None` means an explicit unavailable release/OS/channel; transport errors
/// and malformed metadata must not be reported as up-to-date.
///
/// # Errors
/// Returns an error on malformed or unsafe metadata.
pub fn parse_desktop_release(
    body: &[u8],
    current: &str,
    prerelease: bool,
    os: &str,
    arch: &str,
) -> Result<Option<DesktopRelease>, &'static str> {
    if body.len() > 128 * 1024 {
        return Err("Manifeste trop volumineux");
    }
    if !matches!(os, "windows" | "macos" | "linux")
        || !matches!(arch, "x86_64" | "arm64" | "universal")
    {
        return Err("Plateforme ou architecture incompatible");
    }
    let raw: RawDesktopRelease = serde_json::from_slice(body).map_err(|_| "Manifeste invalide")?;
    if raw.has_release == Some(false) || raw.platform_available == Some(false) {
        return Ok(None);
    }
    let channel = raw.channel.ok_or("Canal absent")?;
    if channel != "stable" && channel != "prerelease" {
        return Err("Canal invalide");
    }
    if !prerelease && channel == "prerelease" {
        return Ok(None);
    }

    // The public API already selects an artifact. latest.json carries a map;
    // don't fall back to another CPU or an archive when a matching binary is absent.
    let (platform, artifact_arch, kind, sha256, size_bytes, download_url) =
        if raw.has_release.is_some() {
            (
                raw.platform,
                raw.arch,
                None,
                raw.sha256,
                raw.size_bytes,
                raw.download_url,
            )
        } else if let Some(platforms) = &raw.platforms {
            let Some(choices) = platforms.get(os) else {
                return Ok(None);
            };
            let selected = choices.get(arch).or_else(|| {
                if os != "linux" {
                    choices.get("universal")
                } else {
                    None
                }
            });
            let Some(asset) = selected else {
                return Ok(None);
            };
            (
                Some(asset.platform.clone()),
                Some(asset.arch.clone()),
                Some(asset.kind.clone()),
                asset.sha256.clone(),
                asset.size_bytes,
                asset.download_url.clone(),
            )
        } else {
            (
                raw.platform,
                raw.arch,
                None,
                raw.sha256,
                raw.size_bytes,
                raw.download_url,
            )
        };
    let expected_kind = if os == "linux" {
        "appimage"
    } else {
        "installer"
    };
    let valid_arch = artifact_arch.as_deref().is_some_and(|a| {
        a == arch
            || (os != "linux" && a == "universal")
            || (os == "windows" && a == "universal-x86_64")
    });
    if platform.as_deref() != Some(os)
        || !valid_arch
        || kind.as_deref().is_some_and(|k| k != expected_kind)
    {
        return Err("Artefact incompatible");
    }
    let sha256 = sha256.ok_or("SHA-256 absent")?;
    if sha256.len() != 64 || !sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("SHA-256 invalide");
    }
    let size_bytes = size_bytes
        .filter(|size| *size > 0 && *size <= MAX_INSTALLER_SIZE)
        .ok_or("Taille invalide")?;
    let download_url = download_url.ok_or("URL absente")?;
    let url = Url::parse(&download_url).map_err(|_| "URL invalide")?;
    let extension = match os {
        "windows" => ".exe",
        "macos" => ".dmg",
        _ => ".AppImage",
    };
    if url.scheme() != "https"
        || url.host_str() != Some("cdn1.afmarbre.com")
        || url.port().is_some_and(|port| port != 443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().starts_with("/app-updates/desktop/")
        || !url.path().ends_with(extension)
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
    let decision = manifest.decision_for(current);
    if matches!(decision, UpdateDecision::InvalidVersion)
        || (raw.minimum_version_required && matches!(decision, UpdateDecision::UpToDate))
    {
        return Err("Version ou politique minimale incohérente");
    }
    Ok(Some(DesktopRelease {
        manifest,
        channel,
        sha256: sha256.to_ascii_lowercase(),
        size_bytes,
        download_url,
    }))
}

/// Copies one artifact while hashing its bytes. It succeeds only when both the
/// exact advertised size and SHA-256 match. The caller must use a private,
/// freshly created staging file, delete it on error, and prevent replacement
/// between verification and handoff; this function never executes the file.
///
/// # Errors
/// Returns an error on I/O, invalid length, or checksum mismatch.
pub fn verify_desktop_artifact(
    mut source: impl Read,
    mut target: impl Write,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), &'static str> {
    if expected_size == 0
        || expected_size > MAX_INSTALLER_SIZE
        || expected_sha256.len() != 64
        || !expected_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Métadonnées de vérification invalides");
    }
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let count = source.read(&mut buf).map_err(|_| "Lecture impossible")?;
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).ok_or("Taille invalide")?;
        if total > expected_size {
            return Err("Taille incorrecte");
        }
        hash.update(&buf[..count]);
        target
            .write_all(&buf[..count])
            .map_err(|_| "Écriture impossible")?;
    }
    if total != expected_size {
        return Err("Taille incorrecte");
    }
    if !hex::encode(hash.finalize()).eq_ignore_ascii_case(expected_sha256) {
        return Err("SHA-256 incorrect");
    }
    target.flush().map_err(|_| "Écriture impossible")?;
    Ok(())
}

/// Human-readable binary size using French unit names.
#[must_use]
pub fn format_installer_size(bytes: u64) -> String {
    format!("{:.1} Mo", bytes as f64 / (1024.0 * 1024.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(os: &str, arch: &str, ext: &str) -> serde_json::Value {
        serde_json::json!({"hasRelease":true,"platformAvailable":true,"platform":os,"arch":arch,
          "channel":"stable","version":"0.4.0","mandatory":false,"minimum_version":"0.3.0",
          "minimumVersionRequired":true,"sha256":"a".repeat(64),"sizeBytes":44879053,
          "downloadUrl":format!("https://cdn1.afmarbre.com/app-updates/desktop/app{ext}"),
          "notes":"Notes","publishedAt":"2026-01-01"})
    }
    #[test]
    fn worker_responses_and_required_minimum() {
        for (os, arch, ext) in [
            ("windows", "universal", ".exe"),
            ("macos", "universal", ".dmg"),
            ("linux", "arm64", ".AppImage"),
            ("linux", "x86_64", ".AppImage"),
        ] {
            let release = parse_desktop_release(
                &serde_json::to_vec(&fixture(os, arch, ext)).unwrap(),
                "0.2.0",
                false,
                os,
                arch,
            )
            .unwrap()
            .unwrap();
            assert!(matches!(
                release.manifest.decision_for("0.2.0"),
                UpdateDecision::Mandatory { .. }
            ));
            assert_eq!(format_installer_size(release.size_bytes), "42.8 Mo");
            assert_eq!(release.sha256.len(), 64);
        }
    }
    #[test]
    fn unavailable_release_or_platform_falls_back() {
        for body in [
            br#"{"hasRelease":false}"#.as_slice(),
            br#"{"hasRelease":true,"platformAvailable":false}"#.as_slice(),
        ] {
            assert_eq!(
                parse_desktop_release(body, "0.2.0", false, "linux", "arm64"),
                Ok(None)
            );
        }
    }
    #[test]
    fn direct_manifest_selects_only_matching_platform_and_arch() {
        let mut body = fixture("windows", "universal", ".exe");
        body.as_object_mut().unwrap().remove("hasRelease");
        body["platforms"] = serde_json::json!({"linux":{"arm64":{"platform":"linux","arch":"arm64","kind":"appimage","sha256":"b".repeat(64),"sizeBytes":12345,"downloadUrl":"https://cdn1.afmarbre.com/app-updates/desktop/a.AppImage"}}});
        let bytes = serde_json::to_vec(&body).unwrap();
        assert!(
            parse_desktop_release(&bytes, "0.2.0", false, "linux", "arm64")
                .unwrap()
                .is_some()
        );
        assert_eq!(
            parse_desktop_release(&bytes, "0.2.0", false, "linux", "x86_64"),
            Ok(None)
        );
    }
    #[test]
    fn verifies_exact_bytes_and_rejects_mismatch_or_extra_data() {
        let body = b"test";
        let digest = hex::encode(Sha256::digest(body));
        let mut output = Vec::new();
        assert_eq!(
            verify_desktop_artifact(&body[..], &mut output, 4, &digest),
            Ok(())
        );
        assert_eq!(output, body);
        assert_eq!(
            verify_desktop_artifact(&body[..], Vec::new(), 4, &"0".repeat(64)),
            Err("SHA-256 incorrect")
        );
        assert_eq!(
            verify_desktop_artifact(&body[..], Vec::new(), 3, &digest),
            Err("Taille incorrecte")
        );
        assert_eq!(
            verify_desktop_artifact(&body[..], Vec::new(), 5, &digest),
            Err("Taille incorrecte")
        );
    }

    #[test]
    fn rejects_unsafe_and_android_downloads() {
        let mut value = fixture("windows", "universal", ".exe");
        for url in [
            "http://cdn1.afmarbre.com/app-updates/desktop/a.exe",
            "https://evil.example/app-updates/desktop/a.exe",
            "https://cdn1.afmarbre.com/app-updates/android/a.apk",
            "https://cdn1.afmarbre.com/app-updates/desktop/a.exe?x=1",
        ] {
            value["downloadUrl"] = url.into();
            assert!(
                parse_desktop_release(
                    &serde_json::to_vec(&value).unwrap(),
                    "0.2.0",
                    false,
                    "windows",
                    "universal"
                )
                .is_err()
            );
        }
    }
}
