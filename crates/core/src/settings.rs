//! User settings, persisted as JSON in the config folder.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Appearance mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// Dark zinc palette, the product default.
    #[default]
    Dark,
    Light,
    /// Follow the operating system appearance.
    System,
}

/// Update channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    /// Stable GitHub releases only.
    #[default]
    Stable,
    /// Also offer pre-releases (tags with a suffix such as `-rc.1`).
    Prerelease,
}

/// A version observed when the application started. This is not proof of an
/// installation time: older versions may predate this record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionObservation {
    pub version: String,
    pub observed_unix: u64,
}

/// All settings the settings screen can change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// `fr` (default) or `en`.
    pub language: String,
    pub theme: ThemeMode,
    /// User interface scale percentage; clamped before applying.
    pub ui_scale_percent: u16,
    /// Filename of a custom palette in `themes/`, if any.
    pub custom_theme: Option<String>,
    /// Keep running in the tray with the window closed.
    pub close_to_tray: bool,
    /// Start at login.
    pub autostart: bool,
    /// Start hidden in the tray when launched at login.
    pub start_hidden: bool,
    /// Check for updates on launch and then periodically.
    pub update_check_on_start: bool,
    /// Minimum delay between two automatic checks. GitHub's unauthenticated
    /// API allows 60 requests per hour per address; 6 h leaves room for many
    /// machines behind one shop connection while still reacting same-day.
    pub update_check_interval_hours: u64,
    /// Download an available update in the background, then install it on
    /// the next restart. Checks stay rate-limit aware either way.
    pub update_auto_download: bool,
    pub update_channel: UpdateChannel,
    /// First version observed after this feature was introduced, not
    /// necessarily the version that was originally installed.
    pub first_observed_version: Option<String>,
    /// Bounded chronological log of locally observed version transitions.
    pub update_history: Vec<VersionObservation>,
    /// Background polling interval in seconds (dashboard, alerts…).
    pub poll_interval_secs: u64,
    /// Native notifications for high-severity stock alerts.
    pub notifications_enabled: bool,
    /// Custom title bar drawn by the app.
    pub custom_titlebar: bool,
    /// Allow system window material when the Windows accessibility policy permits it.
    pub windows_material: bool,
    /// Honour the web's `prefers-reduced-motion` rule: every animation then
    /// resolves instantly (the forked egui exposes no OS signal, so the
    /// choice is an in-app, persisted toggle).
    pub reduced_motion: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "fr".to_owned(),
            theme: ThemeMode::Light,
            ui_scale_percent: 100,
            custom_theme: None,
            close_to_tray: true,
            autostart: false,
            start_hidden: true,
            update_check_on_start: true,
            update_check_interval_hours: 6,
            update_auto_download: false,
            update_channel: UpdateChannel::Stable,
            first_observed_version: None,
            update_history: Vec::new(),
            poll_interval_secs: 120,
            notifications_enabled: true,
            custom_titlebar: true,
            windows_material: true,
            reduced_motion: false,
        }
    }
}

impl Settings {
    /// Record an observed version transition, not an inferred installation.
    /// `false` means no data changed and no disk write is required.
    pub fn observe_version(&mut self, version: &str, observed_unix: u64) -> bool {
        if version.trim().is_empty() {
            return false;
        }
        let mut changed = false;
        if self.first_observed_version.is_none() {
            self.first_observed_version = Some(version.to_owned());
            changed = true;
        }
        if self
            .update_history
            .last()
            .is_none_or(|entry| entry.version != version)
        {
            self.update_history.push(VersionObservation {
                version: version.to_owned(),
                observed_unix,
            });
            const HISTORY_LIMIT: usize = 30;
            if self.update_history.len() > HISTORY_LIMIT {
                self.update_history
                    .drain(..self.update_history.len() - HISTORY_LIMIT);
            }
            changed = true;
        }
        changed
    }

    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Whole-file replacement; call off the UI thread (see `fastframe-file`).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        fastframe_file::write(path, json)
    }

    #[must_use]
    pub fn is_french(&self) -> bool {
        self.language.as_str() != "en"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_french_and_light() {
        let settings = Settings::default();
        assert!(settings.is_french());
        assert_eq!(settings.theme, ThemeMode::Light);
        assert!(settings.close_to_tray);
        assert!(settings.windows_material);
    }

    #[test]
    fn old_settings_keep_a_default_interface_scale() {
        let settings: Settings =
            serde_json::from_str(r#"{"theme":"dark","language":"fr"}"#).unwrap();
        assert_eq!(settings.ui_scale_percent, 100);
        assert_eq!(settings.theme, ThemeMode::Dark);
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
    }

    #[test]
    fn records_only_observed_version_transitions_and_bounds_history() {
        let mut settings: Settings = serde_json::from_str(r#"{"language":"fr"}"#).unwrap();
        assert_eq!(settings.first_observed_version, None);
        assert!(settings.observe_version("0.3.1", 100));
        assert!(!settings.observe_version("0.3.1", 200));
        assert!(settings.observe_version("0.3.2", 300));
        assert!(settings.observe_version("0.3.1", 400));
        assert_eq!(settings.first_observed_version.as_deref(), Some("0.3.1"));
        assert_eq!(settings.update_history.len(), 3);
        for index in 0..40 {
            settings.observe_version(&format!("test-{index}"), index);
        }
        assert_eq!(settings.update_history.len(), 30);
        assert_eq!(settings.first_observed_version.as_deref(), Some("0.3.1"));
        let encoded = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<Settings>(&encoded).unwrap(),
            settings
        );
    }

    #[test]
    fn damaged_settings_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("atlas-settings-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings, Settings::default());
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
