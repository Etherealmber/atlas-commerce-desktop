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

/// All settings the settings screen can change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// `fr` (default) or `en`.
    pub language: String,
    pub theme: ThemeMode,
    /// Filename of a custom palette in `themes/`, if any.
    pub custom_theme: Option<String>,
    /// Keep running in the tray with the window closed.
    pub close_to_tray: bool,
    /// Start at login.
    pub autostart: bool,
    /// Start hidden in the tray when launched at login.
    pub start_hidden: bool,
    /// Check for updates on launch.
    pub update_check_on_start: bool,
    pub update_channel: UpdateChannel,
    /// Background polling interval in seconds (dashboard, alerts…).
    pub poll_interval_secs: u64,
    /// Native notifications for high-severity stock alerts.
    pub notifications_enabled: bool,
    /// Custom title bar drawn by the app.
    pub custom_titlebar: bool,
    /// Honour the web's `prefers-reduced-motion` rule: every animation then
    /// resolves instantly (the forked egui exposes no OS signal, so the
    /// choice is an in-app, persisted toggle).
    pub reduced_motion: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "fr".to_owned(),
            theme: ThemeMode::Dark,
            custom_theme: None,
            close_to_tray: true,
            autostart: false,
            start_hidden: true,
            update_check_on_start: true,
            update_channel: UpdateChannel::Stable,
            poll_interval_secs: 120,
            notifications_enabled: true,
            custom_titlebar: true,
            reduced_motion: false,
        }
    }
}

impl Settings {
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
    fn defaults_are_french_and_dark() {
        let settings = Settings::default();
        assert!(settings.is_french());
        assert_eq!(settings.theme, ThemeMode::Dark);
        assert!(settings.close_to_tray);
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
