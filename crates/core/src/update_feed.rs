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
        if !is_newer(&self.version, current_version) {
            return UpdateDecision::UpToDate;
        }
        let forced_by_minimum = self
            .minimum_version
            .as_deref()
            .is_some_and(|minimum| is_newer(minimum, current_version));
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

/// Numeric semantic-version comparison: true when `candidate` is newer than
/// `current`. Pre-release suffixes (`-rc.1`) sort below the same core version.
#[must_use]
pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse_semver(candidate).is_some_and(|candidate_parts| {
        parse_semver(current).is_some_and(|current_parts| candidate_parts > current_parts)
    })
}

fn parse_semver(value: &str) -> Option<(u64, u64, u64, u64)> {
    let trimmed = value.trim().trim_start_matches('v');
    let (core, pre) = trimmed.split_once('-').unwrap_or((trimmed, ""));
    let mut numbers = core.split('.');
    let major = numbers.next()?.parse().ok()?;
    let minor = numbers
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let patch = numbers
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    // A release without a suffix ranks above any pre-release.
    let pre_rank = if pre.is_empty() { 1 } else { 0 };
    Some((major, minor, patch, pre_rank))
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
        assert!(is_newer("v0.2", "0.1.9"));
        assert!(!is_newer("not-a-version", "0.1.0"));
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
