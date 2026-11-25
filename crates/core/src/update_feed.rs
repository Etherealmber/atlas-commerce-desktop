//! The GitHub-Releases update feed and the mandatory-update gate.
//!
//! The desktop app is updated exclusively from this repository's GitHub
//! Releases — never a CDN. Each release carries an `update.json` asset with a
//! mandatory flag. The actual download, checksum verification, helper install
//! and rollback are handled by `fastframe-update`; this module adds the
//! blocking gate shown before the shell when a release marks itself
//! mandatory.

use serde::{Deserialize, Serialize};

/// Name of the manifest asset every release uploads.
pub const MANIFEST_ASSET: &str = "update.json";

/// The release manifest, signed only by being attached to a GitHub release.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateManifest {
    /// The version this release publishes, without a leading `v`.
    pub version: String,
    /// When true, the app must update before reaching the shell.
    #[serde(default)]
    pub mandatory: bool,
    /// Versions older than this one are forced through the mandatory gate.
    #[serde(default)]
    pub minimum_version: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub published_at: String,
}

/// What the interface should do about a fetched manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    /// The feed or running version is not a valid semantic version.
    InvalidVersion,
    /// Nothing newer than the running version.
    UpToDate,
    /// A normal, optional update the settings screen offers.
    Optional { version: String, notes: String },
    /// A blocking update shown before the shell.
    Mandatory { version: String, notes: String },
}

impl UpdateManifest {
    /// Compares the manifest against the running `current_version`.
    #[must_use]
    pub fn decision_for(&self, current_version: &str) -> UpdateDecision {
        let (Ok(version), Ok(current)) = (
            semver::Version::parse(&self.version),
            semver::Version::parse(current_version),
        ) else {
            return UpdateDecision::InvalidVersion;
        };
        let minimum = match self.minimum_version.as_deref() {
            Some(value) => match semver::Version::parse(value) {
                Ok(value) if !value.cmp_precedence(&version).is_gt() => Some(value),
                _ => return UpdateDecision::InvalidVersion,
            },
            None => None,
        };
        if !version.cmp_precedence(&current).is_gt() {
            return UpdateDecision::UpToDate;
        }
        let forced_by_minimum =
            minimum.is_some_and(|minimum| minimum.cmp_precedence(&current).is_gt());
        if self.mandatory || forced_by_minimum {
            UpdateDecision::Mandatory {
                version: self.version.clone(),
                notes: self.notes.clone(),
            }
        } else {
            UpdateDecision::Optional {
                version: self.version.clone(),
                notes: self.notes.clone(),
            }
        }
    }
}

/// Strict SemVer precedence, including prerelease identifiers. Invalid versions
/// never compare as newer; callers handling feeds must report invalid input.
#[must_use]
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (
        semver::Version::parse(candidate),
        semver::Version::parse(current),
    ) {
        (Ok(candidate), Ok(current)) => candidate.cmp_precedence(&current).is_gt(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparisons() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-rc.1", "0.1.0"));
        assert!(is_newer("0.1.0", "0.1.0-rc.1"));
        assert!(!is_newer("v0.2", "0.1.9"));
        assert!(is_newer("0.2.0-beta.2", "0.2.0-beta.1"));
        assert!(!is_newer("0.2.0-beta.01", "0.2.0-beta.1"));
        assert!(!is_newer("0.2.0+build.2", "0.2.0+build.1"));
        assert!(!is_newer("not-a-version", "0.1.0"));
    }

    #[test]
    fn invalid_manifest_never_reports_up_to_date() {
        let mut manifest = UpdateManifest {
            version: "not-semver".into(),
            mandatory: false,
            minimum_version: None,
            notes: String::new(),
            published_at: String::new(),
        };
        assert_eq!(
            manifest.decision_for("0.1.0"),
            UpdateDecision::InvalidVersion
        );
        manifest.version = "0.2.0".into();
        manifest.minimum_version = Some("0.3.0".into());
        assert_eq!(
            manifest.decision_for("0.1.0"),
            UpdateDecision::InvalidVersion
        );
        manifest.minimum_version = Some("garbage".into());
        assert_eq!(
            manifest.decision_for("0.1.0"),
            UpdateDecision::InvalidVersion
        );
    }

    #[test]
    fn optional_update() {
        let manifest = UpdateManifest {
            version: "0.2.0".to_owned(),
            mandatory: false,
            minimum_version: None,
            notes: "Corrections".to_owned(),
            published_at: String::new(),
        };
        assert_eq!(
            manifest.decision_for("0.1.0"),
            UpdateDecision::Optional {
                version: "0.2.0".to_owned(),
                notes: "Corrections".to_owned()
            }
        );
        assert_eq!(manifest.decision_for("0.2.0"), UpdateDecision::UpToDate);
    }

    #[test]
    fn mandatory_gate_blocks_old_versions() {
        let manifest = UpdateManifest {
            version: "0.3.0".to_owned(),
            mandatory: false,
            minimum_version: Some("0.2.0".to_owned()),
            notes: String::new(),
            published_at: String::new(),
        };
        assert!(matches!(
            manifest.decision_for("0.1.0"),
            UpdateDecision::Mandatory { .. }
        ));
        assert!(matches!(
            manifest.decision_for("0.2.0"),
            UpdateDecision::Optional { .. }
        ));
        let forced = UpdateManifest {
            version: "0.3.0".to_owned(),
            mandatory: true,
            minimum_version: None,
            notes: String::new(),
            published_at: String::new(),
        };
        assert!(matches!(
            forced.decision_for("0.2.0"),
            UpdateDecision::Mandatory { .. }
        ));
    }
}
